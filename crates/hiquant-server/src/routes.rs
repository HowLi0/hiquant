//! 路由表与服务器启动

use crate::api;
use crate::state::AppState;
use crate::ws::ws_handler;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

/// 构建完整路由
pub fn app(state: Arc<AppState>, static_dir: Option<std::path::PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/health", get(api::health))
        .route("/codes", get(api::list_codes))
        .route("/bars", get(api::query_bars))
        .route("/accounts", get(api::list_accounts).post(api::create_account))
        .route("/accounts/:name/summary", get(api::account_summary))
        .route("/accounts/:name/positions", get(api::account_positions))
        .route("/orders", post(api::place_order))
        .route("/backtest", post(api::run_backtest))
        .route("/sync", post(api::sync_sample_data))
        .route("/mock-feed", post(api::start_mock_feed))
        .route("/broker/account", get(api::broker_account))
        .route("/broker/positions", get(api::broker_positions))
        .route("/broker/place_order", post(api::broker_place_order));

    let mut router = Router::new()
        .route("/ws", get(ws_handler))
        .nest("/api", api_routes)
        .layer(cors)
        .with_state(state);

    if let Some(dir) = static_dir {
        router = router.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    }
    router
}

/// 启动 HTTP 服务
pub async fn serve(
    state: Arc<AppState>,
    addr: std::net::SocketAddr,
    static_dir: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let app = app(state, static_dir);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("hiquant server listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
