//! 双均线交叉策略（SMA Cross）
//!
//! 经典入门策略：短均线上穿长均线买入，下穿卖出。
//! 用于演示策略框架与回测引擎的端到端流程。

use crate::strategy::{Context, Strategy};
use hiquant_core::Date;
use hiquant_data::Bar;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmaParams {
    /// 标的代码
    pub code: String,
    /// 短均线周期
    pub fast: usize,
    /// 长均线周期
    pub slow: usize,
    /// 每次开仓的手数（股数）
    pub lots: f64,
}

impl Default for SmaParams {
    fn default() -> Self {
        Self {
            code: "000001".into(),
            fast: 5,
            slow: 20,
            lots: 100.0,
        }
    }
}

/// 双均线交叉策略
pub struct SmaCrossStrategy {
    params: SmaParams,
    closes: Vec<f64>,
    position: f64,
}

impl SmaCrossStrategy {
    pub fn new(params: SmaParams) -> Self {
        Self {
            params,
            closes: Vec::new(),
            position: 0.0,
        }
    }

    fn sma(&self, period: usize) -> Option<f64> {
        if self.closes.len() < period {
            return None;
        }
        let start = self.closes.len() - period;
        let sum: f64 = self.closes[start..].iter().sum();
        Some(sum / period as f64)
    }
}

impl Strategy for SmaCrossStrategy {
    fn name(&self) -> &str {
        "sma_cross"
    }

    fn on_bar(&mut self, ctx: &mut Context, bar: &Bar) {
        if bar.order_book_id != self.params.code {
            return;
        }
        self.closes.push(bar.close);

        let fast = self.sma(self.params.fast);
        let slow = self.sma(self.params.slow);

        if let (Some(f), Some(s)) = (fast, slow) {
            ctx.record("sma_fast", f);
            ctx.record("sma_slow", s);

            if f > s && self.position < 1e-9 {
                // 金叉买入
                ctx.buy(&self.params.code, self.params.lots, bar.close);
                self.position = self.params.lots;
                ctx.record("signal", 1.0);
            } else if f < s && self.position > 1e-9 {
                // 死叉卖出
                ctx.sell(&self.params.code, self.position, bar.close);
                self.position = 0.0;
                ctx.record("signal", -1.0);
            } else {
                ctx.record("signal", 0.0);
            }
        }
    }

    fn on_day_close(&mut self, ctx: &mut Context, _date: Date) {
        // 记录收盘价便于绘图
        if let Some(&c) = self.closes.last() {
            ctx.record("close", c);
        }
    }

    fn instruments(&self) -> Vec<String> {
        vec![self.params.code.clone()]
    }
}
