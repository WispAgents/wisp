//! v1 codex.js `turnUsage`: `turn.completed`'s usage is the thread's running total, not the turn's
//! (`exec resume` and `exec fork` start from its saved counters). `prior` (the last turn's
//! `stats.totals`) is taken off unless the counters are behind it (they started over). A turn that
//! ended with no usage passes `prior` on. With no `prior` and `unknown_prior` (a thread wisp never counted: adopted,
//! brought in, restored, or stored before turns kept their totals) the turn's own counters are the baseline: it counts
//! as 0 and keeps its totals for the next turn. Unlike v1, a turn's `inputTokens` leave out its cached input, which goes
//! in `cacheReadTokens` (the provider's input includes it); `totals` keep the CLI's own counters.

use serde_json::{Map, Value, json};
use wisp_harness_kit::js::json_num;

fn n(v: &Value) -> f64 {
    v.as_f64().filter(|x| x.is_finite() && *x > 0.0).unwrap_or(0.0)
}

/// The stats keys a turn's usage adds (`inputTokens`, `cachedInputTokens`, `outputTokens`, `totals`).
pub fn turn_usage(usage: Option<&Value>, prior: Option<&Value>, unknown_prior: bool, thread: &str) -> Map<String, Value> {
    let mut out = Map::new();
    let prior = prior.filter(|p| !p.is_null());
    let Some(u) = usage.filter(|u| !u.is_null()) else {
        if let Some(p) = prior {
            out.insert("totals".into(), p.clone());
        }
        return out;
    };
    let (input, cached, output) = (n(&u["input_tokens"]), n(&u["cached_input_tokens"]), n(&u["output_tokens"]));
    // the thread's counters before this turn are unknown: these are the baseline, not this turn's usage
    let base = if prior.is_none() && unknown_prior {
        json!({ "inputTokens": json_num(input), "cachedInputTokens": json_num(cached), "outputTokens": json_num(output) })
    } else {
        json!({})
    };
    let from = prior.filter(|p| input >= n(&p["inputTokens"]) && output >= n(&p["outputTokens"])).unwrap_or(&base);
    let left = (cached - n(&from["cachedInputTokens"])).max(0.0);
    let read = input - n(&from["inputTokens"]);
    // OpenAI's input counts its cached part too: the turn's input is stored without it, and the cached part as cache
    // reads (`cacheReadTokens`), so a usage total (input + reads + writes) counts it once and its cache share is right
    let hit = left.min(read);
    out.insert("inputTokens".into(), json_num(read - hit));
    if hit != 0.0 {
        out.insert("cachedInputTokens".into(), json_num(hit));
        out.insert("cacheReadTokens".into(), json_num(hit));
    }
    out.insert("outputTokens".into(), json_num(output - n(&from["outputTokens"])));
    out.insert("totals".into(), json!({ "inputTokens": json_num(input), "cachedInputTokens": json_num(cached), "outputTokens": json_num(output), "thread": thread }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prior_comes_off_unless_the_counters_started_over() {
        let u = json!({ "input_tokens": 30, "cached_input_tokens": 6, "output_tokens": 15 });
        let got = turn_usage(Some(&u), Some(&json!({ "inputTokens": 20, "cachedInputTokens": 4, "outputTokens": 10 })), false, "t");
        assert_eq!(
            (got["inputTokens"].as_f64(), got["cachedInputTokens"].as_f64(), got["outputTokens"].as_f64()),
            (Some(8.0), Some(2.0), Some(5.0)),
            "10 in, 2 of them cached: 8 and 2 reads"
        );
        assert_eq!(got["cacheReadTokens"].as_f64(), Some(2.0));
        assert_eq!(got["totals"]["inputTokens"].as_f64(), Some(30.0), "the CLI's own counters, for the next resume");
        let behind = turn_usage(Some(&u), Some(&json!({ "inputTokens": 100, "outputTokens": 50 })), false, "t");
        assert_eq!(behind["inputTokens"].as_f64(), Some(24.0));
        assert_eq!(turn_usage(None, Some(&json!({ "inputTokens": 1 })), false, "t")["totals"]["inputTokens"], 1);
        assert!(turn_usage(None, None, false, "t").is_empty());
    }

    #[test]
    fn a_resumed_turn_counts_only_its_own_input() {
        let prior = json!({ "inputTokens": 1000, "cachedInputTokens": 0, "outputTokens": 100 });
        let u = json!({ "input_tokens": 1600, "cached_input_tokens": 0, "output_tokens": 150 });
        let got = turn_usage(Some(&u), Some(&prior), false, "t");
        assert_eq!((got["inputTokens"].as_f64(), got["outputTokens"].as_f64()), (Some(600.0), Some(50.0)));
    }

    #[test]
    fn an_uncounted_thread_takes_its_first_result_as_the_baseline() {
        let u = json!({ "input_tokens": 50_000, "cached_input_tokens": 40_000, "output_tokens": 3000 });
        let got = turn_usage(Some(&u), None, true, "t");
        assert_eq!(
            (got["inputTokens"].as_f64(), got.get("cacheReadTokens"), got["outputTokens"].as_f64()),
            (Some(0.0), None, Some(0.0)),
            "the whole thread's counters are not this turn's usage"
        );
        assert_eq!(got["totals"]["inputTokens"].as_f64(), Some(50_000.0), "kept as the next turn's prior");
        assert_eq!(got["totals"]["outputTokens"].as_f64(), Some(3000.0));
        let next = json!({ "input_tokens": 50_600, "cached_input_tokens": 40_400, "output_tokens": 3100 });
        let second = turn_usage(Some(&next), Some(&got["totals"]), false, "t");
        assert_eq!(
            (second["inputTokens"].as_f64(), second["cacheReadTokens"].as_f64(), second["outputTokens"].as_f64()),
            (Some(200.0), Some(400.0), Some(100.0))
        );
        let known = turn_usage(Some(&u), Some(&json!({ "inputTokens": 1000, "outputTokens": 100 })), true, "t");
        assert_eq!(known["outputTokens"].as_f64(), Some(2900.0), "a known prior wins");
        assert_eq!(turn_usage(Some(&u), None, false, "t")["outputTokens"].as_f64(), Some(3000.0), "a fresh thread counts from zero");
    }
}
