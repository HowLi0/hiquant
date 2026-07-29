//! 示例行情数据生成器
//!
//! 使用几何布朗运动 + 趋势/波动分量生成 OHLCV，用于无外部数据时的回测演示。

use crate::bar::Bar;
use hiquant_core::{Date, Frequency};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub struct SampleDataGenerator {
    rng: StdRng,
}

impl SampleDataGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// 生成日 K 线序列
    ///
    /// - `code`: 标的代码
    /// - `start`: 起始日期
    /// - `n`: 生成多少根
    /// - `init_price`: 起始价
    /// - `annual_drift`: 年化漂移（如 0.05 = 5%）
    /// - `annual_vol`: 年化波动率（如 0.25 = 25%）
    /// - `with_limit`: 是否生成涨跌停（A 股 ±10%）
    pub fn gen_daily(
        &mut self,
        code: &str,
        start: Date,
        n: usize,
        init_price: f64,
        annual_drift: f64,
        annual_vol: f64,
        with_limit: bool,
    ) -> Vec<Bar> {
        let dt = 1.0 / 252.0;
        let drift = (annual_drift - 0.5 * annual_vol * annual_vol) * dt;
        let vol = annual_vol * dt.sqrt();
        let limit_rate = if with_limit { 0.10 } else { 0.0 };

        let mut bars: Vec<Bar> = Vec::with_capacity(n);
        let mut price = init_price;
        let mut date = start;
        let calendar = crate::calendar::TradingCalendar::new_built_in();

        for _ in 0..n {
            // 跳到下一个交易日
            if !bars.is_empty() {
                date = calendar.next_trading_day(date);
            } else {
                // 起点若非交易日，也跳到下一个交易日
                if !calendar.is_trading_day(date) {
                    date = calendar.next_trading_day(date);
                }
            }

            let z: f64 = self.rng.sample(rand_distr_normal());
            let ret = drift + vol * z;
            let open = price;
            let close = (open * (1.0 + ret)).max(0.01);
            let high = open.max(close) * (1.0 + self.rng.gen_range(0.0..0.01));
            let low = open.min(close) * (1.0 - self.rng.gen_range(0.0..0.01));
            let volume: f64 = self.rng.gen_range(1_000_000f64..5_000_000f64).round();
            let amount = (open + close + high + low) / 4.0 * volume;

            let (limit_up, limit_down) = if with_limit {
                (
                    (open * (1.0 + limit_rate) * 100.0).round() / 100.0,
                    (open * (1.0 - limit_rate) * 100.0).round() / 100.0,
                )
            } else {
                (0.0, 0.0)
            };

            bars.push(Bar {
                order_book_id: code.to_string(),
                exchange_id: "SIM".to_string(),
                frequency: Frequency::Day,
                datetime: date.as_str(),
                trading_date: Some(date),
                open,
                high,
                low,
                close,
                volume,
                amount,
                limit_up,
                limit_down,
                pre_close: if bars.is_empty() { 0.0 } else { bars.last().unwrap().close },
                open_interest: 0.0,
                split_coefficient_to: 1.0,
                dividend_cash_before_tax: 0.0,
            });

            price = close;
        }
        bars
    }

    /// 生成日内分钟 K 线序列（一个交易日内）
    pub fn gen_minutes(
        &mut self,
        code: &str,
        date: Date,
        init_price: f64,
        bars_per_day: usize,
        annual_vol: f64,
    ) -> Vec<Bar> {
        let dt = 1.0 / (252.0 * bars_per_day as f64);
        let vol = annual_vol * dt.sqrt();
        let mut bars: Vec<Bar> = Vec::with_capacity(bars_per_day);
        let mut price = init_price;
        // 9:30 ~ 11:30 + 13:00 ~ 15:00 共 240 分钟
        let mut minutes = gen_a_share_minutes(date);
        minutes.truncate(bars_per_day);

        for ts in minutes {
            let z: f64 = self.rng.sample(rand_distr_normal());
            let ret = vol * z;
            let open = price;
            let close = (open * (1.0 + ret)).max(0.01);
            let high = open.max(close) * (1.0 + self.rng.gen_range(0.0..0.003));
            let low = open.min(close) * (1.0 - self.rng.gen_range(0.0..0.003));
            let volume: f64 = self.rng.gen_range(1000f64..50000f64).round();
            let amount = (open + close) / 2.0 * volume;
            bars.push(Bar {
                order_book_id: code.to_string(),
                exchange_id: "SIM".to_string(),
                frequency: Frequency::Min1,
                datetime: ts,
                trading_date: Some(date),
                open,
                high,
                low,
                close,
                volume,
                amount,
                limit_up: 0.0,
                limit_down: 0.0,
                pre_close: if bars.is_empty() { init_price } else { bars.last().unwrap().close },
                open_interest: 0.0,
                split_coefficient_to: 1.0,
                dividend_cash_before_tax: 0.0,
            });
            price = close;
        }
        bars
    }
}

fn gen_a_share_minutes(date: Date) -> Vec<String> {
    let d = date.as_str();
    let mut out = Vec::with_capacity(240);
    // 上午 09:31 ~ 11:30
    for h in 9..=11 {
        let start_m = if h == 9 { 31 } else { 0 };
        let end_m = if h == 11 { 30 } else { 59 };
        for m in start_m..=end_m {
            out.push(format!("{d} {h:02}:{m:02}:00"));
        }
    }
    // 下午 13:01 ~ 15:00
    for h in 13..=15 {
        let start_m = if h == 13 { 1 } else { 0 };
        let end_m = if h == 15 { 0 } else { 59 };
        for m in start_m..=end_m {
            out.push(format!("{d} {h:02}:{m:02}:00"));
        }
    }
    out
}

// Box<dyn Distribution<f64>> 包装的标准正态采样
fn rand_distr_normal() -> impl rand::distributions::Distribution<f64> {
    // 用 Box-Muller 自己实现，避免引入 rand_distr crate
    struct StdNormal;
    impl rand::distributions::Distribution<f64> for StdNormal {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
            let u1: f64 = rng.gen_range(1e-10..1.0);
            let u2: f64 = rng.gen_range(0.0..1.0);
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            z
        }
    }
    StdNormal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_daily_smoke() {
        let mut g = SampleDataGenerator::new(42);
        let bars = g.gen_daily("000001", Date::from_ymd(2024, 1, 2), 10, 10.0, 0.05, 0.25, true);
        assert_eq!(bars.len(), 10);
        for b in &bars {
            assert!(b.high >= b.low);
            assert!(b.high >= b.open);
            assert!(b.high >= b.close);
            assert!(b.low <= b.open);
            assert!(b.low <= b.close);
        }
    }
}
