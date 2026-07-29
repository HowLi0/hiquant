//! hiquant-storage: SQLite 行情/账户本地存储与增量更新
//!
//! 设计目标：
//! - 个人量化的"自建数据库"：用 SQLite 单文件存储 K 线、Tick、QIFI 账户快照
//! - 支持增量更新：按 (code, freq, datetime) 主键 UPSERT，记录每个 code 的最后更新时间
//! - 作为回测数据源（实现 [`hiquant_data::DataSource`]）与账户持久化后端

pub mod store;
pub use store::{AccountSnapshot, MarketStore, StoreConfig, UpsertResult};

mod datasource;
pub use datasource::StoreDataSource;
