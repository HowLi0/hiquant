//! TIFI: 交易信息格式接口（Trading Information Format Interface）
//!
//! 描述交易侧的订单/成交/持仓/账户/风险指标，比 QIFI 更面向实盘交易系统。

use hiquant_core::{Amount, Price, Volume};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Order {
    pub order_id: String,
    #[serde(default)]
    pub strategy_id: String,
    #[serde(default)]
    pub product_id: String,
    #[serde(default)]
    pub account_id: String,
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub offset: String,
    #[serde(default)]
    pub price_type: String,
    #[serde(default)]
    pub time_condition: String,
    pub price: Price,
    #[serde(default)]
    pub volume_orign: Volume,
    #[serde(default)]
    pub volume_traded: Volume,
    #[serde(default)]
    pub volume_left: Volume,
    #[serde(default)]
    pub avg_price: Price,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error_code: i32,
    #[serde(default)]
    pub error_message: String,
    #[serde(default)]
    pub exchange_order_id: String,
    #[serde(default)]
    pub parent_order_id: String,
    #[serde(default)]
    pub order_time: String,
    #[serde(default)]
    pub trade_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Trade {
    pub trade_id: String,
    pub order_id: String,
    #[serde(default)]
    pub account_id: String,
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    pub price: Price,
    pub volume: Volume,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub offset: String,
    #[serde(default)]
    pub commission: Amount,
    #[serde(default)]
    pub trade_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Position {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub long_position_today: Volume,
    #[serde(default)]
    pub long_position_yesterday: Volume,
    #[serde(default)]
    pub short_position_today: Volume,
    #[serde(default)]
    pub short_position_yesterday: Volume,
    #[serde(default)]
    pub avg_price: Price,
    #[serde(default)]
    pub pre_settle: Price,
    #[serde(default)]
    pub settle: Price,
    #[serde(default)]
    pub position_pnl: Amount,
    #[serde(default)]
    pub close_pnl: Amount,
    #[serde(default)]
    pub realized_pnl: Amount,
    #[serde(default)]
    pub unrealized_pnl: Amount,
    #[serde(default)]
    pub margin: Amount,
    #[serde(default)]
    pub margin_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub account_id: String,
    #[serde(default)]
    pub total_asset: Amount,
    #[serde(default)]
    pub available_cash: Amount,
    #[serde(default)]
    pub frozen_cash: Amount,
    #[serde(default)]
    pub position_value: Amount,
    #[serde(default)]
    pub realized_pnl: Amount,
    #[serde(default)]
    pub unrealized_pnl: Amount,
    #[serde(default)]
    pub margin: Amount,
    #[serde(default)]
    pub risk_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskMetrics {
    #[serde(default)]
    pub sharpe: f64,
    #[serde(default)]
    pub sortino: f64,
    #[serde(default)]
    pub max_drawdown: f64,
    #[serde(default)]
    pub win_rate: f64,
    #[serde(default)]
    pub profit_loss_ratio: f64,
}
