//! 基础数值类型别名
//!
//! 与 C++ hiquant.hpp 对齐：Price/Volume/Amount = f64，Timestamp = chrono::DateTime。
//! 个人量化场景下 f64 精度足够；如需严格定点可在此处切换为 rust_decimal。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Price = f64;
pub type Volume = f64;
pub type Amount = f64;
pub type Timestamp = DateTime<Utc>;
/// 标的资产标识（股票/期货代码，如 "000001" / "IF2306"）。
pub type AssetId = String;

/// 行情频率
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    /// Tick 逐笔
    Tick,
    /// 1 分钟 K 线
    Min1,
    /// 5 分钟 K 线
    Min5,
    /// 15 分钟
    Min15,
    /// 60 分钟
    Min60,
    /// 日线
    Day,
    /// 周线
    Week,
    /// 月线
    Month,
}

impl Frequency {
    pub fn as_str(self) -> &'static str {
        match self {
            Frequency::Tick => "tick",
            Frequency::Min1 => "1min",
            Frequency::Min5 => "5min",
            Frequency::Min15 => "15min",
            Frequency::Min60 => "60min",
            Frequency::Day => "day",
            Frequency::Week => "week",
            Frequency::Month => "month",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "tick" => Frequency::Tick,
            "1min" | "min" | "1" => Frequency::Min1,
            "5min" => Frequency::Min5,
            "15min" => Frequency::Min15,
            "60min" | "hour" => Frequency::Min60,
            "day" | "d" => Frequency::Day,
            "week" | "w" => Frequency::Week,
            "month" | "m" => Frequency::Month,
            _ => return None,
        })
    }
}
