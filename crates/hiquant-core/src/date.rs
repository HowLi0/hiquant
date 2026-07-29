//! 日期类型：对标 chrono::NaiveDate，提供便捷序列化与交易日历工具入口

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 简单日期（不含时间），用于交易日记账与回测日期循环。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date(pub NaiveDate);

impl Date {
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Self {
        Self(NaiveDate::from_ymd_opt(year, month, day).expect("invalid date"))
    }

    pub fn today() -> Self {
        Self(chrono::Local::now().date_naive())
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        // 支持 YYYYMMDD / YYYY-MM-DD / YYYY/MM/DD
        let normalized = if s.len() == 8 && !s.contains('-') && !s.contains('/') {
            format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
        } else {
            s.replace('/', "-")
        };
        NaiveDate::parse_from_str(&normalized, "%Y-%m-%d").ok().map(Self)
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        (self.0.year(), self.0.month(), self.0.day())
    }

    pub fn year(self) -> i32 {
        self.0.year()
    }

    pub fn month(self) -> u32 {
        self.0.month()
    }

    pub fn day(self) -> u32 {
        self.0.day()
    }

    pub fn succ(self) -> Self {
        Self(self.0.succ_opt().unwrap())
    }

    pub fn pred(self) -> Self {
        Self(self.0.pred_opt().unwrap())
    }

    /// 下一日历日（不区分交易日）
    pub fn next_day(self) -> Self {
        self.succ()
    }

    pub fn as_str(&self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }

    /// YYYYMMDD 整数
    pub fn to_int(self) -> i32 {
        self.0.format("%Y%m%d").to_string().parse().unwrap_or(0)
    }

    pub fn to_naive(self) -> NaiveDate {
        self.0
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d"))
    }
}

impl FromStr for Date {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("invalid date: {s}"))
    }
}

impl Serialize for Date {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> std::result::Result<S::Ok, S::Error> {
        ser.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Date::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid date: {s}")))
    }
}
