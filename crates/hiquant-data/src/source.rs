//! 可插拔数据源 trait 与若干实现
//!
//! 设计目标：回测使用本地数据（CSV/SQLite/Parquet）；
//! 实盘行情可对接 miniqmt/大QMT 等（在 hiquant-broker crate 中实现 broker 行情订阅）。
//! HTTP 数据源支持对接 Tushare/AkShare 等（用户自行实现具体 API 适配）。

use crate::bar::Bar;
use crate::tick::Tick;
use async_trait::async_trait;
use hiquant_core::{Date, Frequency, Result};
use serde::{Deserialize, Serialize};

/// 查询区间
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRange {
    pub code: String,
    pub freq: Frequency,
    pub start: String,
    pub end: String,
}

impl QueryRange {
    pub fn new(code: impl Into<String>, freq: Frequency, start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            freq,
            start: start.into(),
            end: end.into(),
        }
    }
}

/// 异步数据源 trait
///
/// 所有方法均为异步，便于 HTTP / 进程间通信数据源实现。
#[async_trait]
pub trait DataSource: Send + Sync {
    fn name(&self) -> &str;

    /// 查询 K 线
    async fn fetch_bars(&self, range: &QueryRange) -> Result<Vec<Bar>>;

    /// 查询 Tick（可选实现）
    async fn fetch_ticks(&self, _range: &QueryRange) -> Result<Vec<Tick>> {
        Ok(Vec::new())
    }

    /// 列出可用的标的代码
    async fn list_instruments(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

/// 数据源上下文：持有运行时可动态切换的数据源实例
pub struct DataSourceContext {
    pub source: Box<dyn DataSource>,
}

impl DataSourceContext {
    pub fn new(source: Box<dyn DataSource>) -> Self {
        Self { source }
    }

    pub async fn fetch_bars(&self, range: &QueryRange) -> Result<Vec<Bar>> {
        self.source.fetch_bars(range).await
    }
}

/// 空实现数据源（开发期占位）
pub struct StubSource;

#[async_trait]
impl DataSource for StubSource {
    fn name(&self) -> &str {
        "stub"
    }

    async fn fetch_bars(&self, _range: &QueryRange) -> Result<Vec<Bar>> {
        Ok(Vec::new())
    }
}

/// CSV 数据源：从本地 CSV 文件加载
///
/// 期望列：datetime,open,high,low,close,volume[,amount,pre_close,limit_up,limit_down]
pub struct CsvSource {
    pub directory: String,
    pub freq: Frequency,
}

impl CsvSource {
    pub fn new(directory: impl Into<String>, freq: Frequency) -> Self {
        Self {
            directory: directory.into(),
            freq,
        }
    }

    fn file_for(&self, code: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.directory).join(format!("{code}.csv"))
    }
}

#[async_trait]
impl DataSource for CsvSource {
    fn name(&self) -> &str {
        "csv"
    }

    async fn fetch_bars(&self, range: &QueryRange) -> Result<Vec<Bar>> {
        let path = self.file_for(&range.code);
        if !path.exists() {
            return Err(hiquant_core::HiquantError::DataSource(format!(
                "csv not found: {}",
                path.display()
            )));
        }
        let mut reader = csv::Reader::from_path(&path).map_err(|e| {
            hiquant_core::HiquantError::DataSource(format!("csv open {}: {e}", path.display()))
        })?;
        let mut bars = Vec::new();
        for rec in reader.deserialize() {
            let row: CsvRow = rec.map_err(|e| {
                hiquant_core::HiquantError::DataSource(format!("csv parse: {e}"))
            })?;
            // 区间过滤（按字典序，CSV 通常已按时间排序）
            if row.datetime.as_str() < range.start.as_str() {
                continue;
            }
            if row.datetime.as_str() > range.end.as_str() {
                continue;
            }
            bars.push(Bar {
                order_book_id: range.code.clone(),
                exchange_id: String::new(),
                frequency: self.freq,
                datetime: row.datetime.clone(),
                trading_date: Date::parse(&row.datetime),
                open: row.open,
                high: row.high,
                low: row.low,
                close: row.close,
                volume: row.volume,
                amount: row.amount.unwrap_or(0.0),
                limit_up: row.limit_up.unwrap_or(0.0),
                limit_down: row.limit_down.unwrap_or(0.0),
                pre_close: row.pre_close.unwrap_or(0.0),
                open_interest: row.open_interest.unwrap_or(0.0),
                split_coefficient_to: 1.0,
                dividend_cash_before_tax: 0.0,
            });
        }
        Ok(bars)
    }

    async fn list_instruments(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.directory) {
            for entry in rd.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(stem) = name.strip_suffix(".csv") {
                        out.push(stem.to_string());
                    }
                }
            }
        }
        Ok(out)
    }
}

#[derive(Debug, serde::Deserialize)]
struct CsvRow {
    datetime: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    pre_close: Option<f64>,
    #[serde(default)]
    limit_up: Option<f64>,
    #[serde(default)]
    limit_down: Option<f64>,
    #[serde(default)]
    open_interest: Option<f64>,
}

/// HTTP 数据源：通过 HTTP API 拉取行情
///
/// 调用方需要提供 endpoint 模板与响应解析器。这里给出基础实现，
/// 具体适配 Tushare/AkShare 的代码由调用方注入 closure。
pub struct HttpSource {
    pub client: reqwest::Client,
}

impl Default for HttpSource {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpSource {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

// 注：HttpSource 的具体 fetch 实现需要业务方定义 API 协议，
// 这里只提供 client 持有与基础结构，避免提前假设 API 形态。
#[async_trait]
impl DataSource for HttpSource {
    fn name(&self) -> &str {
        "http"
    }

    async fn fetch_bars(&self, _range: &QueryRange) -> Result<Vec<Bar>> {
        Err(hiquant_core::HiquantError::DataSource(
            "HttpSource 需要业务方实现具体 API 协议".to_string(),
        ))
    }
}
