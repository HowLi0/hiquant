//! hiquant-account: 账户、订单、持仓、合约预设
//!
//! 对标 C++ hiquant 的 account/ 模块。
//! - [`Order`]/[`Position`]/[`Account`] 为业务实体（含计算逻辑）
//! - 与 [`hiquant_protocol::qifi`] 的 DTO 之间通过 From/Into 转换
//! - 使用 parking_lot::RwLock 保证账户并发安全

pub mod account;
pub mod order;
pub mod position;
pub mod preset;

pub use account::{Account, AccountSummary, AccountType};
pub use order::{Order, OrderFactory, OrderSide};
pub use position::{Position, PositionStats};
pub use preset::{CodePreset, MarketPreset};
