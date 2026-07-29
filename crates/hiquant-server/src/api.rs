//! REST API handlers
//!
//! 路由分组：
//!   GET  /api/health
//!   GET  /api/codes
//!   GET  /api/bars?code=&freq=&start=&end=
//!   GET  /api/accounts
//!   GET  /api/accounts/:name/summary
//!   GET  /api/accounts/:name/positions
//!   POST /api/accounts            {account_cookie, init_cash}
//!   POST /api/orders              {account_cookie, code, direction, volume, price}
//!   POST /api/backtest            {code, fast, slow, lots, init_cash, days, seed}
//!   GET  /api/broker/account
//!   GET  /api/broker/positions
//!   POST /api/broker/place_order  {code, direction, volume, price}

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use hiquant_broker::BrokerOrder;
use hiquant_core::{Date, Direction, Frequency};
use hiquant_engine::{BacktestConfig, BacktestEngine, SmaCrossStrategy, SmaParams};
use hiquant_market::MarketOrder;
use hiquant_storage::StoreConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

type SharedState = State<Arc<AppState>>;

#[derive(Debug, Deserialize)]
pub struct BarsQuery {
    pub code: String,
    pub freq: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountReq {
    pub account_cookie: String,
    pub init_cash: f64,
}

#[derive(Debug, Deserialize)]
pub struct PlaceOrderReq {
    pub account_cookie: String,
    pub code: String,
    /// "buy" | "sell"
    pub direction: String,
    pub volume: f64,
    pub price: f64,
}

#[derive(Debug, Deserialize)]
pub struct BacktestReq {
    pub code: String,
    #[serde(default)]
    pub fast: usize,
    #[serde(default)]
    pub slow: usize,
    #[serde(default)]
    pub lots: f64,
    #[serde(default)]
    pub init_cash: f64,
    #[serde(default)]
    pub days: usize,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Debug, Deserialize)]
pub struct BrokerOrderReq {
    pub code: String,
    /// "buy" | "sell"
    pub direction: String,
    pub volume: f64,
    pub price: f64,
}

#[derive(Debug, Serialize)]
pub struct GenericResp<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> GenericResp<T> {
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

pub async fn health() -> Json<GenericResp<&'static str>> {
    Json(GenericResp::ok("hiquant-rs server running"))
}

pub async fn list_codes(State(state): SharedState) -> Json<GenericResp<Vec<String>>> {
    match state.store.list_codes() {
        Ok(codes) => Json(GenericResp::ok(codes)),
        Err(e) => Json(GenericResp::err(e.to_string())),
    }
}

pub async fn query_bars(
    State(state): SharedState,
    Query(q): Query<BarsQuery>,
) -> Json<GenericResp<Vec<hiquant_data::Bar>>> {
    let freq = q
        .freq
        .as_deref()
        .and_then(Frequency::from_str)
        .unwrap_or(Frequency::Day);
    let start = q.start.clone().unwrap_or_else(|| "2000-01-01".into());
    let end = q.end.clone().unwrap_or_else(|| "2099-12-31".into());
    match state.store.query_bars(&q.code, freq, &start, &end) {
        Ok(bars) => Json(GenericResp::ok(bars)),
        Err(e) => Json(GenericResp::err(e.to_string())),
    }
}

pub async fn list_accounts(State(state): SharedState) -> Json<GenericResp<Vec<String>>> {
    Json(GenericResp::ok(state.market.account_names()))
}

pub async fn account_summary(
    State(state): SharedState,
    Path(name): Path<String>,
) -> Json<GenericResp<hiquant_account::AccountSummary>> {
    match state.market.get_account(&name) {
        Some(acc) => Json(GenericResp::ok(acc.summary())),
        None => Json(GenericResp::err(format!("account {name} not found"))),
    }
}

pub async fn account_positions(
    State(state): SharedState,
    Path(name): Path<String>,
) -> Json<GenericResp<Vec<hiquant_account::Position>>> {
    match state.market.get_account(&name) {
        Some(acc) => Json(GenericResp::ok(acc.positions())),
        None => Json(GenericResp::err(format!("account {name} not found"))),
    }
}

pub async fn create_account(
    State(state): SharedState,
    Json(req): Json<CreateAccountReq>,
) -> Result<Json<GenericResp<String>>, StatusCode> {
    let acc = state.market.register_account(req.account_cookie.clone(), req.init_cash);
    state
        .broadcaster
        .send(&crate::ws::WsEvent::Log {
            level: "info".into(),
            message: format!("account {} created", acc.account_cookie),
            datetime: chrono::Utc::now().to_rfc3339(),
        });
    Ok(Json(GenericResp::ok(acc.account_cookie.clone())))
}

pub async fn place_order(
    State(state): SharedState,
    Json(req): Json<PlaceOrderReq>,
) -> Json<GenericResp<usize>> {
    let direction = match Direction::from_str(&req.direction) {
        Some(d) => d,
        None => return Json(GenericResp::err(format!("invalid direction: {}", req.direction))),
    };
    let mut mo = if direction.is_buy() {
        MarketOrder::buy(&req.account_cookie, &req.code, req.volume, req.price)
    } else {
        MarketOrder::sell(&req.account_cookie, &req.code, req.volume, req.price)
    };
    mo.direction = direction;
    state.market.schedule_order(mo);
    let trades = state.market.process_order_queue();
    let n = trades.len();
    // 广播成交
    for t in trades {
        state.broadcaster.send(&crate::ws::WsEvent::Trade {
            trade: hiquant_protocol::qifi::Trade {
                instrument_id: req.code.clone(),
                price: t.price,
                volume: t.volume,
                ..Default::default()
            },
        });
    }
    // 广播账户更新
    if let Some(acc) = state.market.get_account(&req.account_cookie) {
        state
            .broadcaster
            .send(&crate::ws::WsEvent::Account {
                summary: acc.summary(),
            });
    }
    Json(GenericResp::ok(n))
}

pub async fn run_backtest(
    State(state): SharedState,
    Json(req): Json<BacktestReq>,
) -> Json<GenericResp<hiquant_engine::BacktestResult>> {
    let fast = if req.fast == 0 { 5 } else { req.fast };
    let slow = if req.slow == 0 { 20 } else { req.slow };
    let lots = if req.lots == 0.0 { 100.0 } else { req.lots };
    let init_cash = if req.init_cash == 0.0 {
        *state.init_cash.read()
    } else {
        req.init_cash
    };
    let days = if req.days == 0 { 120 } else { req.days };
    let seed = if req.seed == 0 { 42 } else { req.seed };

    // 优先从数据库读取；若没有则用生成器生成样本数据
    let bars = match state.store.query_bars(
        &req.code,
        Frequency::Day,
        "2000-01-01",
        "2099-12-31",
    ) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            let mut gen = hiquant_data::SampleDataGenerator::new(seed);
            gen.gen_daily(
                &req.code,
                Date::from_ymd(2024, 1, 2),
                days,
                10.0,
                0.10,
                0.30,
                false,
            )
        }
    };

    let params = SmaParams {
        code: req.code.clone(),
        fast,
        slow,
        lots,
    };
    let strategy = Box::new(SmaCrossStrategy::new(params));
    let cfg = BacktestConfig {
        account_cookie: format!("bt_{}", req.code),
        init_cash,
        environment: hiquant_core::AccountEnvironment::Backtest,
    };
    let mut engine = BacktestEngine::new(cfg, strategy);
    engine.load_bars(req.code.clone(), bars);
    let result = engine.run();
    Json(GenericResp::ok(result))
}

pub async fn broker_account(
    State(state): SharedState,
) -> Json<GenericResp<hiquant_broker::BrokerAccount>> {
    match state.broker().await {
        Some(b) => match b.query_account().await {
            Ok(acc) => Json(GenericResp::ok(acc)),
            Err(e) => Json(GenericResp::err(e.to_string())),
        },
        None => Json(GenericResp::err("no broker configured")),
    }
}

pub async fn broker_positions(
    State(state): SharedState,
) -> Json<GenericResp<Vec<hiquant_broker::BrokerPosition>>> {
    match state.broker().await {
        Some(b) => match b.query_positions().await {
            Ok(p) => Json(GenericResp::ok(p)),
            Err(e) => Json(GenericResp::err(e.to_string())),
        },
        None => Json(GenericResp::err("no broker configured")),
    }
}

pub async fn broker_place_order(
    State(state): SharedState,
    Json(req): Json<BrokerOrderReq>,
) -> Json<GenericResp<hiquant_broker::OrderResponse>> {
    let direction = match Direction::from_str(&req.direction) {
        Some(d) => d,
        None => return Json(GenericResp::err(format!("invalid direction: {}", req.direction))),
    };
    let order = if direction.is_buy() {
        BrokerOrder::buy(&req.code, req.volume, req.price)
    } else {
        BrokerOrder::sell(&req.code, req.volume, req.price)
    };
    match state.broker().await {
        Some(b) => match b.place_order(order).await {
            Ok(r) => Json(GenericResp::ok(r)),
            Err(e) => Json(GenericResp::err(e.to_string())),
        },
        None => Json(GenericResp::err("no broker configured")),
    }
}

/// 触发一次数据库增量同步示例（用生成器填充数据，便于前端演示）
pub async fn sync_sample_data(
    State(state): SharedState,
    Json(req): Json<BacktestReq>,
) -> Json<GenericResp<hiquant_storage::UpsertResult>> {
    let days = if req.days == 0 { 120 } else { req.days };
    let seed = if req.seed == 0 { 42 } else { req.seed };
    let mut gen = hiquant_data::SampleDataGenerator::new(seed);
    let bars = gen.gen_daily(
        &req.code,
        Date::from_ymd(2024, 1, 2),
        days,
        10.0,
        0.10,
        0.30,
        false,
    );
    match state.store.upsert_bars(&req.code, Frequency::Day, &bars) {
        Ok(r) => Json(GenericResp::ok(r)),
        Err(e) => Json(GenericResp::err(e.to_string())),
    }
}

/// 启动一个模拟行情推送循环（用于演示 WS 实时行情）
pub async fn start_mock_feed(State(state): SharedState) -> Json<GenericResp<&'static str>> {
    let market = state.market.clone();
    let broadcaster = state.broadcaster.clone();
    tokio::spawn(async move {
        let codes = market.account_names();
        let code = "000001".to_string();
        let mut price = 10.0_f64;
        let mut step = 0u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            step += 1;
            // 简单随机游走
            let delta = ((step as f64).sin() * 0.05) + (step as f64 % 3.0 - 1.5) * 0.02;
            price = (price + delta).max(1.0);
            let now = chrono::Utc::now().to_rfc3339();
            market.update_price(&code, price);
            broadcaster.send(&crate::ws::WsEvent::Price {
                code: code.clone(),
                price,
                datetime: now.clone(),
            });
            // 广播账户
            for name in &codes {
                if let Some(acc) = market.get_account(name) {
                    broadcaster.send(&crate::ws::WsEvent::Account {
                        summary: acc.summary(),
                    });
                }
            }
            let _ = StoreConfig::default;
        }
    });
    Json(GenericResp::ok("mock feed started"))
}
