//! 应用状态：持有市场系统、存储、broker 与广播通道

use crate::ws::Broadcaster;
use parking_lot::RwLock;
use hiquant_broker::Broker;
use hiquant_market::QAMarketSystem;
use hiquant_storage::MarketStore;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 全局共享状态
pub struct AppState {
    pub market: Arc<QAMarketSystem>,
    pub store: Arc<MarketStore>,
    pub broker: Mutex<Option<Arc<dyn Broker>>>,
    pub broadcaster: Broadcaster,
    pub init_cash: RwLock<f64>,
}

impl AppState {
    pub fn new(market: Arc<QAMarketSystem>, store: Arc<MarketStore>, init_cash: f64) -> Self {
        Self {
            market,
            store,
            broker: Mutex::new(None),
            broadcaster: Broadcaster::new(),
            init_cash: RwLock::new(init_cash),
        }
    }

    pub async fn set_broker(&self, broker: Arc<dyn Broker>) {
        *self.broker.lock().await = Some(broker);
    }

    pub async fn broker(&self) -> Option<Arc<dyn Broker>> {
        self.broker.lock().await.clone()
    }
}
