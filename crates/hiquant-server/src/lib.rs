//! hiquant-server: axum REST + WebSocket 服务
//!
//! 提供：
//! - REST API：行情查询、账户管理、下单、回测、broker 桥接
//! - WebSocket：实时推送行情/账户/成交事件
//! - 静态前端托管：把构建好的 React 前端放在 `static/` 目录即可通过根路径访问

pub mod api;
pub mod routes;
pub mod state;
pub mod ws;

pub use routes::{app, serve};
pub use state::AppState;
pub use ws::{ws_handler, Broadcaster, WsEvent};
