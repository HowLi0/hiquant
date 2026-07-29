//! WebSocket 广播与订阅

use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::broadcast;

/// 广播器：向所有订阅者推送 JSON 消息
#[derive(Clone)]
pub struct Broadcaster {
    tx: broadcast::Sender<String>,
}

impl Broadcaster {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn send<T: Serialize>(&self, msg: &T) {
        if let Ok(json) = serde_json::to_string(msg) {
            // 忽略无订阅者错误
            let _ = self.tx.send(json);
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// WS 推送的事件类型
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    Price {
        code: String,
        price: f64,
        datetime: String,
    },
    Account {
        summary: hiquant_account::AccountSummary,
    },
    Order {
        order: hiquant_account::Order,
    },
    Trade {
        trade: hiquant_protocol::qifi::Trade,
    },
    Log {
        level: String,
        message: String,
        datetime: String,
    },
}

/// /ws 处理器
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<std::sync::Arc<crate::state::AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let mut rx = state.broadcaster.subscribe();
        // 推送一次当前账户快照
        for name in state.market.account_names() {
            if let Some(acc) = state.market.get_account(&name) {
                let evt = WsEvent::Account {
                    summary: acc.summary(),
                };
                if let Ok(json) = serde_json::to_string(&evt) {
                    let _ = socket.send(Message::Text(json)).await;
                }
            }
        }
        loop {
            tokio::select! {
                Ok(msg) = rx.recv() => {
                    if socket.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                Some(Ok(msg)) = socket.next() => {
                    if matches!(msg, Message::Close(_)) {
                        break;
                    }
                }
            }
        }
    })
}
