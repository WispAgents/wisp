//! API list prices by model id, and the estimate of a turn's cost from its tokens when its harness reports none
//! (ADR-0119). Sans-IO: a static table.
//!
//! Only exact ids are priced (after [`bare_model`]): a model this table doesn't name stays unpriced, never
//! guessed from a family. An estimate is always flagged (`cost_estimated`) and shown with a `~`, so it never reads as
//! what the vendor billed. Subscription harnesses (Codex, Cursor, Copilot, Antigravity) bill a plan, not tokens: the
//! estimate says what the same tokens would cost at API list prices.

use wisp_runtime_api::model::{Turn, TurnStats};

/// USD per million tokens at API list price, as `[input, cache read, output]`; table dated 2026-11-25 (ADR-0119).
pub const PRICES: &[(&str, [f64; 3])] = &[
    // Anthropic (the five cold.rs had, unchanged; v1 #763: Fable 5.2 lists at $8 / $0.20 / $40)
    ("claude-opus-5-5", [4.0, 0.2, 20.0]),
    ("claude-sonnet-5-5", [2.0, 0.2, 10.0]),
    ("claude-fable-5-2", [8.0, 0.2, 40.0]),
    ("claude-fable-5-1", [10.0, 0.25, 50.0]),
    ("claude-fable-5", [10.0, 0.25, 50.0]),
    ("claude-opus-4-5", [5.0, 0.5, 25.0]),
    ("claude-opus-4-1", [15.0, 1.5, 75.0]),
    ("claude-sonnet-4-5", [3.0, 0.3, 15.0]),
    ("claude-haiku-4-5", [1.0, 0.1, 5.0]),
    // OpenAI (Codex, and the GPT ids Cursor and Copilot run)
    ("gpt-5", [1.25, 0.125, 10.0]),
    ("gpt-5-codex", [1.25, 0.125, 10.0]),
    ("gpt-5.1", [1.25, 0.125, 10.0]),
    ("gpt-5.1-codex", [1.25, 0.125, 10.0]),
    ("gpt-5-mini", [0.25, 0.025, 2.0]),
    // Google (Antigravity)
    ("gemini-2.5-pro", [1.25, 0.31, 10.0]),
    ("gemini-2.5-flash", [0.3, 0.075, 2.5]),
    ("gemini-3-pro-preview", [2.0, 0.2, 12.0]),
    // xAI (Grok)
    ("grok-4", [3.0, 0.75, 15.0]),
    ("grok-4-fast", [0.2, 0.05, 0.5]),
    ("grok-code-fast-1", [0.2, 0.02, 1.5]),
    // Cursor's own
    ("composer-1", [1.25, 0.125, 10.0]),
];

/// A cache write as a multiple of input (the 5-minute write; the 1-hour one is cold.rs's).
const WRITE: f64 = 1.25;

/// v1 `bareModel`, widened: `claude-opus-5-5[1m]` → `claude-opus-5-5`, `…-20261101` → `…`, `anthropic/claude-…` →
/// `claude-…` (opencode's provider/model), and Claude's dotted form (`claude-sonnet-4.5`, Copilot's) → dashed.
pub fn bare_model(id: &str) -> String {
    let id = id.trim();
    let id = id.strip_suffix("[1m]").or_else(|| id.strip_suffix("[1M]")).unwrap_or(id);
    let id = id.rsplit_once('/').map_or(id, |(_, m)| m);
    let id = match id.rsplit_once('-') {
        Some((head, d)) if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => id,
    };
    let id = id.to_ascii_lowercase();
    if id.starts_with("claude-") { id.replace('.', "-") } else { id }
}

/// A model's list price, by its bare id.
pub fn price(model: &str) -> Option<[f64; 3]> {
    let id = bare_model(model);
    PRICES.iter().find(|(k, _)| *k == id).map(|(_, p)| *p)
}

fn n(v: Option<&serde_json::Value>) -> f64 {
    v.and_then(serde_json::Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).unwrap_or(0.0)
}

/// What a turn's tokens cost at list price: by each model it used when the harness splits them (every one must be
/// priced), else by the turn's model. None: no tokens, or a model this table doesn't name.
pub fn estimate(stats: &TurnStats, model: &str) -> Option<f64> {
    if let Some(models) = stats.models.as_ref().filter(|m| !m.is_empty()) {
        let mut sum = 0.0;
        let mut any = false;
        for (id, v) in models {
            let (i, r, w, o) = (n(v.get("input")), n(v.get("cacheRead")), n(v.get("cacheWrite")), n(v.get("output")));
            if i + r + w + o <= 0.0 {
                continue;
            }
            let [pi, pr, po] = price(id)?;
            sum += (i * pi + r * pr + w * pi * WRITE + o * po) / 1e6;
            any = true;
        }
        if any {
            return Some(sum);
        }
    }
    #[allow(clippy::cast_precision_loss)] // token counts, far below 2^52
    let (i, c, o) =
        (stats.input_tokens.unwrap_or(0) as f64, stats.cache_tokens.unwrap_or(0) as f64, stats.output_tokens.unwrap_or(0) as f64);
    if i + c + o <= 0.0 {
        return None;
    }
    let [pi, pr, po] = price(model)?;
    Some((i * pi + c * pr + o * po) / 1e6)
}

/// A turn's cost and whether it's an estimate: what the harness reported, else the list-price estimate of its tokens.
pub fn turn_cost(t: &Turn) -> Option<(f64, bool)> {
    let st = t.stats.as_ref()?;
    if let Some(c) = st.cost_usd {
        return Some((c, false));
    }
    estimate(st, &t.model).map(|c| (c, true))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn prices_cover_every_harness_family_and_normalise_ids() {
        assert_eq!(price("claude-fable-5-2[1m]"), Some([8.0, 0.2, 40.0]));
        assert_eq!(price("claude-sonnet-4.5"), price("claude-sonnet-4-5"), "Copilot's dotted id");
        assert_eq!(price("anthropic/claude-opus-4-5-20251101"), Some([5.0, 0.5, 25.0]), "opencode's provider/model");
        for id in ["gpt-5-codex", "gemini-2.5-pro", "grok-4", "composer-1"] {
            assert!(price(id).is_some(), "{id}");
        }
        assert_eq!(price("gpt-9"), None, "an id the table doesn't name is never guessed from its family");
    }

    #[test]
    fn prices_estimate_a_turn_with_tokens_and_no_cost_and_flag_it() {
        let mut t = Turn { model: "gpt-5-codex".into(), ..Turn::default() };
        t.stats = Some(TurnStats {
            input_tokens: Some(1_000_000),
            cache_tokens: Some(2_000_000),
            output_tokens: Some(100_000),
            ..TurnStats::default()
        });
        let (c, est) = turn_cost(&t).unwrap();
        assert!(est, "no cost_usd: an estimate");
        assert!((c - (1.25 + 0.25 + 1.0)).abs() < 1e-9);
        t.stats.as_mut().unwrap().cost_usd = Some(0.4);
        assert_eq!(turn_cost(&t), Some((0.4, false)), "a reported cost wins");
        t.model = "some-new-model".into();
        t.stats.as_mut().unwrap().cost_usd = None;
        assert_eq!(turn_cost(&t), None, "unpriced stays unpriced");
        let st = TurnStats {
            models: Some(BTreeMap::from([("claude-haiku-4-5".to_owned(), serde_json::json!({ "input": 1_000_000, "output": 1_000_000 }))])),
            ..TurnStats::default()
        };
        assert_eq!(estimate(&st, ""), Some(6.0), "per model when the harness splits them");
    }
}
