//! 交易核心枚举

use serde::{Deserialize, Serialize};

/// 买卖方向。BUY=1, SELL=-1（与 C++ 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i8)]
pub enum Direction {
    Buy = 1,
    Sell = -1,
}

impl Direction {
    pub fn as_i8(self) -> i8 {
        self as i8
    }

    pub fn is_buy(self) -> bool {
        matches!(self, Direction::Buy)
    }

    pub fn is_sell(self) -> bool {
        matches!(self, Direction::Sell)
    }

    pub fn opposite(self) -> Self {
        match self {
            Direction::Buy => Direction::Sell,
            Direction::Sell => Direction::Buy,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Buy => "buy",
            Direction::Sell => "sell",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "buy" | "b" | "1" => Direction::Buy,
            "sell" | "s" | "-1" => Direction::Sell,
            _ => return None,
        })
    }
}

/// 开平方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Offset {
    Open,
    Close,
    CloseToday,
    CloseYesterday,
}

impl Offset {
    pub fn as_str(self) -> &'static str {
        match self {
            Offset::Open => "open",
            Offset::Close => "close",
            Offset::CloseToday => "close_today",
            Offset::CloseYesterday => "close_yesterday",
        }
    }
}

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    PartialFilled,
    Filled,
    Cancelled,
    Rejected,
}

impl OrderStatus {
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected
        )
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            OrderStatus::Pending | OrderStatus::PartialFilled
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::PartialFilled => "partial_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Rejected => "rejected",
        }
    }
}

/// 持仓方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionSide {
    Long,
    Short,
}

/// 市场类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketType {
    Stock,
    Future,
    Option,
    Forex,
    Fund,
    Bond,
    Index,
    Crypto,
}

impl MarketType {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketType::Stock => "stock",
            MarketType::Future => "future",
            MarketType::Option => "option",
            MarketType::Forex => "forex",
            MarketType::Fund => "fund",
            MarketType::Bond => "bond",
            MarketType::Index => "index",
            MarketType::Crypto => "crypto",
        }
    }
}

/// 价格类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceType {
    Limit,
    Market,
    Stop,
    StopLimit,
}

/// 账户运行环境：回测 / 模拟 / 实盘
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountEnvironment {
    Backtest,
    Sim,
    Real,
}

impl AccountEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            AccountEnvironment::Backtest => "backtest",
            AccountEnvironment::Sim => "sim",
            AccountEnvironment::Real => "real",
        }
    }
}
