//! 限价订单簿撮合引擎
//!
//! 价格优先 + 时间优先，BTreeMap 维护买卖盘。
//! 支持限价单与市价单，支持撤单。

use hiquant_core::{Direction, Price, Volume};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 订单簿中的一个委托
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookOrder {
    pub order_id: String,
    pub direction: Direction,
    pub price: Price,
    pub volume: Volume,
    pub timestamp: u64,
}

/// 成交结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradeResult {
    pub trade_id: String,
    pub bid_order_id: String,
    pub ask_order_id: String,
    pub price: Price,
    pub volume: Volume,
}

/// 内部未分配 trade_id 的中间成交记录
#[derive(Debug, Clone)]
struct RawTrade {
    bid_order_id: String,
    ask_order_id: String,
    price: Price,
    volume: Volume,
}

/// 单标的订单簿
pub struct Orderbook {
    pub instrument_id: String,
    /// 买盘
    bids: BTreeMap<PriceKey, Vec<BookOrder>>,
    /// 卖盘
    asks: BTreeMap<PriceKey, Vec<BookOrder>>,
    /// 订单 ID 索引（用于撤单时定位）
    order_index: std::collections::HashMap<String, (Direction, Price)>,
    /// 时间序号
    seq: u64,
    /// 成交序号
    trade_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PriceKey(i64);

impl PriceKey {
    fn from_price(p: Price) -> Self {
        Self((p * 1_000_000.0).round() as i64)
    }
    fn to_price(self) -> Price {
        self.0 as Price / 1_000_000.0
    }
}

impl Orderbook {
    pub fn new(instrument_id: impl Into<String>) -> Self {
        Self {
            instrument_id: instrument_id.into(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: std::collections::HashMap::new(),
            seq: 0,
            trade_seq: 0,
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn assign_trade_ids(&mut self, raws: Vec<RawTrade>) -> Vec<TradeResult> {
        raws
            .into_iter()
            .map(|r| {
                self.trade_seq += 1;
                TradeResult {
                    trade_id: format!("TRD_{}_{}", self.instrument_id, self.trade_seq),
                    bid_order_id: r.bid_order_id,
                    ask_order_id: r.ask_order_id,
                    price: r.price,
                    volume: r.volume,
                }
            })
            .collect()
    }

    /// 下单（price=0 视为市价单）
    pub fn add_order(&mut self, mut order: BookOrder) -> Vec<TradeResult> {
        order.timestamp = self.next_seq();
        if order.price <= 0.0 {
            let raws = self.match_market_order(order);
            self.assign_trade_ids(raws)
        } else {
            let raws = self.match_limit_order(order);
            self.assign_trade_ids(raws)
        }
    }

    fn match_market_order(&mut self, order: BookOrder) -> Vec<RawTrade> {
        let mut raws = Vec::new();
        let mut remaining = order.volume;
        let is_buy = order.direction.is_buy();
        let order_id = order.order_id.clone();

        while remaining > 1e-9 {
            let best_key = if is_buy {
                self.asks.keys().next().copied()
            } else {
                self.bids.keys().next_back().copied()
            };
            let best_key = match best_key {
                Some(k) => k,
                None => break,
            };
            let best_price = best_key.to_price();
            let book = if is_buy { &mut self.asks } else { &mut self.bids };
            let queue = match book.get_mut(&best_key) {
                Some(q) => q,
                None => break,
            };
            while remaining > 1e-9 && !queue.is_empty() {
                let mut head = queue[0].clone();
                let fill = remaining.min(head.volume);
                raws.push(RawTrade {
                    bid_order_id: if is_buy {
                        order_id.clone()
                    } else {
                        head.order_id.clone()
                    },
                    ask_order_id: if is_buy {
                        head.order_id.clone()
                    } else {
                        order_id.clone()
                    },
                    price: best_price,
                    volume: fill,
                });
                remaining -= fill;
                head.volume -= fill;
                if head.volume < 1e-9 {
                    queue.remove(0);
                    self.order_index.remove(&head.order_id);
                } else {
                    queue[0] = head;
                }
            }
            if queue.is_empty() {
                book.remove(&best_key);
            }
        }
        raws
    }

    fn match_limit_order(&mut self, order: BookOrder) -> Vec<RawTrade> {
        let mut raws = Vec::new();
        let mut remaining_vol = order.volume;
        let is_buy = order.direction.is_buy();
        let order_price = order.price;
        let order_id = order.order_id.clone();

        loop {
            let best_key = if is_buy {
                self.asks.keys().next().copied()
            } else {
                self.bids.keys().next_back().copied()
            };
            let best_key = match best_key {
                Some(k) => k,
                None => break,
            };
            let best_price = best_key.to_price();
            let can_match = if is_buy {
                order_price >= best_price - 1e-9
            } else {
                order_price <= best_price + 1e-9
            };
            if !can_match {
                break;
            }
            let book = if is_buy { &mut self.asks } else { &mut self.bids };
            let queue = match book.get_mut(&best_key) {
                Some(q) => q,
                None => break,
            };
            while remaining_vol > 1e-9 && !queue.is_empty() {
                let mut head = queue[0].clone();
                let fill = remaining_vol.min(head.volume);
                raws.push(RawTrade {
                    bid_order_id: if is_buy {
                        order_id.clone()
                    } else {
                        head.order_id.clone()
                    },
                    ask_order_id: if is_buy {
                        head.order_id.clone()
                    } else {
                        order_id.clone()
                    },
                    price: best_price,
                    volume: fill,
                });
                remaining_vol -= fill;
                head.volume -= fill;
                if head.volume < 1e-9 {
                    queue.remove(0);
                    self.order_index.remove(&head.order_id);
                } else {
                    queue[0] = head;
                }
            }
            let empty = queue.is_empty();
            if empty {
                book.remove(&best_key);
            }
            if remaining_vol < 1e-9 {
                break;
            }
        }

        // 剩余量进入订单簿
        if remaining_vol > 1e-9 {
            let key = PriceKey::from_price(order_price);
            let ts = self.next_seq();
            let book = if is_buy { &mut self.bids } else { &mut self.asks };
            self.order_index
                .insert(order_id.clone(), (order.direction, order_price));
            let entry = book.entry(key).or_default();
            entry.push(BookOrder {
                order_id,
                direction: order.direction,
                price: order_price,
                volume: remaining_vol,
                timestamp: ts,
            });
        }
        raws
    }

    /// 撤单
    pub fn cancel(&mut self, order_id: &str) -> bool {
        let (direction, price) = match self.order_index.remove(order_id) {
            Some(v) => v,
            None => return false,
        };
        let key = PriceKey::from_price(price);
        if direction.is_buy() {
            remove_from_book(&mut self.bids, &key, order_id)
        } else {
            remove_from_book(&mut self.asks, &key, order_id)
        }
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().map(|k| k.to_price())
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().map(|k| k.to_price())
    }

    pub fn spread(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some((a + b) / 2.0),
            _ => None,
        }
    }

    pub fn depth_bids(&self, n: usize) -> Vec<(Price, Volume)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(k, q)| (k.to_price(), q.iter().map(|o| o.volume).sum()))
            .collect()
    }

    pub fn depth_asks(&self, n: usize) -> Vec<(Price, Volume)> {
        self.asks
            .iter()
            .take(n)
            .map(|(k, q)| (k.to_price(), q.iter().map(|o| o.volume).sum()))
            .collect()
    }
}

/// 从订单簿中删除指定订单，并在队列为空时移除价格层级
fn remove_from_book(
    book: &mut BTreeMap<PriceKey, Vec<BookOrder>>,
    key: &PriceKey,
    order_id: &str,
) -> bool {
    let mut should_remove = false;
    let mut found = false;
    if let Some(queue) = book.get_mut(key) {
        let before = queue.len();
        queue.retain(|o| o.order_id != order_id);
        found = queue.len() < before;
        should_remove = queue.is_empty();
    }
    if should_remove {
        book.remove(key);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_order_matches_opposite_book() {
        let mut ob = Orderbook::new("000001");
        let asks = ob.add_order(BookOrder {
            order_id: "s1".into(),
            direction: Direction::Sell,
            price: 10.0,
            volume: 100.0,
            timestamp: 0,
        });
        assert!(asks.is_empty());
        assert_eq!(ob.best_ask(), Some(10.0));
        let trades = ob.add_order(BookOrder {
            order_id: "b1".into(),
            direction: Direction::Buy,
            price: 10.0,
            volume: 50.0,
            timestamp: 0,
        });
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].volume, 50.0);
        assert_eq!(trades[0].price, 10.0);
        assert_eq!(ob.best_ask(), Some(10.0));
    }

    #[test]
    fn price_priority() {
        let mut ob = Orderbook::new("000001");
        ob.add_order(BookOrder {
            order_id: "s1".into(),
            direction: Direction::Sell,
            price: 10.0,
            volume: 100.0,
            timestamp: 0,
        });
        ob.add_order(BookOrder {
            order_id: "s2".into(),
            direction: Direction::Sell,
            price: 9.9,
            volume: 100.0,
            timestamp: 0,
        });
        let trades = ob.add_order(BookOrder {
            order_id: "b1".into(),
            direction: Direction::Buy,
            price: 0.0,
            volume: 60.0,
            timestamp: 0,
        });
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 9.9);
        assert_eq!(trades[0].volume, 60.0);
        assert_eq!(ob.best_ask(), Some(9.9));
    }

    #[test]
    fn cancel_removes_from_book() {
        let mut ob = Orderbook::new("000001");
        ob.add_order(BookOrder {
            order_id: "s1".into(),
            direction: Direction::Sell,
            price: 10.0,
            volume: 100.0,
            timestamp: 0,
        });
        assert!(ob.cancel("s1"));
        assert!(ob.best_ask().is_none());
    }
}
