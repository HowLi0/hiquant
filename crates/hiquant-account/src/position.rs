//! 持仓实体

use hiquant_core::{Amount, AssetId, Date, Direction, Offset, Price, Volume};
use hiquant_protocol::qifi;
use serde::{Deserialize, Serialize};

/// 持仓：多空今昨分离，含成本/保证金/浮动盈亏计算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub code: AssetId,
    #[serde(default)]
    pub exchange_id: String,
    /// 今仓多头
    pub volume_long_today: Volume,
    /// 昨仓多头
    pub volume_long_his: Volume,
    /// 今仓空头
    pub volume_short_today: Volume,
    /// 昨仓空头
    pub volume_short_his: Volume,
    /// 冻结（今/昨 多/空）
    #[serde(default)]
    pub volume_long_frozen_today: Volume,
    #[serde(default)]
    pub volume_long_frozen_his: Volume,
    #[serde(default)]
    pub volume_short_frozen_today: Volume,
    #[serde(default)]
    pub volume_short_frozen_his: Volume,

    pub position_price_long: Price,
    pub position_price_short: Price,
    pub position_cost_long: Amount,
    pub position_cost_short: Amount,
    pub open_price_long: Price,
    pub open_price_short: Price,
    pub open_cost_long: Amount,
    pub open_cost_short: Amount,
    pub margin_long: Amount,
    pub margin_short: Amount,

    /// 最新价
    pub lastest_price: Price,
    #[serde(default)]
    pub lastest_datetime: String,

    /// 合约预设
    #[serde(default)]
    pub preset: crate::preset::CodePreset,
}

impl Position {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            exchange_id: String::new(),
            volume_long_today: 0.0,
            volume_long_his: 0.0,
            volume_short_today: 0.0,
            volume_short_his: 0.0,
            volume_long_frozen_today: 0.0,
            volume_long_frozen_his: 0.0,
            volume_short_frozen_today: 0.0,
            volume_short_frozen_his: 0.0,
            position_price_long: 0.0,
            position_price_short: 0.0,
            position_cost_long: 0.0,
            position_cost_short: 0.0,
            open_price_long: 0.0,
            open_price_short: 0.0,
            open_cost_long: 0.0,
            open_cost_short: 0.0,
            margin_long: 0.0,
            margin_short: 0.0,
            lastest_price: 0.0,
            lastest_datetime: String::new(),
            preset: crate::preset::CodePreset::stock_default(),
        }
    }

    pub fn volume_long(&self) -> Volume {
        self.volume_long_today + self.volume_long_his
    }

    pub fn volume_short(&self) -> Volume {
        self.volume_short_today + self.volume_short_his
    }

    pub fn volume_long_avaliable(&self) -> Volume {
        self.volume_long() - self.volume_long_frozen_today - self.volume_long_frozen_his
    }

    pub fn volume_short_avaliable(&self) -> Volume {
        self.volume_short() - self.volume_short_frozen_today - self.volume_short_frozen_his
    }

    pub fn volume_net(&self) -> Volume {
        self.volume_long() - self.volume_short()
    }

    pub fn volume_total(&self) -> Volume {
        self.volume_long() + self.volume_short()
    }

    pub fn market_value(&self) -> Amount {
        (self.volume_long() - self.volume_short()) * self.lastest_price * self.preset.unit_table
    }

    /// 持仓盈亏（基于持仓成本）
    pub fn position_profit(&self) -> Amount {
        let long = (self.lastest_price - self.position_price_long)
            * self.volume_long()
            * self.preset.unit_table;
        let short = (self.position_price_short - self.lastest_price)
            * self.volume_short()
            * self.preset.unit_table;
        long + short
    }

    /// 浮动盈亏（基于开仓成本）
    pub fn float_profit(&self) -> Amount {
        let long = (self.lastest_price - self.open_price_long)
            * self.volume_long()
            * self.preset.unit_table;
        let short = (self.open_price_short - self.lastest_price)
            * self.volume_short()
            * self.preset.unit_table;
        long + short
    }

    pub fn avg_price_long(&self) -> Price {
        let v = self.volume_long();
        if v.abs() < 1e-9 {
            return 0.0;
        }
        self.position_cost_long / (v * self.preset.unit_table)
    }

    pub fn avg_price_short(&self) -> Price {
        let v = self.volume_short();
        if v.abs() < 1e-9 {
            return 0.0;
        }
        self.position_cost_short / (v.abs() * self.preset.unit_table)
    }

    pub fn margin(&self) -> Amount {
        self.margin_long + self.margin_short
    }

    /// 接收一笔成交，更新持仓与成本
    pub fn receive_deal(
        &mut self,
        _trade_id: &str,
        direction: Direction,
        offset: Offset,
        volume: Volume,
        price: Price,
        datetime: impl Into<String>,
    ) {
        let unit = self.preset.unit_table;
        self.lastest_price = price;
        self.lastest_datetime = datetime.into();

        match (direction, offset) {
            (Direction::Buy, Offset::Open) => {
                // 开多：今仓增加，按成交价加权平均成本
                let new_vol = self.volume_long_today + volume;
                let new_cost = self.position_cost_long
                    + price * volume * unit;
                if new_vol > 1e-9 {
                    self.position_price_long = new_cost / (new_vol * unit);
                }
                self.position_cost_long = new_cost;
                self.open_cost_long += price * volume * unit;
                if self.open_price_long.abs() < 1e-9 {
                    self.open_price_long = price;
                }
                self.volume_long_today = new_vol;
                // 保证金 = 开仓成本 * 保证金率
                self.margin_long = self.position_cost_long * self.preset.margin_ratio;
            }
            (Direction::Sell, Offset::Open) => {
                let new_vol = self.volume_short_today + volume;
                let new_cost = self.position_cost_short
                    + price * volume * unit;
                if new_vol > 1e-9 {
                    self.position_price_short = new_cost / (new_vol * unit);
                }
                self.position_cost_short = new_cost;
                self.open_cost_short += price * volume * unit;
                if self.open_price_short.abs() < 1e-9 {
                    self.open_price_short = price;
                }
                self.volume_short_today = new_vol;
                self.margin_short = self.position_cost_short * self.preset.margin_ratio;
            }
            (Direction::Sell, Offset::Close | Offset::CloseToday | Offset::CloseYesterday) => {
                // 平多：先平今仓，再平昨仓
                let mut vol = volume;
                let close_today = vol.min(self.volume_long_today);
                if close_today > 0.0 {
                    self.volume_long_today -= close_today;
                    vol -= close_today;
                }
                if vol > 0.0 {
                    let close_his = vol.min(self.volume_long_his);
                    self.volume_long_his -= close_his;
                    vol -= close_his;
                }
                let v = self.volume_long();
                if v.abs() < 1e-9 {
                    self.position_cost_long = 0.0;
                    self.margin_long = 0.0;
                } else {
                    self.position_cost_long = self.position_price_long * v * unit;
                    self.margin_long = self.position_cost_long * self.preset.margin_ratio;
                }
            }
            (Direction::Buy, Offset::Close | Offset::CloseToday | Offset::CloseYesterday) => {
                let mut vol = volume;
                let close_today = vol.min(self.volume_short_today);
                if close_today > 0.0 {
                    self.volume_short_today -= close_today;
                    vol -= close_today;
                }
                if vol > 0.0 {
                    let close_his = vol.min(self.volume_short_his);
                    self.volume_short_his -= close_his;
                    vol -= close_his;
                }
                let v = self.volume_short();
                if v.abs() < 1e-9 {
                    self.position_cost_short = 0.0;
                    self.margin_short = 0.0;
                } else {
                    self.position_cost_short = self.position_price_short * v * unit;
                    self.margin_short = self.position_cost_short * self.preset.margin_ratio;
                }
            }
        }
    }

    /// 价格变动：重算最新价与浮动盈亏
    pub fn on_price_change(&mut self, new_price: Price, datetime: impl Into<String>) {
        self.lastest_price = new_price;
        self.lastest_datetime = datetime.into();
    }

    /// 日终结算：今仓转昨仓
    pub fn daily_settle(&mut self) {
        self.volume_long_his += self.volume_long_today;
        self.volume_long_today = 0.0;
        self.volume_short_his += self.volume_short_today;
        self.volume_short_today = 0.0;
    }

    pub fn freeze_position(&mut self, direction: Direction, volume: Volume) -> bool {
        match direction {
            Direction::Sell => {
                let avail = self.volume_long_avaliable();
                if avail < volume {
                    return false;
                }
                // 优先冻结今仓
                let f_today = volume.min(self.volume_long_today);
                self.volume_long_frozen_today += f_today;
                let rest = volume - f_today;
                self.volume_long_frozen_his += rest;
                true
            }
            Direction::Buy => {
                let avail = self.volume_short_avaliable();
                if avail < volume {
                    return false;
                }
                let f_today = volume.min(self.volume_short_today);
                self.volume_short_frozen_today += f_today;
                let rest = volume - f_today;
                self.volume_short_frozen_his += rest;
                true
            }
        }
    }

    pub fn unfreeze_position(&mut self, direction: Direction, volume: Volume) {
        match direction {
            Direction::Sell => {
                let f_today = volume.min(self.volume_long_frozen_today);
                self.volume_long_frozen_today -= f_today;
                let rest = volume - f_today;
                self.volume_long_frozen_his -= rest.min(self.volume_long_frozen_his);
            }
            Direction::Buy => {
                let f_today = volume.min(self.volume_short_frozen_today);
                self.volume_short_frozen_today -= f_today;
                let rest = volume - f_today;
                self.volume_short_frozen_his -= rest.min(self.volume_short_frozen_his);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.volume_total().abs() < 1e-9
    }
}

impl From<&Position> for qifi::Position {
    fn from(p: &Position) -> Self {
        qifi::Position {
            instrument_id: p.code.clone(),
            exchange_id: p.exchange_id.clone(),
            volume_long_today: p.volume_long_today,
            volume_long_his: p.volume_long_his,
            volume_short_today: p.volume_short_today,
            volume_short_his: p.volume_short_his,
            volume_long_frozen_today: p.volume_long_frozen_today,
            volume_long_frozen_his: p.volume_long_frozen_his,
            volume_short_frozen_today: p.volume_short_frozen_today,
            volume_short_frozen_his: p.volume_short_frozen_his,
            position_price_long: p.position_price_long,
            position_price_short: p.position_price_short,
            position_cost_long: p.position_cost_long,
            position_cost_short: p.position_cost_short,
            open_price_long: p.open_price_long,
            open_price_short: p.open_price_short,
            open_cost_long: p.open_cost_long,
            open_cost_short: p.open_cost_short,
            margin_long: p.margin_long,
            margin_short: p.margin_short,
            lastest_price: p.lastest_price,
            float_pnl_long: 0.0,
            float_pnl_short: 0.0,
            close_pnl: 0.0,
            position_profit: p.position_profit(),
            float_profit: p.float_profit(),
            lastest_datetime: p.lastest_datetime.clone(),
        }
    }
}

/// 持仓统计聚合
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PositionStats {
    pub total_market_value: Amount,
    pub total_margin: Amount,
    pub total_position_profit: Amount,
    pub total_float_profit: Amount,
    pub position_count: usize,
}

impl PositionStats {
    pub fn from_positions(positions: &[Position]) -> Self {
        let mut s = Self::default();
        s.position_count = positions.iter().filter(|p| !p.is_empty()).count();
        for p in positions {
            s.total_market_value += p.market_value();
            s.total_margin += p.margin();
            s.total_position_profit += p.position_profit();
            s.total_float_profit += p.float_profit();
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_then_close_profit() {
        let mut p = Position::new("000001");
        // 买入 100 股 @ 10
        p.receive_deal("t1", Direction::Buy, Offset::Open, 100.0, 10.0, "2024-01-02");
        assert_eq!(p.volume_long(), 100.0);
        // 价格涨到 11
        p.on_price_change(11.0, "2024-01-02");
        assert!((p.float_profit() - 100.0).abs() < 1e-6);
        // 平仓
        p.receive_deal("t2", Direction::Sell, Offset::Close, 100.0, 11.0, "2024-01-02");
        assert!(p.volume_long().abs() < 1e-9);
    }
}
