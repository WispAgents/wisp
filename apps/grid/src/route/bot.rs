//! The Telegram bot the grid owns for `on laptop`: what's kept of it, who may route, and Telegram's Bot API.
use serde_json::{Value, json};

use super::{MAX_SENDERS, Telegram};

/// What's sealed for a user's route.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bot {
    pub bot: String,
    pub name: String,
    pub secret: String,
    pub senders: Vec<String>,
}

/// `BotFather`'s `123456:ABC-DEF`.
pub fn bot_token_ok(t: &str) -> bool {
    t.split_once(':').is_some_and(|(id, rest)| {
        (1..=20).contains(&id.len())
            && id.bytes().all(|b| b.is_ascii_digit())
            && (1..=100).contains(&rest.len())
            && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}

/// The people who may route, as typed (ids, comma or space apart).
pub fn senders_of(text: &str) -> std::result::Result<Vec<String>, String> {
    let mut ids: Vec<String> = Vec::new();
    for x in text.split([',', ' ', '\n', '\t']).filter(|x| !x.is_empty()) {
        if !(1..=20).contains(&x.len()) || !x.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Telegram user ids are numbers: @userinfobot tells you yours.".into());
        }
        if !ids.iter().any(|i| i == x) {
            ids.push(x.to_owned());
        }
    }
    if ids.is_empty() {
        return Err("Telegram user ids are numbers: @userinfobot tells you yours.".into());
    }
    if ids.len() > MAX_SENDERS {
        return Err(format!("At most {MAX_SENDERS} people."));
    }
    Ok(ids)
}

/// A listed person, in a private chat with the bot, not a forward.
pub fn person(m: &Value, b: &Bot) -> bool {
    let from = &m["from"];
    let id = from["id"].as_i64().map(|n| n.to_string()).unwrap_or_default();
    from.is_object()
        && from["is_bot"] != json!(true)
        && m["chat"]["type"] == "private"
        && b.senders.contains(&id)
        && m.get("forward_origin").is_none()
        && m.get("forward_date").is_none()
}

/// Telegram over HTTPS (grid.json `channels.telegramApi` for tests).
#[derive(Debug, Default)]
pub struct HttpTelegram {
    pub base: String,
    pub client: reqwest::Client,
}

#[async_trait::async_trait]
impl Telegram for HttpTelegram {
    async fn call(&self, bot: &str, method: &str, body: Value) -> std::result::Result<Value, String> {
        let r = self
            .client
            .post(format!("{}/bot{bot}/{method}", self.base.trim_end_matches('/')))
            .json(&body)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = r.status().as_u16();
        let j: Value = r.json().await.unwrap_or(Value::Null);
        if j["ok"] != json!(true) {
            return Err(j["description"].as_str().map_or_else(|| status.to_string(), str::to_owned));
        }
        Ok(j["result"].clone())
    }
}
