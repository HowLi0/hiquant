//! MIFI: 市场信息格式接口（Market Information Format Interface）
//!
//! 描述 K 线、Tick、合约信息等行情数据结构。

use hiquant_core::{Amount, Price, Volume};
use serde::{Deserialize, Serialize};

/// K 线（市场信息格式）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Kline {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub datetime: String,
    #[serde(default)]
    pub trading_date: String,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    #[serde(default)]
    pub pre_close: Price,
    #[serde(default)]
    pub settle_price: Price,
    #[serde(default)]
    pub pre_settle: Price,
    #[serde(default)]
    pub limit_up: Price,
    #[serde(default)]
    pub limit_down: Price,
    pub volume: Volume,
    #[serde(default)]
    pub amount: Amount,
    #[serde(default)]
    pub open_interest: Volume,
    #[serde(default)]
    pub pre_open_interest: Volume,
    #[serde(default)]
    pub trade_count: i64,
    #[serde(default)]
    pub avg_price: Price,
}

impl Kline {
    pub fn change_percent(&self) -> f64 {
        if self.pre_close.abs() > 1e-9 {
            (self.close - self.pre_close) / self.pre_close * 100.0
        } else {
            0.0
        }
    }

    pub fn amplitude(&self) -> f64 {
        if self.pre_close.abs() > 1e-9 {
            (self.high - self.low) / self.pre_close * 100.0
        } else {
            0.0
        }
    }

    pub fn is_limit_up(&self) -> bool {
        self.limit_up > 0.0 && (self.close - self.limit_up).abs() < 1e-6
    }

    pub fn is_limit_down(&self) -> bool {
        self.limit_down > 0.0 && (self.close - self.limit_down).abs() < 1e-6
    }
}

/// Tick：盘口快照，含 10 档买卖盘
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tick {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub datetime: String,
    pub last_price: Price,
    #[serde(default)]
    pub pre_close: Price,
    #[serde(default)]
    pub open: Price,
    #[serde(default)]
    pub high: Price,
    #[serde(default)]
    pub low: Price,
    pub volume: Volume,
    #[serde(default)]
    pub amount: Amount,
    #[serde(default)]
    pub trade_count: i64,
    #[serde(default)]
    pub bid_prices: Vec<Price>,
    #[serde(default)]
    pub bid_volumes: Vec<Volume>,
    #[serde(default)]
    pub ask_prices: Vec<Price>,
    #[serde(default)]
    pub ask_volumes: Vec<Volume>,
    #[serde(default)]
    pub settle: Price,
    #[serde(default)]
    pub pre_settle: Price,
    #[serde(default)]
    pub open_interest: Volume,
    #[serde(default)]
    pub limit_up: Price,
    #[serde(default)]
    pub limit_down: Price,
}

impl Tick {
    pub fn bid1(&self) -> Option<(Price, Volume)> {
        self.bid_prices
            .first()
            .copied()
            .zip(self.bid_volumes.first().copied())
    }

    pub fn ask1(&self) -> Option<(Price, Volume)> {
        self.ask_prices
            .first()
            .copied()
            .zip(self.ask_volumes.first().copied())
    }

    pub fn spread(&self) -> Option<Price> {
        match (self.bid1(), self.ask1()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Price> {
        match (self.bid1(), self.ask1()) {
            (Some((bid, _)), Some((ask, _))) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }
}

/// 合约信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstrumentInfo {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub instrument_name: String,
    #[serde(default)]
    pub market_type: String,
    pub price_tick: Price,
    #[serde(default)]
    pub lot_size: Volume,
    #[serde(default)]
    pub multiplier: Volume,
    #[serde(default)]
    pub margin_rate: f64,
    #[serde(default)]
    pub commission_rate: f64,
    #[serde(default)]
    pub limit_up_rate: f64,
    #[serde(default)]
    pub limit_down_rate: f64,
    #[serde(default)]
    pub list_date: String,
    #[serde(default)]
    pub expire_date: String,
}

/// 逐笔成交
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Transaction {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub datetime: String,
    pub price: Price,
    pub volume: Volume,
    #[serde(default)]
    pub direction: i32,
    #[serde(default)]
    pub trade_type: i32,
}

/// 委托队列（盘口某价位）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PriceLevel {
    pub price: Price,
    pub volume: Volume,
    #[serde(default)]
    pub order_count: i32,
}

/// 市场状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketStatus {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
    #[serde(default)]
    pub trading_status: String,
    #[serde(default)]
    pub datetime: String,
}
