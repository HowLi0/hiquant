//! hiquant-broker: 交易接口抽象与 miniqmt 桥接
//!
//! - [`Broker`] trait：统一的经纪商接口
//! - [`MockBroker`]：内存模拟经纪商，用于本地联调
//! - [`MiniQmtBroker`]：通过 Python sidecar 对接 miniqmt/大QMT 实盘
//!
//! 回测不经过 Broker，直接走 [`hiquant_market::QAMarketSystem`] 撮合。

pub mod broker;
pub mod miniqmt;
pub mod mock;

pub use broker::{
    params_to_map, Broker, BrokerAccount, BrokerFactory, BrokerOrder, BrokerPosition,
    BrokerQuote, OrderResponse,
};
pub use miniqmt::{MiniQmtBroker, MiniQmtConfig};
pub use mock::{MockBroker, MockConfig};
