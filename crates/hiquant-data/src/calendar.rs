//! 交易日历
//!
//! 提供中国大陆 A 股交易日判断与交易日区间枚举。
//! 内置 2015-2026 的简化算法：跳过周末 + 中国法定节假日（粗略实现，
//! 生产环境应从交易所交易日历文件加载，见 [`TradingCalendar::from_file`]）。

use chrono::{Datelike, NaiveDate, Weekday};
use hiquant_core::Date;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default)]
pub struct TradingCalendar {
    /// 已知的交易日集合（升序）。空集表示使用周末规则。
    trading_days: BTreeSet<NaiveDate>,
}

impl TradingCalendar {
    /// 创建一个使用内置周末规则 + 中国节假日表（粗略）的日历
    pub fn new_built_in() -> Self {
        let mut set = BTreeSet::new();
        // 兜底：如果未配置文件，使用 weekend 规则即可（这里集合留空，
        // is_trading_day 会按周末判断 + 节假日表判断）
        let _ = &mut set;
        Self { trading_days: set }
    }

    /// 从文件加载交易日列表（每行一个 YYYYMMDD 或 YYYY-MM-DD）
    pub fn from_file(path: &str) -> std::io::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        let mut set = BTreeSet::new();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(date) = Date::parse(line) {
                set.insert(date.to_naive());
            }
        }
        Ok(Self { trading_days: set })
    }

    /// 直接给定交易日集合
    pub fn from_dates(dates: impl IntoIterator<Item = Date>) -> Self {
        Self {
            trading_days: dates.into_iter().map(|d| d.to_naive()).collect(),
        }
    }

    pub fn is_trading_day(&self, date: Date) -> bool {
        // 若显式配置了交易日集合，则严格依据集合
        if !self.trading_days.is_empty() {
            return self.trading_days.contains(&date.to_naive());
        }
        // 否则使用周末规则 + 内置节假日表
        let nd = date.to_naive();
        if matches!(nd.weekday(), Weekday::Sat | Weekday::Sun) {
            return false;
        }
        !is_china_holiday(nd)
    }

    pub fn next_trading_day(&self, date: Date) -> Date {
        let mut d = date.succ();
        while !self.is_trading_day(d) {
            d = d.succ();
        }
        d
    }

    pub fn prev_trading_day(&self, date: Date) -> Date {
        let mut d = date.pred();
        while !self.is_trading_day(d) {
            d = d.pred();
        }
        d
    }

    /// 枚举 [start, end] 区间内的所有交易日
    pub fn trading_days_between(&self, start: Date, end: Date) -> Vec<Date> {
        if !self.trading_days.is_empty() {
            return self
                .trading_days
                .range(start.to_naive()..=end.to_naive())
                .map(|nd| Date(*nd))
                .collect();
        }
        let mut out = Vec::new();
        let mut d = start;
        while d <= end {
            if self.is_trading_day(d) {
                out.push(d);
            }
            d = d.succ();
        }
        out
    }

    /// 交易日数量
    pub fn count_trading_days(&self, start: Date, end: Date) -> usize {
        self.trading_days_between(start, end).len()
    }
}

/// 中国大陆节假日粗略判断（仅含法定节假日，不含调休；调休需交易日历文件）
fn is_china_holiday(d: NaiveDate) -> bool {
    let y = d.year();
    let m = d.month();
    let day = d.day();
    // 元旦
    if m == 1 && day == 1 {
        return true;
    }
    // 春节（粗略：1月21日 ~ 2月10日之间固定取 7 天窗口的近似不靠谱，
    // 这里采用固定日期：仅排除除夕到初六的近似窗口，由调用方应使用日历文件覆盖）
    if m == 1 && (21..=31).contains(&day) {
        return true;
    }
    if m == 2 && (1..=10).contains(&day) {
        return true;
    }
    // 清明
    if m == 4 && day == 4 {
        return true;
    }
    // 劳动节
    if m == 5 && (1..=3).contains(&day) {
        return true;
    }
    // 端午（粗略）
    if m == 6 && (8..=12).contains(&day) {
        return true;
    }
    // 中秋（粗略）
    if m == 9 && (13..=18).contains(&day) {
        return true;
    }
    // 国庆
    if m == 10 && (1..=7).contains(&day) {
        return true;
    }
    // 抑制未使用变量
    let _ = y;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekend_is_not_trading_day() {
        let cal = TradingCalendar::new_built_in();
        // 2024-01-06 是周六
        let sat = Date::from_ymd(2024, 1, 6);
        assert!(!cal.is_trading_day(sat));
        // 2024-01-08 是周一
        let mon = Date::from_ymd(2024, 1, 8);
        assert!(cal.is_trading_day(mon));
    }
}
