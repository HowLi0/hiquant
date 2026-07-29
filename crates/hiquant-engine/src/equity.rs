//! 净值曲线与回测绩效统计

use hiquant_core::Amount;
use serde::{Deserialize, Serialize};

/// 净值曲线上的一个点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub date: String,
    pub datetime: String,
    pub cash: Amount,
    pub market_value: Amount,
    pub total_value: Amount,
    pub float_pnl: Amount,
    pub close_pnl: Amount,
}

/// 回测绩效汇总
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Performance {
    pub init_cash: Amount,
    pub final_value: Amount,
    pub total_return: f64,
    /// 年化收益率
    pub annual_return: f64,
    /// 最大回撤
    pub max_drawdown: f64,
    /// 夏普比率（按日频、无风险利率 0 估算）
    pub sharpe: f64,
    pub trading_days: usize,
    pub trade_count: usize,
}

impl Performance {
    /// 由净值序列计算绩效
    pub fn from_equity(init_cash: Amount, equity: &[EquityPoint]) -> Self {
        let final_value = equity.last().map(|p| p.total_value).unwrap_or(init_cash);
        let total_return = if init_cash.abs() > 1e-9 {
            (final_value - init_cash) / init_cash
        } else {
            0.0
        };
        let trading_days = equity.len();

        // 最大回撤
        let mut peak: f64 = init_cash;
        let mut max_dd: f64 = 0.0;
        let mut daily_returns: Vec<f64> = Vec::with_capacity(equity.len());
        let mut prev: f64 = init_cash;
        for p in equity {
            peak = peak.max(p.total_value);
            if peak > 1e-9 {
                let dd = (peak - p.total_value) / peak;
                max_dd = max_dd.max(dd);
            }
            if prev > 1e-9 {
                daily_returns.push((p.total_value - prev) / prev);
            }
            prev = p.total_value;
        }

        // 夏普比率（日频，年化 = sharpe * sqrt(252)）
        let sharpe = if !daily_returns.is_empty() {
            let mean = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
            let var = daily_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
                / daily_returns.len() as f64;
            let std = var.sqrt();
            if std > 1e-9 {
                (mean / std) * (252.0_f64).sqrt()
            } else {
                0.0
            }
        } else {
            0.0
        };

        // 年化收益率
        let annual_return = if trading_days > 0 && init_cash.abs() > 1e-9 {
            let years = trading_days as f64 / 252.0;
            if years > 1e-9 {
                (final_value / init_cash).powf(1.0 / years) - 1.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        Self {
            init_cash,
            final_value,
            total_return,
            annual_return,
            max_drawdown: max_dd,
            sharpe,
            trading_days,
            trade_count: 0,
        }
    }
}
