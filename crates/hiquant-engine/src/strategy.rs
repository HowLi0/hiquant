//! 策略框架：Strategy trait 与回测运行上下文
//!
//! 策略通过 [`Context`] 与市场系统交互：查询持仓、下达订单、记录指标。
//! 对标 C++ hiquant 的 strategy 模块。

use hiquant_core::{Price, Volume};
use hiquant_data::Bar;
use hiquant_market::{MarketOrder, QAMarketSystem};
use serde::{Deserialize, Serialize};

/// 回测/实盘运行上下文：暴露给策略的能力子集
pub struct Context<'a> {
    pub market: &'a QAMarketSystem,
    pub account_cookie: String,
    pub current_datetime: String,
    /// 策略可写的指标快照（运行结束后由引擎收集）
    pub indicators: &'a mut Vec<IndicatorPoint>,
}

impl<'a> Context<'a> {
    /// 买入（市价/限价由 price 决定，price=0 视为市价单）
    pub fn buy(&self, code: &str, volume: Volume, price: Price) {
        let order = MarketOrder::buy(&self.account_cookie, code, volume, price);
        self.market.schedule_order(order);
    }

    /// 卖出
    pub fn sell(&self, code: &str, volume: Volume, price: Price) {
        let order = MarketOrder::sell(&self.account_cookie, code, volume, price);
        self.market.schedule_order(order);
    }

    /// 记录一个指标点（用于前端绘图）
    pub fn record(&mut self, name: impl Into<String>, value: f64) {
        self.indicators.push(IndicatorPoint {
            datetime: self.current_datetime.clone(),
            name: name.into(),
            value,
        });
    }
}

/// 指标点（策略运行时记录的任意时间序列）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorPoint {
    pub datetime: String,
    pub name: String,
    pub value: f64,
}

/// 策略 trait：事件驱动的交易逻辑
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    /// 回测/实盘开始前调用一次
    fn on_start(&mut self, _ctx: &mut Context) {}

    /// 每根 bar 到达时调用（核心入口）
    fn on_bar(&mut self, ctx: &mut Context, bar: &Bar);

    /// 每个交易日开始（仅日线及以上频率有意义的调用）
    fn on_day_open(&mut self, _ctx: &mut Context, _date: hiquant_core::Date) {}

    /// 每个交易日结束
    fn on_day_close(&mut self, _ctx: &mut Context, _date: hiquant_core::Date) {}

    /// 结束时调用一次
    fn on_stop(&mut self, _ctx: &mut Context) {}

    /// 该策略订阅的标的代码（驱动 on_bar 时只推送订阅过的标的）
    fn instruments(&self) -> Vec<String>;
}

/// 策略工厂 trait：用于从配置构造策略实例
pub trait StrategyFactory: Send + Sync {
    fn name(&self) -> &'static str;
    fn build(&self, params: &serde_json::Value) -> Box<dyn Strategy>;
}
