//! hiquant-data: 行情数据类型、数据源 trait、交易日历、示例数据生成
//!
//! 对标 C++ hiquant 的 data/ 模块。统一使用 [`Bar`] 作为业务实体 K 线类型
//! （消除 C++ 中 data::Kline 与 kline::Kline 的分裂），
//! 与 [`hiquant_protocol::mifi::Kline`]（DTO）通过 From/Into 转换。

pub mod bar;
pub mod calendar;
pub mod generator;
pub mod source;
pub mod tick;

pub use bar::{Bar, BarCollection};
pub use calendar::TradingCalendar;
pub use generator::SampleDataGenerator;
pub use source::{
    CsvSource, DataSource, DataSourceContext, HttpSource, QueryRange, StubSource,
};
pub use tick::Tick;
