//! 账户：资金、持仓、订单的统一管理

use crate::order::{Order, OrderSide};
use crate::position::{Position, PositionStats};
use crate::preset::{CodePreset, MarketPreset};
use parking_lot::RwLock;
use hiquant_core::{
    generate_uuid, Amount, AssetId, Direction, Offset, OrderStatus, Price, Volume,
};
use hiquant_protocol::qifi;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Stock,
    Future,
    Forex,
}

/// 账户快照摘要
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountSummary {
    pub account_cookie: String,
    pub init_cash: Amount,
    pub cash: Amount,
    pub frozen_cash: Amount,
    pub available: Amount,
    pub market_value: Amount,
    pub margin: Amount,
    pub total_value: Amount,
    pub float_pnl: Amount,
    pub close_pnl: Amount,
    pub position_count: usize,
    pub order_count: usize,
}

#[derive(Debug)]
struct AccountInner {
    cash: Amount,
    frozen_cash: Amount,
    close_pnl: Amount,
    commission_total: Amount,
    tax_total: Amount,
    positions: HashMap<AssetId, Position>,
    orders: HashMap<String, Order>,
    frozen_by_order: HashMap<String, Amount>,
    market_prices: HashMap<AssetId, Price>,
    market_preset: MarketPreset,
}

impl AccountInner {
    fn market_value(&self) -> Amount {
        self.positions.values().map(|p| p.market_value()).sum()
    }
    fn margin(&self) -> Amount {
        self.positions.values().map(|p| p.margin()).sum()
    }
    fn float_pnl(&self) -> Amount {
        self.positions.values().map(|p| p.float_profit()).sum()
    }
}

/// 账户
pub struct Account {
    pub account_cookie: String,
    pub user_cookie: String,
    pub portfolio_cookie: String,
    pub account_type: AccountType,
    pub init_cash: Amount,
    inner: Arc<RwLock<AccountInner>>,
}

impl Account {
    pub fn new(account_cookie: impl Into<String>, init_cash: Amount, ty: AccountType) -> Self {
        Self {
            account_cookie: account_cookie.into(),
            user_cookie: String::new(),
            portfolio_cookie: String::new(),
            account_type: ty,
            init_cash,
            inner: Arc::new(RwLock::new(AccountInner {
                cash: init_cash,
                frozen_cash: 0.0,
                close_pnl: 0.0,
                commission_total: 0.0,
                tax_total: 0.0,
                positions: HashMap::new(),
                orders: HashMap::new(),
                frozen_by_order: HashMap::new(),
                market_prices: HashMap::new(),
                market_preset: MarketPreset::with_defaults(),
            })),
        }
    }

    pub fn new_stock(account_cookie: impl Into<String>, init_cash: Amount) -> Self {
        Self::new(account_cookie, init_cash, AccountType::Stock)
    }

    pub fn new_future(account_cookie: impl Into<String>, init_cash: Amount) -> Self {
        Self::new(account_cookie, init_cash, AccountType::Future)
    }

    pub fn create_with_uuid(init_cash: Amount, ty: AccountType) -> Self {
        Self::new(generate_uuid(), init_cash, ty)
    }

    /// 下单（核心入口）：风控 → 冻结 → 入订单簿
    pub fn place_order(
        &self,
        side: OrderSide,
        code: &str,
        volume: Volume,
        price: Price,
    ) -> hiquant_core::Result<String> {
        if volume <= 0.0 {
            return Err(hiquant_core::HiquantError::Order("volume must be positive".into()));
        }
        if price < 0.0 {
            return Err(hiquant_core::HiquantError::Order("price must be non-negative".into()));
        }
        let order = crate::order::OrderFactory::create(&self.account_cookie, code, side, volume, price);

        let mut inner = self.inner.write();
        let preset = inner.market_preset.get(code);

        // 取出或创建持仓
        let mut pos = inner.positions.remove(code).unwrap_or_else(|| {
            let mut p = Position::new(code);
            p.preset = preset.clone();
            p
        });
        pos.preset = preset.clone();

        // 风控 + 冻结
        let freeze_action: FreezeAction = match side {
            OrderSide::Buy | OrderSide::BuyOpen => {
                let frozen = preset.calc_frozenmoney(price, volume);
                if inner.cash - inner.frozen_cash < frozen {
                    inner.positions.insert(code.to_string(), pos);
                    return Err(hiquant_core::HiquantError::RiskCheck(format!(
                        "insufficient cash: available {:.2} < need {:.2}",
                        inner.cash - inner.frozen_cash,
                        frozen
                    )));
                }
                FreezeAction::Cash(frozen)
            }
            OrderSide::BuyClose => {
                let avail = pos.volume_short_avaliable();
                if avail < volume {
                    inner.positions.insert(code.to_string(), pos);
                    return Err(hiquant_core::HiquantError::RiskCheck(format!(
                        "insufficient short position: avail {:.0} < need {:.0}",
                        avail, volume
                    )));
                }
                pos.freeze_position(Direction::Buy, volume);
                FreezeAction::Position
            }
            OrderSide::SellOpen => {
                let margin = preset.calc_margin(price, volume);
                if inner.cash - inner.frozen_cash < margin {
                    inner.positions.insert(code.to_string(), pos);
                    return Err(hiquant_core::HiquantError::RiskCheck(format!(
                        "insufficient cash for margin: available {:.2} < need {:.2}",
                        inner.cash - inner.frozen_cash,
                        margin
                    )));
                }
                FreezeAction::Cash(margin)
            }
            OrderSide::Sell | OrderSide::SellClose => {
                let avail = pos.volume_long_avaliable();
                if avail < volume {
                    inner.positions.insert(code.to_string(), pos);
                    return Err(hiquant_core::HiquantError::RiskCheck(format!(
                        "insufficient long position: avail {:.0} < need {:.0}",
                        avail, volume
                    )));
                }
                pos.freeze_position(Direction::Sell, volume);
                FreezeAction::Position
            }
        };

        // 应用资金冻结
        if let FreezeAction::Cash(amt) = freeze_action {
            inner.frozen_cash += amt;
            inner.frozen_by_order.insert(order.order_id.clone(), amt);
        }

        inner.positions.insert(code.to_string(), pos);
        let oid = order.order_id.clone();
        inner.orders.insert(oid.clone(), order);
        Ok(oid)
    }

    pub fn buy(&self, code: &str, volume: Volume, price: Price) -> hiquant_core::Result<String> {
        self.place_order(OrderSide::Buy, code, volume, price)
    }

    pub fn sell(&self, code: &str, volume: Volume, price: Price) -> hiquant_core::Result<String> {
        self.place_order(OrderSide::Sell, code, volume, price)
    }

    pub fn cancel_order(&self, order_id: &str) -> hiquant_core::Result<()> {
        let mut inner = self.inner.write();
        let (direction, volume_left, instrument, can_cancel) = {
            let order = inner
                .orders
                .get_mut(order_id)
                .ok_or_else(|| {
                    hiquant_core::HiquantError::Order(format!("order not found: {order_id}"))
                })?;
            if !order.can_cancel() {
                return Err(hiquant_core::HiquantError::Order(format!(
                    "order cannot be cancelled: status {:?}",
                    order.status
                )));
            }
            (order.direction, order.volume_left, order.instrument_id.clone(), true)
        };
        if !can_cancel {
            return Ok(());
        }
        // 标记撤单
        if let Some(o) = inner.orders.get_mut(order_id) {
            o.cancel();
        }
        // 解冻资金
        if let Some(frozen) = inner.frozen_by_order.remove(order_id) {
            inner.frozen_cash -= frozen;
        }
        // 解冻持仓
        if let Some(pos) = inner.positions.get_mut(&instrument) {
            pos.unfreeze_position(direction, volume_left);
        }
        Ok(())
    }

    /// 成交回报
    pub fn on_trade(
        &self,
        order_id: &str,
        filled_volume: Volume,
        filled_price: Price,
    ) -> hiquant_core::Result<()> {
        // 先取出订单信息（不持有 pos 借用）
        let (direction, offset, instrument, volume_orign, finished) = {
            let mut inner = self.inner.write();
            let order = inner.orders.get_mut(order_id).ok_or_else(|| {
                hiquant_core::HiquantError::Order(format!("order not found: {order_id}"))
            })?;
            if !order.is_active() {
                return Err(hiquant_core::HiquantError::Order(format!(
                    "order not active: status {:?}",
                    order.status
                )));
            }
            order.update(filled_volume, filled_price);
            (
                order.direction,
                order.offset,
                order.instrument_id.clone(),
                order.volume_orign,
                order.is_finished(),
            )
        };

        let mut inner = self.inner.write();
        let preset = inner.market_preset.get(&instrument);
        let trade_amount = preset.calc_marketvalue(filled_price, filled_volume);
        let commission = preset.calc_commission(trade_amount);
        let tax = if direction == Direction::Sell {
            trade_amount * preset.tax_ratio
        } else {
            0.0
        };

        // 释放冻结资金
        if let Some(frozen) = inner.frozen_by_order.remove(order_id) {
            let ratio = filled_volume / volume_orign;
            let release = frozen * ratio;
            inner.frozen_cash -= release;
            if finished {
                // 剩余冻结全部释放
                let rest = inner.frozen_by_order.remove(order_id).unwrap_or(0.0);
                inner.frozen_cash -= rest;
            } else {
                inner.frozen_by_order.insert(order_id.to_string(), frozen - release);
            }
        }

        // 取出持仓做更新
        let mut pos = inner.positions.remove(&instrument).unwrap_or_else(|| {
            let mut p = Position::new(&instrument);
            p.preset = preset.clone();
            p
        });
        pos.preset = preset.clone();
        pos.unfreeze_position(direction, filled_volume);
        let trade_id = generate_uuid();
        pos.receive_deal(
            &trade_id,
            direction,
            offset,
            filled_volume,
            filled_price,
            chrono::Utc::now().to_rfc3339(),
        );

        // 计算已实现盈亏（先取出 open price）
        let mut close_pnl_delta = 0.0;
        let mut cash_recover = 0.0;
        if matches!(
            offset,
            Offset::Close | Offset::CloseToday | Offset::CloseYesterday
        ) {
            let open_price = match direction {
                Direction::Sell => pos.open_price_long,
                Direction::Buy => pos.open_price_short,
            };
            let unit = preset.unit_table;
            close_pnl_delta = match direction {
                Direction::Sell => (filled_price - open_price) * filled_volume * unit,
                Direction::Buy => (open_price - filled_price) * filled_volume * unit,
            };
            cash_recover = filled_price * filled_volume * unit;
        }

        inner.positions.insert(instrument.clone(), pos);
        inner.commission_total += commission;
        inner.tax_total += tax;
        inner.cash -= commission + tax;
        inner.close_pnl += close_pnl_delta;
        inner.cash += cash_recover;

        // 开仓时扣资金
        if matches!(offset, Offset::Open) {
            if preset.is_stock {
                inner.cash -= filled_price * filled_volume * preset.unit_table;
            } else {
                inner.cash -= preset.calc_margin(filled_price, filled_volume);
            }
        }

        Ok(())
    }

    pub fn update_market_data(&self, code: &str, price: Price) {
        let mut inner = self.inner.write();
        inner.market_prices.insert(code.to_string(), price);
        if let Some(pos) = inner.positions.get_mut(code) {
            pos.on_price_change(price, chrono::Utc::now().to_rfc3339());
        }
    }

    pub fn cash(&self) -> Amount {
        self.inner.read().cash
    }
    pub fn frozen_cash(&self) -> Amount {
        self.inner.read().frozen_cash
    }
    pub fn available_cash(&self) -> Amount {
        let inner = self.inner.read();
        inner.cash - inner.frozen_cash
    }
    pub fn close_pnl(&self) -> Amount {
        self.inner.read().close_pnl
    }
    pub fn commission_total(&self) -> Amount {
        self.inner.read().commission_total
    }
    pub fn market_value(&self) -> Amount {
        self.inner.read().market_value()
    }
    pub fn margin(&self) -> Amount {
        self.inner.read().margin()
    }
    pub fn float_pnl(&self) -> Amount {
        self.inner.read().float_pnl()
    }
    pub fn total_value(&self) -> Amount {
        let inner = self.inner.read();
        inner.cash + inner.market_value() + inner.float_pnl()
    }

    pub fn positions(&self) -> Vec<Position> {
        self.inner.read().positions.values().cloned().collect()
    }

    pub fn position(&self, code: &str) -> Option<Position> {
        self.inner.read().positions.get(code).cloned()
    }

    pub fn has_position(&self, code: &str) -> bool {
        self.inner
            .read()
            .positions
            .get(code)
            .map(|p| !p.is_empty())
            .unwrap_or(false)
    }

    pub fn orders(&self) -> Vec<Order> {
        self.inner.read().orders.values().cloned().collect()
    }

    pub fn order(&self, order_id: &str) -> Option<Order> {
        self.inner.read().orders.get(order_id).cloned()
    }

    pub fn pending_orders(&self) -> Vec<Order> {
        self.inner
            .read()
            .orders
            .values()
            .filter(|o| o.status.is_active())
            .cloned()
            .collect()
    }

    pub fn filled_orders(&self) -> Vec<Order> {
        self.inner
            .read()
            .orders
            .values()
            .filter(|o| o.status == OrderStatus::Filled)
            .cloned()
            .collect()
    }

    pub fn daily_settle(&self) {
        let mut inner = self.inner.write();
        for pos in inner.positions.values_mut() {
            pos.daily_settle();
        }
    }

    pub fn position_stats(&self) -> PositionStats {
        PositionStats::from_positions(&self.positions())
    }

    pub fn summary(&self) -> AccountSummary {
        let inner = self.inner.read();
        AccountSummary {
            account_cookie: self.account_cookie.clone(),
            init_cash: self.init_cash,
            cash: inner.cash,
            frozen_cash: inner.frozen_cash,
            available: inner.cash - inner.frozen_cash,
            market_value: inner.market_value(),
            margin: inner.margin(),
            total_value: inner.cash + inner.market_value() + inner.float_pnl(),
            float_pnl: inner.float_pnl(),
            close_pnl: inner.close_pnl,
            position_count: inner.positions.values().filter(|p| !p.is_empty()).count(),
            order_count: inner.orders.len(),
        }
    }

    pub fn to_qifi(&self) -> qifi::Qifi {
        let inner = self.inner.read();
        let mut q = qifi::Qifi::new(self.account_cookie.clone(), self.init_cash);
        q.portfolio = self.portfolio_cookie.clone();
        q.investor_name = self.user_cookie.clone();
        q.updatetime = chrono::Utc::now().to_rfc3339();

        for (_code, pos) in &inner.positions {
            if !pos.is_empty() {
                q.add_position(qifi::Position::from(pos));
            }
        }
        for (_oid, order) in &inner.orders {
            q.add_order(qifi::Order::from(order.clone()));
        }

        let mv = inner.market_value();
        let fp = inner.float_pnl();
        let mg = inner.margin();
        q.accounts.balance = inner.cash + mv + fp;
        q.accounts.static_balance = self.init_cash;
        q.accounts.margin = mg;
        q.accounts.frozen_margin = inner.frozen_cash;
        q.accounts.available = inner.cash - inner.frozen_cash;
        q.accounts.position_profit = fp;
        q.accounts.float_profit = fp;
        q.accounts.close_profit = inner.close_pnl;
        q.accounts.commission = inner.commission_total;
        q.accounts.tax = inner.tax_total;
        q.calculate_derived_values();
        q
    }

    pub fn risk_ratio(&self) -> f64 {
        let tv = self.total_value();
        if tv.abs() < 1e-9 {
            return 0.0;
        }
        self.margin() / tv
    }
}

enum FreezeAction {
    Cash(Amount),
    Position,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_buy_and_close() {
        let acc = Account::new_stock("acc1", 100_000.0);
        let oid = acc.buy("000001", 100.0, 10.0).unwrap();
        acc.on_trade(&oid, 100.0, 10.0).unwrap();
        let pos = acc.position("000001").unwrap();
        assert_eq!(pos.volume_long(), 100.0);
        acc.update_market_data("000001", 11.0);
        let summary = acc.summary();
        assert!(summary.float_pnl > 90.0 && summary.float_pnl < 110.0);
    }

    #[test]
    fn insufficient_cash_rejected() {
        let acc = Account::new_stock("acc1", 1_000.0);
        let r = acc.buy("000001", 10000.0, 100.0);
        assert!(r.is_err());
    }

    #[test]
    fn cancel_releases_frozen() {
        let acc = Account::new_stock("acc1", 100_000.0);
        let oid = acc.buy("000001", 100.0, 10.0).unwrap();
        // 100*10 + min fee 5 ≈ 1005
        assert!(acc.frozen_cash() >= 1000.0);
        acc.cancel_order(&oid).unwrap();
        assert!(acc.frozen_cash() < 1e-6);
    }
}
