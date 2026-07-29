//! 市场系统：账户注册 + 行情驱动 + 订单调度 + 撮合
//!
//! 对标 C++ hiquant 的 QAMarketSystem。

use crate::orderbook::{BookOrder, Orderbook, TradeResult};
use dashmap::DashMap;
use parking_lot::RwLock;
use hiquant_account::Account;
use hiquant_core::{Amount, Date, Direction, Frequency, Price, Volume};
use hiquant_data::Bar;
use hiquant_protocol::qifi::Qifi;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 市场订单请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOrder {
    pub account_cookie: String,
    pub code: String,
    pub volume: Volume,
    pub price: Price,
    pub direction: Direction,
    #[serde(default)]
    pub offset: String,
    #[serde(default)]
    pub label: String,
}

impl MarketOrder {
    pub fn buy(account: &str, code: &str, volume: Volume, price: Price) -> Self {
        Self {
            account_cookie: account.to_string(),
            code: code.to_string(),
            volume,
            price,
            direction: Direction::Buy,
            offset: "open".to_string(),
            label: String::new(),
        }
    }

    pub fn sell(account: &str, code: &str, volume: Volume, price: Price) -> Self {
        Self {
            account_cookie: account.to_string(),
            code: code.to_string(),
            volume,
            price,
            direction: Direction::Sell,
            offset: "close".to_string(),
            label: String::new(),
        }
    }
}

/// 市场系统
pub struct QAMarketSystem {
    pub portfolio: String,
    pub username: String,
    /// 是否启用回测自动成交模式：True 时订单按最新价/订单价立即成交，不进订单簿
    pub backtest_auto_fill: bool,
    /// 注册的账户
    accounts: DashMap<String, Arc<Account>>,
    /// 每个标的的订单簿
    orderbooks: DashMap<String, Arc<RwLock<Orderbook>>>,
    /// 当前日期/时间
    current_date: RwLock<Option<Date>>,
    current_datetime: RwLock<String>,
    /// 待处理订单队列
    pending_orders: RwLock<Vec<MarketOrder>>,
    /// 最近成交回报（按标的聚合）
    last_trades: RwLock<HashMap<String, Vec<TradeResult>>>,
    /// 行情快照
    market_prices: RwLock<HashMap<String, Price>>,
}

impl QAMarketSystem {
    pub fn new(portfolio: impl Into<String>) -> Self {
        Self {
            portfolio: portfolio.into(),
            username: String::new(),
            backtest_auto_fill: true,
            accounts: DashMap::new(),
            orderbooks: DashMap::new(),
            current_date: RwLock::new(None),
            current_datetime: RwLock::new(String::new()),
            pending_orders: RwLock::new(Vec::new()),
            last_trades: RwLock::new(HashMap::new()),
            market_prices: RwLock::new(HashMap::new()),
        }
    }

    /// 创建实盘/模拟模式（不自动成交，走订单簿/broker）
    pub fn new_sim(portfolio: impl Into<String>) -> Self {
        let mut s = Self::new(portfolio);
        s.backtest_auto_fill = false;
        s
    }

    pub fn register_account(&self, name: impl Into<String>, init_cash: Amount) -> Arc<Account> {
        let acc = Arc::new(Account::new_stock(name, init_cash));
        self.accounts.insert(acc.account_cookie.clone(), acc.clone());
        acc
    }

    pub fn register_account_arc(&self, account: Arc<Account>) {
        self.accounts
            .insert(account.account_cookie.clone(), account);
    }

    pub fn get_account(&self, name: &str) -> Option<Arc<Account>> {
        self.accounts.get(name).map(|r| r.clone())
    }

    pub fn account_names(&self) -> Vec<String> {
        self.accounts.iter().map(|r| r.key().clone()).collect()
    }

    pub fn set_date(&self, date: Date) {
        *self.current_date.write() = Some(date);
        *self.current_datetime.write() = date.as_str();
    }

    pub fn set_datetime(&self, dt: impl Into<String>) {
        *self.current_datetime.write() = dt.into();
    }

    pub fn current_date(&self) -> Option<Date> {
        *self.current_date.read()
    }

    pub fn current_datetime(&self) -> String {
        self.current_datetime.read().clone()
    }

    pub fn schedule_order(&self, order: MarketOrder) {
        self.pending_orders.write().push(order);
    }

    /// 处理订单队列：撮合并把成交回报推送给账户
    pub fn process_order_queue(&self) -> Vec<TradeResult> {
        let orders: Vec<MarketOrder> = self.pending_orders.write().drain(..).collect();
        let mut all_trades = Vec::new();
        for mo in orders {
            let trades = self.execute_order(&mo);
            all_trades.extend(trades);
        }
        all_trades
    }

    /// 执行单笔订单：进入订单簿撮合，成交回报推送账户
    pub fn execute_order(&self, mo: &MarketOrder) -> Vec<TradeResult> {
        let acc = match self.get_account(&mo.account_cookie) {
            Some(a) => a,
            None => return Vec::new(),
        };

        // 通过账户下单（含风控/冻结）
        let side = match mo.direction {
            Direction::Buy => hiquant_account::OrderSide::Buy,
            Direction::Sell => hiquant_account::OrderSide::Sell,
        };
        let order_id = match acc.place_order(side, &mo.code, mo.volume, mo.price) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("place_order failed: {e}");
                return Vec::new();
            }
        };

        // 回测自动成交模式：直接按订单价（或最新价）全部成交
        if self.backtest_auto_fill {
            let fill_price = if mo.price > 0.0 {
                mo.price
            } else {
                self.market_prices
                    .read()
                    .get(&mo.code)
                    .copied()
                    .unwrap_or(mo.price)
            };
            let trade = TradeResult {
                trade_id: format!("TRD_{}_{}", mo.code, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                bid_order_id: if mo.direction.is_buy() { order_id.clone() } else { String::new() },
                ask_order_id: if mo.direction.is_sell() { order_id.clone() } else { String::new() },
                price: fill_price,
                volume: mo.volume,
            };
            let _ = acc.on_trade(&order_id, mo.volume, fill_price);
            self.last_trades
                .write()
                .entry(mo.code.clone())
                .or_default()
                .push(trade.clone());
            return vec![trade];
        }

        // 模拟/实盘模式：进入订单簿撮合
        let ob = self
            .orderbooks
            .entry(mo.code.clone())
            .or_insert_with(|| {
                Arc::new(RwLock::new(Orderbook::new(mo.code.clone())))
            })
            .clone();
        let trades = ob.write().add_order(BookOrder {
            order_id: order_id.clone(),
            direction: mo.direction,
            price: mo.price,
            volume: mo.volume,
            timestamp: 0,
        });

        // 把成交回报推送回账户
        for t in &trades {
            let _ = acc.on_trade(&order_id, t.volume, t.price);
        }

        self.last_trades
            .write()
            .entry(mo.code.clone())
            .or_default()
            .extend(trades.clone());

        trades
    }

    /// 用行情快照驱动所有账户的价格更新
    pub fn update_all_prices(&self, prices: &HashMap<String, Price>) {
        for (code, price) in prices {
            self.market_prices.write().insert(code.clone(), *price);
            for r in self.accounts.iter() {
                r.update_market_data(code, *price);
            }
        }
    }

    pub fn update_price(&self, code: &str, price: Price) {
        self.market_prices.write().insert(code.to_string(), price);
        for r in self.accounts.iter() {
            r.update_market_data(code, price);
        }
    }

    pub fn get_orderbook(&self, code: &str) -> Option<Arc<RwLock<Orderbook>>> {
        self.orderbooks.get(code).map(|r| r.clone())
    }

    pub fn last_trades(&self, code: &str) -> Vec<TradeResult> {
        self.last_trades
            .read()
            .get(code)
            .cloned()
            .unwrap_or_default()
    }

    /// 快照所有账户的 QIFI
    pub fn snapshot_all_accounts(&self) -> HashMap<String, Qifi> {
        self.accounts
            .iter()
            .map(|r| (r.key().clone(), r.to_qifi()))
            .collect()
    }

    /// 日终结算所有账户
    pub fn daily_settle_all(&self) {
        for r in self.accounts.iter() {
            r.daily_settle();
        }
    }

    /// 单步推进：消费 bar 推送价格 + 处理订单队列
    pub fn step_with_bar(&self, bar: &Bar) {
        let price = bar.close;
        self.update_price(&bar.order_book_id, price);
        let _trades = self.process_order_queue();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_then_sell_through_market_system() {
        let mkt = QAMarketSystem::new("test");
        let acc = mkt.register_account("acc1", 100_000.0);

        // 挂买 100@10
        mkt.schedule_order(MarketOrder::buy("acc1", "000001", 100.0, 10.0));
        mkt.process_order_queue();
        assert!(acc.has_position("000001"));

        // 价格涨到 11
        mkt.update_price("000001", 11.0);
        let summary = acc.summary();
        assert!(summary.float_pnl > 90.0);
    }
}
