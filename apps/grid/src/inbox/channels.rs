//! The channels a box takes messages from, and what of each request is worth keeping (v1 grid/inbox.js `CHANNELS`).
use serde_json::{Value, json};

use crate::secrets::sha256;

/// What a channel's request is worth keeping, and how to read it.
#[derive(Debug)]
pub struct ChannelDef {
    pub name: &'static str,
    /// the headers the box needs to check it (its signature)
    pub headers: &'static [&'static str],
    pub event_id: fn(&Value) -> Option<String>,
    /// the sender wants the box's answer in this same call
    pub live: fn(&Value) -> bool,
    /// what no box acts on: not held (a wake for it would be refused and count as a strike)
    pub skip: fn(&Value) -> bool,
    /// what the sender gets at once for a message held (Discord wants an answer in three seconds, the box may be
    /// asleep); None: the plain 200
    pub early: fn(&Value) -> Option<Value>,
}

fn kind_of(j: &Value) -> Option<i64> {
    j.get("type").and_then(Value::as_i64)
}

/// Discord's signature: Ed25519 over the timestamp and the body, by the application's public key (hex). No key, no
/// signature, or one that doesn't match: false.
pub fn discord_signed(public_key: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
    fn hex<const N: usize>(s: &str) -> Option<[u8; N]> {
        let s = s.trim().as_bytes();
        if s.len() != N * 2 {
            return None;
        }
        let mut out = [0u8; N];
        for (i, pair) in s.chunks(2).enumerate() {
            let d = |c: u8| (c as char).to_digit(16);
            out[i] = u8::try_from(d(pair[0])? * 16 + d(pair[1])?).ok()?;
        }
        Some(out)
    }
    if timestamp.is_empty() {
        return false;
    }
    let (Some(key), Some(sig)) = (hex::<32>(public_key), hex::<64>(signature)) else { return false };
    let Ok(key) = VerifyingKey::from_bytes(&key) else { return false };
    let mut msg = timestamp.as_bytes().to_vec();
    msg.extend_from_slice(body);
    key.verify(&msg, &Signature::from_bytes(&sig)).is_ok()
}

/// Discord: "thinking" for `/wisp` (5), "seen" for a press (6). The box goes on from that answer.
fn discord_early(j: &Value) -> Option<Value> {
    match kind_of(j)? {
        2 => Some(json!({ "type": 5 })),
        3 => Some(json!({ "type": 6 })),
        _ => None,
    }
}

fn str_of(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str().map(str::to_owned).or_else(|| x.as_i64().map(|n| n.to_string())))
}

/// Slack posts an edit, a deletion and wisp's own replies like any message; a file is the one subtype that is
/// someone speaking.
fn slack_noise(j: &Value) -> bool {
    let e = &j["event"];
    e.is_object()
        && (e.get("bot_id").is_some_and(|b| !b.is_null()) || e.get("subtype").and_then(Value::as_str).is_some_and(|s| s != "file_share"))
}

pub const CHANNELS: [ChannelDef; 4] = [
    ChannelDef {
        name: "telegram",
        headers: &["x-telegram-bot-api-secret-token"],
        event_id: |j| str_of(j.get("update_id")),
        live: |_| false,
        skip: |_| false,
        early: |_| None,
    },
    ChannelDef {
        name: "slack",
        headers: &["x-slack-signature", "x-slack-request-timestamp"],
        event_id: |j| str_of(j.get("event_id")),
        live: |j| j.get("type").and_then(Value::as_str) == Some("url_verification"),
        skip: slack_noise,
        early: |_| None,
    },
    ChannelDef {
        name: "discord",
        headers: &["x-signature-ed25519", "x-signature-timestamp"],
        event_id: |j| str_of(j.get("id")),
        live: |j| matches!(kind_of(j), Some(1 | 4)),
        skip: |j| !matches!(kind_of(j), Some(2 | 3)),
        early: discord_early,
    },
    // held by the grid's mail (mail/service.rs), never by a request: its headers are the grid's own verdict
    ChannelDef {
        name: "email",
        headers: &["x-wisp-mail-from", "x-wisp-mail-dmarc", "x-wisp-mail-id"],
        event_id: |j| str_of(j.get("messageId")),
        live: |_| false,
        skip: |_| false,
        early: |_| None,
    },
];

pub fn channel(name: &str) -> Option<&'static ChannelDef> {
    CHANNELS.iter().find(|c| c.name == name)
}

/// A request's row key: the channel's own event id, else the body's hash, and the headers it came with. An id alone
/// could be taken: anyone holding the URL could post the next update id with a bad secret, and the real update
/// would hit that row and vanish.
pub fn event_key(c: &ChannelDef, j: &Value, body: &[u8], headers: &str) -> String {
    let id = (c.event_id)(j).unwrap_or_else(|| sha256(body));
    format!("{}:{id}:{}", c.name, sha256(headers))
}
