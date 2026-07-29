//! hiquant-engine: 回测引擎与策略框架
//!
//! 提供 [`Strategy`] trait、内置 [`SmaCrossStrategy`] 策略、
//! 以及 [`BacktestEngine`] 用于驱动策略回测并产出净值曲线与绩效。

pub mod backtest;
pub mod equity;
pub mod sma;
pub mod strategy;

pub use backtest::{BacktestConfig, BacktestEngine, BacktestResult};
pub use equity::{EquityPoint, Performance};
pub use sma::{SmaCrossStrategy, SmaParams};
pub use strategy::{Context, IndicatorPoint, Strategy, StrategyFactory};
