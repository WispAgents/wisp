//! Delivering a held message into its box over loopback.
use serde_json::{Value, json};

use super::{Deliver, Delivery};
use crate::secrets::sha256;

/// Delivers into a box over loopback: `POST /api/cmd {cmd: "channels.inbound", args}` with the box's proxy secret,
/// the row's key as the Idempotency-Key (a retry of the same row runs once). The answer is the transport's.
#[derive(Debug, Default)]
pub struct HttpDeliver {
    pub client: reqwest::Client,
}

#[async_trait::async_trait]
impl Deliver for HttpDeliver {
    async fn deliver(&self, port: u16, secret: &str, d: &Delivery, timeout_ms: u64) -> std::result::Result<(u16, Value), String> {
        use base64::Engine as _;
        let args = json!({ "channel": d.channel, "headers": d.headers, "body": base64::engine::general_purpose::STANDARD.encode(&d.body), "at": d.at, "dropped": d.dropped, "verified": d.verified });
        let r = self
            .client
            .post(format!("http://127.0.0.1:{port}/api/cmd"))
            .header("x-wisp-proxy-secret", secret)
            .header("idempotency-key", sha256(&d.key))
            .json(&json!({ "cmd": "channels.inbound", "args": args }))
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        // the command answered: the transport's own status and body; else the edge's status
        Ok(match v.get("result") {
            Some(out) if status == 200 => (
                out.get("status").and_then(Value::as_u64).and_then(|s| u16::try_from(s).ok()).unwrap_or(200),
                out.get("body").cloned().unwrap_or(Value::Null),
            ),
            _ => (status, v),
        })
    }
}
