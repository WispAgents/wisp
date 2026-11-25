//! What a result's stats keep on the stored turn (`land::turn_stats`): the conversation's counters (`stats.totals`),
//! which the next resumed turn starts from, and the cache's reads and writes apart.
#![allow(clippy::unwrap_used)]

use serde_json::json;
use wisp_runtime_api::harness::HarnessStats;
use wisp_runtime_api::model::TurnStats;
use wisp_runtime_core::land::turn_stats;

#[test]
fn a_turn_keeps_its_conversations_totals_and_its_cache_reads_and_writes_apart() {
    let mut hs = HarnessStats { cache_read_tokens: Some(10_000), cache_write_tokens: Some(5_000), ..HarnessStats::default() };
    hs.extra.insert("totals".into(), json!({ "costUsd": 1.5, "models": [] }));
    let st = turn_stats(&hs);
    assert_eq!(st.totals, Some(json!({ "costUsd": 1.5, "models": [] })), "what the next --resume starts from");
    assert_eq!((st.cache_tokens, st.cache_read_tokens, st.cache_write_tokens), (Some(15_000), Some(10_000), Some(5_000)));
    // a turn stored before the fields: it still reads, with none of them
    let old: TurnStats = serde_json::from_value(json!({ "costUsd": 0.1, "cacheTokens": 7 })).unwrap();
    assert_eq!((old.totals, old.cache_read_tokens, old.cache_tokens), (None, None, Some(7)));
    assert!(!serde_json::to_string(&TurnStats::default()).unwrap().contains("totals"), "absent, not null");
}
