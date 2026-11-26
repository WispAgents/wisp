//! `grok usage <SESSION_ID>` ("Print persisted token and cost usage for a session"), as captured from grok 1.0.70 on
//! 2026-11-25 (`tests/cli/usage.json`, `tests/cli/usage-empty.json`): `{sessionId, updatedAt, session: {inputTokens,
//! outputTokens, cachedReadTokens, cacheCreationTokens, reasoningTokens, totalTokens, modelCalls, costUsdTicks?,
//! turnCount, primaryModelId?, modelUsage?}, turns: [{turnNumber, endedAt, …the same counters}]}`. A session with
//! nothing recorded exits non-zero with `Error: No usage recorded …` on stderr. `costUsdTicks` is absent when the CLI
//! priced nothing. Sans-IO: the host lists the sessions and runs the CLI.

use serde_json::Value;

/// xAI bills in ticks: "1 USD = 10,000,000,000 ticks (10^10)" (<https://docs.x.ai/developers/cost-tracking>, read
/// 2026-11-25), the unit of grok's `costUsdTicks`.
pub const TICKS_PER_USD: f64 = 1e10;

/// One turn of a session's usage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageTurn {
    pub number: u32,
    /// RFC 3339, as the CLI wrote it
    pub ended_at: String,
    /// `totalTokens` (input, cache reads included, plus output)
    pub tokens: u64,
    pub cached_read: u64,
    /// None: the CLI recorded no cost for it
    pub cost_usd: Option<f64>,
    pub model: Option<String>,
}

/// One session's usage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrokUsage {
    pub session_id: String,
    pub updated_at: String,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub turns: Vec<UsageTurn>,
}

fn count(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

#[allow(clippy::cast_precision_loss)] // ticks of a session's cost, far below 2^52
fn cost(v: &Value) -> Option<f64> {
    v.get("costUsdTicks").and_then(Value::as_u64).map(|t| t as f64 / TICKS_PER_USD)
}

/// A session's usage from what `grok usage <id>` printed; None for anything else (its "No usage recorded" error).
pub fn usage(out: &str) -> Option<GrokUsage> {
    let v: Value = serde_json::from_str(out.trim()).ok()?;
    let session = v.get("session").filter(|s| s.is_object())?;
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let turns = v
        .get("turns")
        .and_then(Value::as_array)
        .map(|ts| {
            ts.iter()
                .filter(|t| t.is_object())
                .map(|t| UsageTurn {
                    number: u32::try_from(count(t, "turnNumber")).unwrap_or(u32::MAX),
                    ended_at: text(t, "endedAt"),
                    tokens: count(t, "totalTokens"),
                    cached_read: count(t, "cachedReadTokens"),
                    cost_usd: cost(t),
                    model: t.get("primaryModelId").and_then(Value::as_str).map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(GrokUsage {
        session_id: text(&v, "sessionId"),
        updated_at: text(&v, "updatedAt"),
        tokens: count(session, "totalTokens"),
        cost_usd: cost(session),
        turns,
    })
}

/// What the sessions' turns that ended at or after `since` add up to. `ms` reads an RFC 3339 time (0 when it can't).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spent {
    /// None: no turn in the time had a cost recorded
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub turns: u32,
    /// turns with tokens and no recorded cost
    pub unpriced: u32,
}

pub fn spent(sessions: &[GrokUsage], since: i64, ms: &dyn Fn(&str) -> i64) -> Spent {
    let mut s = Spent::default();
    for t in sessions.iter().flat_map(|u| &u.turns).filter(|t| ms(&t.ended_at) >= since) {
        s.turns += 1;
        s.tokens = s.tokens.saturating_add(t.tokens);
        match t.cost_usd {
            Some(c) => s.cost_usd = Some(s.cost_usd.unwrap_or(0.0) + c),
            None if t.tokens > 0 => s.unpriced += 1,
            None => {}
        }
    }
    s
}
