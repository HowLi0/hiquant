//! 回测引擎：把历史 bar 序列推送给策略，并驱动市场系统撮合
//!
//! 工作流程：
//! 1. 注册账户、加载多标的 bar 序列
//! 2. 按时间合并所有 bar，逐根 bar 推送：
//!    - 更新行情价格 → 推送策略 on_bar → 处理订单队列 → 记录净值
//! 3. 输出净值曲线、绩效、QIFI 快照

use crate::equity::{EquityPoint, Performance};
use crate::strategy::{Context, IndicatorPoint, Strategy};
use hiquant_account::Account;
use hiquant_core::{AccountEnvironment, Amount, Date};
use hiquant_data::Bar;
use hiquant_market::QAMarketSystem;
use hiquant_protocol::qifi::Qifi;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 回测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub account_cookie: String,
    pub init_cash: Amount,
    pub performance: Performance,
    pub equity_curve: Vec<EquityPoint>,
    pub indicators: Vec<IndicatorPoint>,
    pub final_qifi: Qifi,
    pub trades: Vec<hiquant_protocol::qifi::Trade>,
}

/// 回测引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub account_cookie: String,
    pub init_cash: Amount,
    pub environment: AccountEnvironment,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            account_cookie: "backtest".into(),
            init_cash: 1_000_000.0,
            environment: AccountEnvironment::Backtest,
        }
    }
}

/// 回测引擎
pub struct BacktestEngine {
    market: QAMarketSystem,
    account: Arc<Account>,
    strategy: Box<dyn Strategy>,
    bars: HashMap<String, Vec<Bar>>,
}

impl BacktestEngine {
    pub fn new(cfg: BacktestConfig, strategy: Box<dyn Strategy>) -> Self {
        let market = QAMarketSystem::new("backtest");
        let account = Arc::new(Account::new_stock(cfg.account_cookie.clone(), cfg.init_cash));
        market.register_account_arc(account.clone());
        Self {
            market,
            account,
            strategy,
            bars: HashMap::new(),
        }
    }

    /// 加载某标的的 bar 序列（必须按时间升序）
    pub fn load_bars(&mut self, code: impl Into<String>, bars: Vec<Bar>) {
        self.bars.insert(code.into(), bars);
    }

    /// 执行回测，返回结果
    pub fn run(&mut self) -> BacktestResult {
        let init_cash = self.account.init_cash;
        let account_cookie = self.account.account_cookie.clone();
        let instruments = self.strategy.instruments();

        let mut indicators: Vec<IndicatorPoint> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();

        // on_start
        {
            let mut ctx = Context {
                market: &self.market,
                account_cookie: account_cookie.clone(),
                current_datetime: String::new(),
                indicators: &mut indicators,
            };
            self.strategy.on_start(&mut ctx);
        }

        // 合并时间线：从所有 bar 中按 datetime 排序
        let mut timeline: Vec<(String, Bar)> = Vec::new();
        for code in &instruments {
            if let Some(bars) = self.bars.get(code) {
                for b in bars {
                    timeline.push((code.clone(), b.clone()));
                }
            }
        }
        // 按标的再按时间排序后，整体按 datetime 稳定排序
        timeline.sort_by(|a, b| a.1.datetime.cmp(&b.1.datetime));

        let mut last_date: Option<Date> = None;
        let mut trade_count = 0usize;

        for (_code, bar) in &timeline {
            let dt = bar.datetime.clone();
            self.market.set_datetime(&dt);

            // 检测日切换
            if let Some(date) = bar.trading_date {
                if last_date.map(|d| d != date).unwrap_or(false) {
                    if let Some(prev) = last_date {
                        let mut ctx = Context {
                            market: &self.market,
                            account_cookie: account_cookie.clone(),
                            current_datetime: dt.clone(),
                            indicators: &mut indicators,
                        };
                        self.strategy.on_day_close(&mut ctx, prev);
                    }
                    let mut ctx = Context {
                        market: &self.market,
                        account_cookie: account_cookie.clone(),
                        current_datetime: dt.clone(),
                        indicators: &mut indicators,
                    };
                    self.strategy.on_day_open(&mut ctx, date);
                    self.market.set_date(date);
                }
                last_date = Some(date);
            }

            // 更新行情
            self.market.update_price(&bar.order_book_id, bar.close);

            // 策略决策
            let mut ctx = Context {
                market: &self.market,
                account_cookie: account_cookie.clone(),
                current_datetime: dt.clone(),
                indicators: &mut indicators,
            };
            self.strategy.on_bar(&mut ctx, bar);

            // 撮合
            let trades = self.market.process_order_queue();
            trade_count += trades.len();

            // 记录净值
            let summary = self.account.summary();
            equity_curve.push(EquityPoint {
                date: bar
                    .trading_date
                    .map(|d| d.as_str())
                    .unwrap_or_else(|| dt.split(' ').next().unwrap_or(&dt).to_string()),
                datetime: dt,
                cash: summary.cash,
                market_value: summary.market_value,
                total_value: summary.total_value,
                float_pnl: summary.float_pnl,
                close_pnl: summary.close_pnl,
            });
        }

        // 最后一天 on_day_close
        if let Some(date) = last_date {
            let mut ctx = Context {
                market: &self.market,
                account_cookie: account_cookie.clone(),
                current_datetime: self.market.current_datetime(),
                indicators: &mut indicators,
            };
            self.strategy.on_day_close(&mut ctx, date);
        }

        // on_stop
        {
            let mut ctx = Context {
                market: &self.market,
                account_cookie: account_cookie.clone(),
                current_datetime: self.market.current_datetime(),
                indicators: &mut indicators,
            };
            self.strategy.on_stop(&mut ctx);
        }

        // 日终结算
        self.market.daily_settle_all();

        // 绩效
        let mut perf = Performance::from_equity(init_cash, &equity_curve);
        perf.trade_count = trade_count;

        // QIFI 快照
        let final_qifi = self.account.to_qifi();
        let trades: Vec<hiquant_protocol::qifi::Trade> =
            final_qifi.trades.values().cloned().collect();

        BacktestResult {
            account_cookie,
            init_cash,
            performance: perf,
            equity_curve,
            indicators,
            final_qifi,
            trades,
        }
    }

    pub fn account(&self) -> &Arc<Account> {
        &self.account
    }

    pub fn market(&self) -> &QAMarketSystem {
        &self.market
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sma::{SmaCrossStrategy, SmaParams};
    use hiquant_data::SampleDataGenerator;

    #[test]
    fn backtest_sma_cross_runs() {
        let mut gen = SampleDataGenerator::new(42);
        let bars = gen.gen_daily(
            "000001",
            Date::from_ymd(2024, 1, 2),
            60,
            10.0,
            0.10,
            0.30,
            false,
        );

        let params = SmaParams {
            code: "000001".into(),
            fast: 5,
            slow: 20,
            lots: 100.0,
        };
        let strategy = Box::new(SmaCrossStrategy::new(params));
        let mut engine = BacktestEngine::new(BacktestConfig::default(), strategy);
        engine.load_bars("000001", bars);

        let result = engine.run();
        assert!(!result.equity_curve.is_empty());
        assert!(result.performance.trading_days > 0);
        assert!(result.performance.init_cash > 0.0);
        // 至少记录了一些指标点
        assert!(!result.indicators.is_empty());
    }
}
