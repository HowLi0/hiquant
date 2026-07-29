//! hiquant-core: 量化交易核心类型与基础抽象
//!
//! 提供类型别名、方向/订单状态等枚举、日期类型、错误类型与 ID 生成器。
//! 不依赖任何业务模块，是整个 workspace 的基础。

pub mod date;
pub mod enums;
pub mod error;
pub mod ids;
pub mod types;

pub use date::Date;
pub use enums::{
    AccountEnvironment, Direction, MarketType, Offset, OrderStatus, PositionSide, PriceType,
};
pub use error::{HiquantError, Result};
pub use ids::{generate_order_id, generate_time_based_id, generate_uuid};
pub use types::{Amount, AssetId, Frequency, Price, Timestamp, Volume};
