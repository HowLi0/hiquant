//! 订单实体

use hiquant_core::{
    generate_order_id, Amount, AssetId, Direction, Offset, OrderStatus, Price, PriceType, Volume,
};
use hiquant_protocol::qifi;
use serde::{Deserialize, Serialize};

/// 订单（业务实体，对齐 C++ account::Order）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub account_cookie: String,
    pub user_cookie: String,
    pub portfolio_cookie: String,
    pub instrument_id: AssetId,
    #[serde(default)]
    pub secu_code: String,
    #[serde(default)]
    pub exchange_id: String,
    pub direction: Direction,
    pub offset: Offset,
    pub volume_orign: Volume,
    pub volume_left: Volume,
    pub volume_fill: Volume,
    pub price_order: Price,
    pub price_fill: Price,
    pub price_type: PriceType,
    pub status: OrderStatus,
    #[serde(default)]
    pub fee: Amount,
    #[serde(default)]
    pub tax: Amount,
    #[serde(default)]
    pub order_time: String,
    #[serde(default)]
    pub trade_time: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub error_message: String,
}

impl Order {
    pub fn new(
        account_cookie: impl Into<String>,
        instrument_id: impl Into<String>,
        direction: Direction,
        offset: Offset,
        volume: Volume,
        price: Price,
    ) -> Self {
        let id = generate_order_id("ORD");
        Self {
            order_id: id,
            account_cookie: account_cookie.into(),
            user_cookie: String::new(),
            portfolio_cookie: String::new(),
            instrument_id: instrument_id.into(),
            secu_code: String::new(),
            exchange_id: String::new(),
            direction,
            offset,
            volume_orign: volume,
            volume_left: volume,
            volume_fill: 0.0,
            price_order: price,
            price_fill: 0.0,
            price_type: PriceType::Limit,
            status: OrderStatus::Pending,
            fee: 0.0,
            tax: 0.0,
            order_time: chrono::Utc::now().to_rfc3339(),
            trade_time: String::new(),
            reason: String::new(),
            error_message: String::new(),
        }
    }

    /// 买卖侧合并方向（用于股票：BUY=1, SELL=-1）
    pub fn towards(&self) -> i32 {
        match (self.direction, self.offset) {
            (Direction::Buy, _) => 1,
            (Direction::Sell, Offset::Open) => -1,
            (Direction::Sell, _) => -1,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.status.is_finished()
    }

    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub fn can_cancel(&self) -> bool {
        self.status.is_active()
    }

    pub fn unfilled_volume(&self) -> Volume {
        self.volume_left
    }

    pub fn filled_ratio(&self) -> f64 {
        if self.volume_orign.abs() < 1e-9 {
            0.0
        } else {
            self.volume_fill / self.volume_orign
        }
    }

    pub fn filled_amount(&self) -> Amount {
        self.price_fill * self.volume_fill
    }

    pub fn total_amount(&self) -> Amount {
        self.price_order * self.volume_orign
    }

    pub fn update(&mut self, filled_volume: Volume, filled_price: Price) {
        self.volume_fill += filled_volume;
        self.volume_left = (self.volume_orign - self.volume_fill).max(0.0);
        if filled_price > 0.0 {
            if self.volume_fill.abs() > 1e-9 {
                self.price_fill = (self.price_fill * (self.volume_fill - filled_volume)
                    + filled_price * filled_volume)
                    / self.volume_fill;
            } else {
                self.price_fill = filled_price;
            }
        }
        if self.volume_left.abs() < 1e-9 {
            self.status = OrderStatus::Filled;
            self.trade_time = chrono::Utc::now().to_rfc3339();
        } else if self.volume_fill.abs() > 1e-9 {
            self.status = OrderStatus::PartialFilled;
        }
    }

    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
    }

    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = OrderStatus::Rejected;
        self.error_message = reason.into();
    }
}

impl From<Order> for qifi::Order {
    fn from(o: Order) -> Self {
        let towards = o.towards();
        let direction = o.direction.as_str().to_string();
        let offset = o.offset.as_str().to_string();
        let price_type = format!("{:?}", o.price_type);
        let status = o.status.as_str().to_string();
        qifi::Order {
            order_id: o.order_id,
            account_cookie: o.account_cookie,
            user_cookie: o.user_cookie,
            portfolio_cookie: o.portfolio_cookie,
            instrument_id: o.instrument_id,
            secu_code: o.secu_code,
            exchange_id: o.exchange_id,
            direction,
            offset,
            volume_orign: o.volume_orign,
            volume_left: o.volume_left,
            volume_fill: o.volume_fill,
            price_order: o.price_order,
            price_fill: o.price_fill,
            price_type,
            status,
            fee: o.fee,
            tax: o.tax,
            order_time: o.order_time,
            trade_time: o.trade_time,
            towards,
        }
    }
}

/// 简化的下单参数
#[derive(Debug, Clone, Copy)]
pub enum OrderSide {
    /// 股票买入
    Buy,
    /// 股票卖出
    Sell,
    /// 期货开多
    BuyOpen,
    /// 期货开空
    SellOpen,
    /// 期货平多
    SellClose,
    /// 期货平空
    BuyClose,
}

/// 订单工厂：快速创建各类订单
pub struct OrderFactory;

impl OrderFactory {
    pub fn create(
        account_cookie: &str,
        instrument_id: &str,
        side: OrderSide,
        volume: Volume,
        price: Price,
    ) -> Order {
        let (dir, off) = match side {
            OrderSide::Buy => (Direction::Buy, Offset::Open),
            OrderSide::Sell => (Direction::Sell, Offset::Close),
            OrderSide::BuyOpen => (Direction::Buy, Offset::Open),
            OrderSide::SellOpen => (Direction::Sell, Offset::Open),
            OrderSide::SellClose => (Direction::Sell, Offset::Close),
            OrderSide::BuyClose => (Direction::Buy, Offset::Close),
        };
        Order::new(account_cookie, instrument_id, dir, off, volume, price)
    }

    pub fn create_stock_buy(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::Buy, vol, price)
    }

    pub fn create_stock_sell(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::Sell, vol, price)
    }

    pub fn create_future_buy_open(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::BuyOpen, vol, price)
    }

    pub fn create_future_sell_open(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::SellOpen, vol, price)
    }

    pub fn create_future_sell_close(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::SellClose, vol, price)
    }

    pub fn create_future_buy_close(account: &str, code: &str, vol: Volume, price: Price) -> Order {
        Self::create(account, code, OrderSide::BuyClose, vol, price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_lifecycle() {
        let mut o = OrderFactory::create_stock_buy("acc", "000001", 100.0, 10.0);
        assert_eq!(o.status, OrderStatus::Pending);
        o.update(50.0, 10.0);
        assert_eq!(o.status, OrderStatus::PartialFilled);
        o.update(50.0, 10.5);
        assert_eq!(o.status, OrderStatus::Filled);
        assert!((o.price_fill - 10.25).abs() < 1e-9);
    }
}
