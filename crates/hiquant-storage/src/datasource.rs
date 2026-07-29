//! 把 [`MarketStore`] 包装成一个 [`DataSource`]，作为回测数据源

use crate::store::MarketStore;
use async_trait::async_trait;
use hiquant_core::Result;
use hiquant_data::{source::QueryRange, Bar, DataSource, Tick};
use std::sync::Arc;

pub struct StoreDataSource {
    pub store: Arc<MarketStore>,
}

impl StoreDataSource {
    pub fn new(store: Arc<MarketStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl DataSource for StoreDataSource {
    fn name(&self) -> &str {
        "sqlite-store"
    }

    async fn fetch_bars(&self, range: &QueryRange) -> Result<Vec<Bar>> {
        self.store
            .query_bars(&range.code, range.freq, &range.start, &range.end)
    }

    async fn fetch_ticks(&self, range: &QueryRange) -> Result<Vec<Tick>> {
        self.store
            .query_ticks(&range.code, &range.start, &range.end)
    }

    async fn list_instruments(&self) -> Result<Vec<String>> {
        self.store.list_codes()
    }
}
