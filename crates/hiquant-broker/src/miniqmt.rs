//! miniqmt / 大QMT 实盘桥接
//!
//! 由于 miniqmt 仅提供 Python SDK（xtquant），实盘接入采用「Python sidecar」模式：
//!   Rust 主程序  --HTTP-->  Python sidecar（xtquant 调用 miniqmt）
//!
//! sidecar 约定的 HTTP 接口（POST/GET，JSON）：
//!   POST /place_order    body: BrokerOrder      -> OrderResponse
//!   POST /cancel_order   body: {broker_order_id} -> {ok: bool}
//!   GET  /account        -> BrokerAccount
//!   GET  /positions      -> [BrokerPosition]
//!   GET  /trades         -> [Trade]
//!   GET  /quote?code=... -> BrokerQuote
//!   GET  /ping           -> {ok: true}
//!
//! 用户启动 sidecar 后，在 Rust 端配置 base_url 即可使用。
//! sidecar 的 Python 参考实现见 `python_sidecar/miniqmt_sidecar.py`。

use crate::broker::{
    BrokerAccount, BrokerOrder, BrokerPosition, BrokerQuote, Broker, OrderResponse,
};
use async_trait::async_trait;
use hiquant_core::{Result, HiquantError};
use hiquant_protocol::qifi::Trade;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniQmtConfig {
    /// sidecar 的 HTTP 基址，如 http://127.0.0.1:7788
    pub base_url: String,
    /// 超时（秒）
    pub timeout_secs: u64,
    /// 账户 ID（mini qmt 资金账号）
    pub account_id: String,
}

impl Default for MiniQmtConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:7788".into(),
            timeout_secs: 10,
            account_id: String::new(),
        }
    }
}

pub struct MiniQmtBroker {
    config: MiniQmtConfig,
    client: reqwest::Client,
}

impl MiniQmtBroker {
    pub fn new(config: MiniQmtConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("reqwest client build");
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .client
            .get(self.url(path))
            .send()
            .await
            .map_err(|e| HiquantError::Broker(format!("sidecar get {path}: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HiquantError::Broker(format!(
                "sidecar {path} HTTP {status}: {body}"
            )));
        }
        resp.json::<T>()
            .await
            .map_err(|e| HiquantError::Broker(format!("sidecar decode {path}: {e}")))
    }

    async fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self
            .client
            .post(self.url(path))
            .json(body)
            .send()
            .await
            .map_err(|e| HiquantError::Broker(format!("sidecar post {path}: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HiquantError::Broker(format!(
                "sidecar {path} HTTP {status}: {body}"
            )));
        }
        resp.json::<T>()
            .await
            .map_err(|e| HiquantError::Broker(format!("sidecar decode {path}: {e}")))
    }
}

#[async_trait]
impl Broker for MiniQmtBroker {
    fn name(&self) -> &str {
        "miniqmt"
    }

    async fn place_order(&self, order: BrokerOrder) -> Result<OrderResponse> {
        self.post("/place_order", &order).await
    }

    async fn cancel_order(&self, broker_order_id: &str) -> Result<bool> {
        #[derive(serde::Serialize)]
        struct Req<'a> {
            broker_order_id: &'a str,
        }
        #[derive(serde::Deserialize)]
        struct Resp {
            ok: bool,
        }
        let r: Resp = self
            .post("/cancel_order", &Req { broker_order_id })
            .await?;
        Ok(r.ok)
    }

    async fn query_account(&self) -> Result<BrokerAccount> {
        self.get("/account").await
    }

    async fn query_positions(&self) -> Result<Vec<BrokerPosition>> {
        self.get("/positions").await
    }

    async fn query_trades(&self) -> Result<Vec<Trade>> {
        self.get("/trades").await
    }

    async fn query_quote(&self, code: &str) -> Result<BrokerQuote> {
        self.get(&format!("/quote?code={}", urlencoding::encode_value(code)))
            .await
    }

    async fn is_connected(&self) -> bool {
        #[derive(serde::Deserialize)]
        struct Ping {
            ok: bool,
        }
        self.get::<Ping>("/ping").await.map(|p| p.ok).unwrap_or(false)
    }
}

/// URL 参数编码（避免引入额外 crate）
mod urlencoding {
    pub fn encode_value(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_basics() {
        assert_eq!(urlencoding::encode_value("000001"), "000001");
        assert_eq!(urlencoding::encode_value("a b"), "a%20b");
        assert_eq!(urlencoding::encode_value("IF2306"), "IF2306");
    }
}
