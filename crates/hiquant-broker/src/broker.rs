//! Broker trait：交易接口抽象
//!
//! 抽象实盘/模拟交易接口，便于在 [`MockBroker`] 与 [`crate::miniqmt::MiniQmtBroker`] 间切换。
//! 回测不经过 Broker，直接走 [`hiquant_market::QAMarketSystem`] 的撮合。

use async_trait::async_trait;
use hiquant_core::{Amount, Direction, Price, Result, Volume};
use hiquant_protocol::qifi::Trade;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 经纪商订单请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrder {
    pub client_order_id: String,
    pub code: String,
    pub direction: Direction,
    pub volume: Volume,
    pub price: Price,
    /// "limit" / "market"
    pub price_type: String,
}

impl BrokerOrder {
    pub fn buy(code: &str, volume: Volume, price: Price) -> Self {
        Self {
            client_order_id: format!("C_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            code: code.to_string(),
            direction: Direction::Buy,
            volume,
            price,
            price_type: if price > 0.0 { "limit" } else { "market" }.to_string(),
        }
    }

    pub fn sell(code: &str, volume: Volume, price: Price) -> Self {
        Self {
            client_order_id: format!("C_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            code: code.to_string(),
            direction: Direction::Sell,
            volume,
            price,
            price_type: if price > 0.0 { "limit" } else { "market" }.to_string(),
        }
    }
}

/// 持仓快照（来自经纪商）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrokerPosition {
    pub code: String,
    pub volume_long: Volume,
    pub volume_short: Volume,
    pub price_long: Price,
    pub price_short: Price,
    pub market_value: Amount,
    pub float_pnl: Amount,
}

/// 资金快照（来自经纪商）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrokerAccount {
    pub account_id: String,
    pub balance: Amount,
    pub available: Amount,
    pub margin: Amount,
    pub frozen_margin: Amount,
}

/// 行情快照
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrokerQuote {
    pub code: String,
    pub last_price: Price,
    pub bid_price: Price,
    pub ask_price: Price,
    pub timestamp: String,
}

/// 订单回报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub client_order_id: String,
    pub broker_order_id: String,
    pub accepted: bool,
    pub message: String,
}

/// Broker trait：所有经纪商实现此接口
#[async_trait]
pub trait Broker: Send + Sync {
    fn name(&self) -> &str;

    /// 下单
    async fn place_order(&self, order: BrokerOrder) -> Result<OrderResponse>;

    /// 撤单
    async fn cancel_order(&self, broker_order_id: &str) -> Result<bool>;

    /// 查询账户资金
    async fn query_account(&self) -> Result<BrokerAccount>;

    /// 查询持仓
    async fn query_positions(&self) -> Result<Vec<BrokerPosition>>;

    /// 查询当日成交
    async fn query_trades(&self) -> Result<Vec<Trade>>;

    /// 查询最新行情快照
    async fn query_quote(&self, code: &str) -> Result<BrokerQuote>;

    /// 是否处于连接状态
    async fn is_connected(&self) -> bool;
}

/// 经纪商工厂：从配置构造一个 Broker 实例
pub trait BrokerFactory: Send + Sync {
    fn name(&self) -> &'static str;
    fn build(&self, params: &serde_json::Value) -> Box<dyn Broker>;
}

/// 用于把 BrokerOrder 等参数序列化的辅助 map
pub fn params_to_map(params: &serde_json::Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            out.insert(k.clone(), s);
        }
    }
    out
}
