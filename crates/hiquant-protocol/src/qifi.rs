//! QIFI: 量化投资格式接口（Quant Investment Format Interface）
//!
//! 用于账户/持仓/订单/成交的标准化描述，便于序列化与跨语言交换。
//! 与 C++ protocol/qifi.hpp 对齐。

use hiquant_core::{Amount, Price, Volume};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 成交记录
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Trade {
    pub user_id: String,
    pub trade_id: String,
    pub order_id: String,
    pub account_id: String,
    pub exchange_id: String,
    pub instrument_id: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub offset: String,
    pub price: Price,
    pub volume: Volume,
    #[serde(default)]
    pub commission: Amount,
    #[serde(default)]
    pub tax: Amount,
    #[serde(default)]
    pub trade_time: String,
}

/// 订单
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Order {
    pub order_id: String,
    pub account_cookie: String,
    pub user_cookie: String,
    pub portfolio_cookie: String,
    pub instrument_id: String,
    #[serde(default)]
    pub secu_code: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub offset: String,
    #[serde(default)]
    pub volume_orign: Volume,
    #[serde(default)]
    pub volume_left: Volume,
    #[serde(default)]
    pub volume_fill: Volume,
    #[serde(default)]
    pub price_order: Price,
    #[serde(default)]
    pub price_fill: Price,
    #[serde(default)]
    pub price_type: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub fee: Amount,
    #[serde(default)]
    pub tax: Amount,
    #[serde(default)]
    pub order_time: String,
    #[serde(default)]
    pub trade_time: String,
    #[serde(default)]
    pub towards: i32,
}

/// 持仓
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Position {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub volume_long_today: Volume,
    #[serde(default)]
    pub volume_long_his: Volume,
    #[serde(default)]
    pub volume_short_today: Volume,
    #[serde(default)]
    pub volume_short_his: Volume,
    #[serde(default)]
    pub volume_long_frozen_today: Volume,
    #[serde(default)]
    pub volume_long_frozen_his: Volume,
    #[serde(default)]
    pub volume_short_frozen_today: Volume,
    #[serde(default)]
    pub volume_short_frozen_his: Volume,
    #[serde(default)]
    pub position_price_long: Price,
    #[serde(default)]
    pub position_price_short: Price,
    #[serde(default)]
    pub position_cost_long: Amount,
    #[serde(default)]
    pub position_cost_short: Amount,
    #[serde(default)]
    pub open_price_long: Price,
    #[serde(default)]
    pub open_price_short: Price,
    #[serde(default)]
    pub open_cost_long: Amount,
    #[serde(default)]
    pub open_cost_short: Amount,
    #[serde(default)]
    pub margin_long: Amount,
    #[serde(default)]
    pub margin_short: Amount,
    #[serde(default)]
    pub lastest_price: Price,
    #[serde(default)]
    pub float_pnl_long: Amount,
    #[serde(default)]
    pub float_pnl_short: Amount,
    #[serde(default)]
    pub close_pnl: Amount,
    #[serde(default)]
    pub position_profit: Amount,
    #[serde(default)]
    pub float_profit: Amount,
    #[serde(default)]
    pub lastest_datetime: String,
}

/// 资金冻结记录
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frozen {
    pub order_id: String,
    pub money: Amount,
    pub datetime: String,
    #[serde(default)]
    pub code: String,
}

/// 账户资金切片
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    #[serde(default)]
    pub pre_balance: Amount,
    #[serde(default)]
    pub deposit: Amount,
    #[serde(default)]
    pub withdraw: Amount,
    #[serde(default)]
    pub close_profit: Amount,
    #[serde(default)]
    pub commission: Amount,
    #[serde(default)]
    pub tax: Amount,
    #[serde(default)]
    pub static_balance: Amount,
    #[serde(default)]
    pub position_profit: Amount,
    #[serde(default)]
    pub float_profit: Amount,
    #[serde(default)]
    pub balance: Amount,
    #[serde(default)]
    pub margin: Amount,
    #[serde(default)]
    pub frozen_margin: Amount,
    #[serde(default)]
    pub available: Amount,
    #[serde(default)]
    pub risk_ratio: f64,
}

/// QIFI 主结构：账户/持仓/订单/成交的完整快照
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Qifi {
    pub account_cookie: String,
    #[serde(default)]
    pub portfolio: String,
    #[serde(default)]
    pub investor_name: String,
    #[serde(default)]
    pub broker_name: String,
    #[serde(default)]
    pub money: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub updatetime: String,
    #[serde(default)]
    pub trading_day: String,
    pub accounts: Account,
    #[serde(default)]
    pub positions: HashMap<String, Position>,
    #[serde(default)]
    pub orders: HashMap<String, Order>,
    #[serde(default)]
    pub trades: HashMap<String, Trade>,
    #[serde(default)]
    pub frozen: HashMap<String, Frozen>,
    #[serde(default)]
    pub banks: Vec<Bank>,
    #[serde(default)]
    pub transfers: Vec<Transfer>,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bank {
    pub bank_id: String,
    pub bank_name: String,
    #[serde(default)]
    pub balance: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Transfer {
    pub transfer_id: String,
    pub datetime: String,
    #[serde(default)]
    pub amount: Amount,
    #[serde(default)]
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event {
    pub event_id: String,
    pub datetime: String,
    pub event_type: String,
    #[serde(default)]
    pub message: String,
}

impl Qifi {
    pub fn new(account_cookie: impl Into<String>, money: f64) -> Self {
        let mut qifi = Self::default();
        qifi.account_cookie = account_cookie.into();
        qifi.money = money;
        qifi.accounts.balance = money;
        qifi.accounts.static_balance = money;
        qifi.accounts.available = money;
        qifi.source = "hiquant-rs".to_string();
        qifi.updatetime = chrono::Utc::now().to_rfc3339();
        qifi
    }

    pub fn add_position(&mut self, pos: Position) {
        self.positions.insert(pos.instrument_id.clone(), pos);
    }

    pub fn add_order(&mut self, order: Order) {
        self.orders.insert(order.order_id.clone(), order);
    }

    pub fn add_trade(&mut self, trade: Trade) {
        self.trades.insert(trade.trade_id.clone(), trade);
    }

    pub fn add_frozen(&mut self, frozen: Frozen) {
        self.frozen.insert(frozen.order_id.clone(), frozen);
    }

    /// 计算派生值：risk_ratio = margin / balance
    pub fn calculate_derived_values(&mut self) {
        self.accounts.balance = self.accounts.static_balance
            + self.accounts.deposit
            - self.accounts.withdraw
            + self.accounts.close_profit
            - self.accounts.commission
            - self.accounts.tax
            + self.accounts.position_profit;
        self.accounts.available =
            self.accounts.balance - self.accounts.margin - self.accounts.frozen_margin;
        if self.accounts.balance.abs() > 1e-9 {
            self.accounts.risk_ratio = self.accounts.margin / self.accounts.balance;
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let json = self.to_json().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        Self::from_json(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}
