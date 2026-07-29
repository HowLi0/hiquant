//! 业务 K 线实体类型 Bar
//!
//! 统一表示日线/分钟线，附带常用技术指标计算方法。
//! 字段对齐 C++ data::Kline（datatype.hpp 中的 Rust-aligned 版本）。

use hiquant_core::{Amount, Date, Frequency, Price, Volume};
use hiquant_protocol::mifi;
use serde::{Deserialize, Serialize};

/// 业务 K 线（统一表示日/分钟/周/月）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    /// 标的代码，如 "000001"
    pub order_book_id: String,
    /// 交易所代码，如 "SSE"/"SZSE"
    #[serde(default)]
    pub exchange_id: String,
    /// 频率
    pub frequency: Frequency,
    /// 行情时间：日线为 Date，分钟线为精确到分的 datetime
    pub datetime: String,
    /// 交易日（仅日/分钟线有意义）
    #[serde(default)]
    pub trading_date: Option<Date>,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Volume,
    #[serde(default)]
    pub amount: Amount,
    #[serde(default)]
    pub limit_up: Price,
    #[serde(default)]
    pub limit_down: Price,
    #[serde(default)]
    pub pre_close: Price,
    #[serde(default)]
    pub open_interest: Volume,
    #[serde(default)]
    pub split_coefficient_to: f64,
    #[serde(default)]
    pub dividend_cash_before_tax: Amount,
}

impl Bar {
    pub fn new(order_book_id: impl Into<String>, freq: Frequency, datetime: impl Into<String>) -> Self {
        Self {
            order_book_id: order_book_id.into(),
            exchange_id: String::new(),
            frequency: freq,
            datetime: datetime.into(),
            trading_date: None,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
            amount: 0.0,
            limit_up: 0.0,
            limit_down: 0.0,
            pre_close: 0.0,
            open_interest: 0.0,
            split_coefficient_to: 1.0,
            dividend_cash_before_tax: 0.0,
        }
    }

    pub fn change_percent(&self) -> f64 {
        if self.pre_close.abs() > 1e-9 {
            (self.close - self.pre_close) / self.pre_close * 100.0
        } else {
            0.0
        }
    }

    pub fn change_amount(&self) -> Price {
        self.close - self.pre_close
    }

    pub fn is_limit_up(&self) -> bool {
        self.limit_up > 0.0 && (self.close - self.limit_up).abs() < 1e-6
    }

    pub fn is_limit_down(&self) -> bool {
        self.limit_down > 0.0 && (self.close - self.limit_down).abs() < 1e-6
    }

    pub fn typical_price(&self) -> Price {
        (self.high + self.low + self.close) / 3.0
    }

    pub fn body_size(&self) -> Price {
        (self.close - self.open).abs()
    }

    pub fn upper_shadow(&self) -> Price {
        self.high - self.close.max(self.open)
    }

    pub fn lower_shadow(&self) -> Price {
        self.close.min(self.open) - self.low
    }

    pub fn range(&self) -> Price {
        self.high - self.low
    }

    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// 换手率（需要流通股本，单位为手/股）
    pub fn get_turnover_rate(&self, float_share: Volume) -> f64 {
        if float_share.abs() > 1e-9 {
            self.volume / float_share
        } else {
            0.0
        }
    }
}

impl From<Bar> for mifi::Kline {
    fn from(b: Bar) -> Self {
        mifi::Kline {
            instrument_id: b.order_book_id,
            exchange_id: b.exchange_id,
            datetime: b.datetime,
            trading_date: b.trading_date.map(|d| d.as_str()).unwrap_or_default(),
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            pre_close: b.pre_close,
            settle_price: 0.0,
            pre_settle: 0.0,
            limit_up: b.limit_up,
            limit_down: b.limit_down,
            volume: b.volume,
            amount: b.amount,
            open_interest: b.open_interest,
            pre_open_interest: 0.0,
            trade_count: 0,
            avg_price: if b.volume > 1e-9 {
                b.amount / b.volume
            } else {
                b.close
            },
        }
    }
}

impl From<mifi::Kline> for Bar {
    fn from(k: mifi::Kline) -> Self {
        Self {
            order_book_id: k.instrument_id,
            exchange_id: k.exchange_id,
            frequency: Frequency::Day,
            datetime: k.datetime,
            trading_date: Date::parse(&k.trading_date),
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            volume: k.volume,
            amount: k.amount,
            limit_up: k.limit_up,
            limit_down: k.limit_down,
            pre_close: k.pre_close,
            open_interest: k.open_interest,
            split_coefficient_to: 1.0,
            dividend_cash_before_tax: 0.0,
        }
    }
}

/// K 线集合：辅助统计与序列访问
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BarCollection {
    pub bars: Vec<Bar>,
}

impl BarCollection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, bar: Bar) {
        self.bars.push(bar);
    }

    pub fn add_batch(&mut self, mut batch: Vec<Bar>) {
        self.bars.append(&mut batch);
    }

    pub fn size(&self) -> usize {
        self.bars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    pub fn last(&self) -> Option<&Bar> {
        self.bars.last()
    }

    pub fn get_range(&self, start: usize, end: usize) -> &[Bar] {
        let end = end.min(self.bars.len());
        let start = start.min(end);
        &self.bars[start..end]
    }

    pub fn closes(&self) -> Vec<Price> {
        self.bars.iter().map(|b| b.close).collect()
    }

    pub fn highs(&self) -> Vec<Price> {
        self.bars.iter().map(|b| b.high).collect()
    }

    pub fn lows(&self) -> Vec<Price> {
        self.bars.iter().map(|b| b.low).collect()
    }

    pub fn volumes(&self) -> Vec<Volume> {
        self.bars.iter().map(|b| b.volume).collect()
    }

    pub fn max_price(&self) -> Option<Price> {
        self.bars.iter().map(|b| b.high).fold(None, |acc, x| {
            Some(acc.map_or(x, |a: Price| a.max(x)))
        })
    }

    pub fn min_price(&self) -> Option<Price> {
        self.bars.iter().map(|b| b.low).fold(None, |acc, x| {
            Some(acc.map_or(x, |a: Price| a.min(x)))
        })
    }

    pub fn avg_price(&self) -> Price {
        if self.bars.is_empty() {
            return 0.0;
        }
        self.bars.iter().map(|b| b.close).sum::<Price>() / self.bars.len() as Price
    }

    pub fn total_volume(&self) -> Volume {
        self.bars.iter().map(|b| b.volume).sum()
    }

    pub fn sort_by_time(&mut self) {
        self.bars.sort_by(|a, b| a.datetime.cmp(&b.datetime));
    }

    /// 简单移动平均
    pub fn sma(&self, period: usize) -> Vec<Option<Price>> {
        let closes = self.closes();
        let n = closes.len();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            if i + 1 < period {
                out.push(None);
            } else {
                let sum: Price = closes[i + 1 - period..=i].iter().sum();
                out.push(Some(sum / period as Price));
            }
        }
        out
    }
}

impl std::ops::Index<usize> for BarCollection {
    type Output = Bar;
    fn index(&self, index: usize) -> &Self::Output {
        &self.bars[index]
    }
}
