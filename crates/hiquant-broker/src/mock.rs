//! MockBroker：内存模拟经纪商
//!
//! 用于在本地无 miniqmt 环境下做实盘流程联调。
//! 维护账户资金、持仓、订单簿，按限价/市价立即成交。

use crate::broker::{
    BrokerAccount, BrokerOrder, BrokerPosition, BrokerQuote, Broker, OrderResponse,
};
use async_trait::async_trait;
use parking_lot::RwLock;
use hiquant_core::{Amount, Direction, Price, Result, Volume};
use hiquant_protocol::qifi::Trade;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    pub account_id: String,
    pub init_cash: Amount,
    /// 最新价表（code -> price），用于撮合与估值
    #[serde(default)]
    pub quotes: HashMap<String, Price>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            account_id: "mock".into(),
            init_cash: 1_000_000.0,
            quotes: HashMap::new(),
        }
    }
}

#[derive(Debug, Default)]
struct PositionInner {
    volume_long: Volume,
    volume_short: Volume,
    price_long: Price,
    price_short: Price,
}

struct MockInner {
    cash: Amount,
    frozen: Amount,
    positions: HashMap<String, PositionInner>,
    quotes: HashMap<String, Price>,
    trades: Vec<Trade>,
    seq: AtomicU64,
}

pub struct MockBroker {
    config: MockConfig,
    inner: Arc<RwLock<MockInner>>,
}

impl MockBroker {
    pub fn new(config: MockConfig) -> Self {
        let quotes = config.quotes.clone();
        let init_cash = config.init_cash;
        Self {
            config,
            inner: Arc::new(RwLock::new(MockInner {
                cash: init_cash,
                frozen: 0.0,
                positions: HashMap::new(),
                quotes,
                trades: Vec::new(),
                seq: AtomicU64::new(0),
            })),
        }
    }

    /// 设置某标的最新价
    pub fn set_price(&self, code: &str, price: Price) {
        self.inner.write().quotes.insert(code.to_string(), price);
    }
}

#[async_trait]
impl Broker for MockBroker {
    fn name(&self) -> &str {
        "mock"
    }

    async fn place_order(&self, order: BrokerOrder) -> Result<OrderResponse> {
        let mut inner = self.inner.write();
        let price = if order.price > 0.0 {
            order.price
        } else {
            inner
                .quotes
                .get(&order.code)
                .copied()
                .ok_or_else(|| {
                    hiquant_core::HiquantError::Broker(format!(
                        "no quote for {} cannot fill market order",
                        order.code
                    ))
                })?
        };

        let seq = inner.seq.fetch_add(1, Ordering::Relaxed);
        let broker_order_id = format!("MKT_{}", seq);
        let trade_id = format!("TRD_{}", seq);
        let now = chrono::Utc::now().to_rfc3339();

        // 先校验卖出是否有足够持仓
        if order.direction == Direction::Sell {
            let avail = inner
                .positions
                .get(&order.code)
                .map(|p| p.volume_long)
                .unwrap_or(0.0);
            if avail + 1e-9 < order.volume {
                return Ok(OrderResponse {
                    client_order_id: order.client_order_id,
                    broker_order_id: String::new(),
                    accepted: false,
                    message: "insufficient long position".into(),
                });
            }
        }

        // 计算资金变化
        let amount = price * order.volume;
        let cash_delta = match order.direction {
            Direction::Buy => -amount,
            Direction::Sell => amount,
        };

        // 更新持仓
        let pos = inner.positions.entry(order.code.clone()).or_default();
        match order.direction {
            Direction::Buy => {
                pos.volume_long += order.volume;
                if pos.price_long <= 0.0 {
                    pos.price_long = price;
                } else {
                    let total = pos.price_long * (pos.volume_long - order.volume)
                        + price * order.volume;
                    pos.price_long = total / pos.volume_long;
                }
            }
            Direction::Sell => {
                pos.volume_long -= order.volume;
                if pos.volume_long < 1e-9 {
                    pos.price_long = 0.0;
                }
            }
        }
        // 结束对 positions 的借用后再改 cash
        inner.cash += cash_delta;

        inner.trades.push(Trade {
            trade_id: trade_id.clone(),
            order_id: broker_order_id.clone(),
            account_id: self.config.account_id.clone(),
            instrument_id: order.code.clone(),
            direction: order.direction.as_str().to_string(),
            price,
            volume: order.volume,
            trade_time: now,
            ..Default::default()
        });

        Ok(OrderResponse {
            client_order_id: order.client_order_id,
            broker_order_id,
            accepted: true,
            message: "filled".into(),
        })
    }

    async fn cancel_order(&self, _broker_order_id: &str) -> Result<bool> {
        // Mock 是立即成交的，没有可撤订单
        Ok(false)
    }

    async fn query_account(&self) -> Result<BrokerAccount> {
        let inner = self.inner.read();
        let mv: Amount = inner
            .positions
            .iter()
            .map(|(code, p)| {
                let px = inner.quotes.get(code).copied().unwrap_or(0.0);
                px * (p.volume_long + p.volume_short)
            })
            .sum();
        Ok(BrokerAccount {
            account_id: self.config.account_id.clone(),
            balance: inner.cash + mv,
            available: inner.cash - inner.frozen,
            margin: 0.0,
            frozen_margin: inner.frozen,
        })
    }

    async fn query_positions(&self) -> Result<Vec<BrokerPosition>> {
        let inner = self.inner.read();
        let out = inner
            .positions
            .iter()
            .filter(|p| p.1.volume_long.abs() > 1e-9 || p.1.volume_short.abs() > 1e-9)
            .map(|(code, p)| {
                let px = inner.quotes.get(code).copied().unwrap_or(0.0);
                BrokerPosition {
                    code: code.clone(),
                    volume_long: p.volume_long,
                    volume_short: p.volume_short,
                    price_long: p.price_long,
                    price_short: p.price_short,
                    market_value: px * (p.volume_long + p.volume_short),
                    float_pnl: (px - p.price_long) * p.volume_long,
                }
            })
            .collect();
        Ok(out)
    }

    async fn query_trades(&self) -> Result<Vec<Trade>> {
        Ok(self.inner.read().trades.clone())
    }

    async fn query_quote(&self, code: &str) -> Result<BrokerQuote> {
        let inner = self.inner.read();
        let px = inner.quotes.get(code).copied().ok_or_else(|| {
            hiquant_core::HiquantError::Broker(format!("no quote for {code}"))
        })?;
        Ok(BrokerQuote {
            code: code.to_string(),
            last_price: px,
            bid_price: px,
            ask_price: px,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn is_connected(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_buy_sell() {
        let mut quotes = HashMap::new();
        quotes.insert("000001".to_string(), 10.0);
        let cfg = MockConfig {
            account_id: "m".into(),
            init_cash: 100_000.0,
            quotes,
        };
        let broker = MockBroker::new(cfg);

        // 买 100@10
        let r = broker
            .place_order(BrokerOrder::buy("000001", 100.0, 10.0))
            .await
            .unwrap();
        assert!(r.accepted);

        // 价格涨到 11
        broker.set_price("000001", 11.0);
        let acc = broker.query_account().await.unwrap();
        assert!(acc.balance > 100_000.0); // 浮盈 100

        // 卖 100@11
        let r = broker
            .place_order(BrokerOrder::sell("000001", 100.0, 11.0))
            .await
            .unwrap();
        assert!(r.accepted);

        let pos = broker.query_positions().await.unwrap();
        assert!(pos.is_empty());
    }
}
