//! 业务 Tick 类型

use hiquant_core::{Amount, Price, Volume};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tick {
    pub instrument_id: String,
    #[serde(default)]
    pub exchange_id: String,
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
    pub bid_prices: Vec<Price>,
    #[serde(default)]
    pub bid_volumes: Vec<Volume>,
    #[serde(default)]
    pub ask_prices: Vec<Price>,
    #[serde(default)]
    pub ask_volumes: Vec<Volume>,
    #[serde(default)]
    pub open_interest: Volume,
    #[serde(default)]
    pub limit_up: Price,
    #[serde(default)]
    pub limit_down: Price,
}

impl Tick {
    pub fn new(instrument_id: impl Into<String>, datetime: impl Into<String>, price: Price) -> Self {
        Self {
            instrument_id: instrument_id.into(),
            datetime: datetime.into(),
            last_price: price,
            ..Default::default()
        }
    }

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

    pub fn mid_price(&self) -> Option<Price> {
        match (self.bid1(), self.ask1()) {
            (Some((bid, _)), Some((ask, _))) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }
}

impl From<Tick> for hiquant_protocol::mifi::Tick {
    fn from(t: Tick) -> Self {
        hiquant_protocol::mifi::Tick {
            instrument_id: t.instrument_id,
            exchange_id: t.exchange_id,
            datetime: t.datetime,
            last_price: t.last_price,
            pre_close: t.pre_close,
            open: t.open,
            high: t.high,
            low: t.low,
            volume: t.volume,
            amount: t.amount,
            trade_count: 0,
            bid_prices: t.bid_prices,
            bid_volumes: t.bid_volumes,
            ask_prices: t.ask_prices,
            ask_volumes: t.ask_volumes,
            settle: 0.0,
            pre_settle: 0.0,
            open_interest: t.open_interest,
            limit_up: t.limit_up,
            limit_down: t.limit_down,
        }
    }
}
