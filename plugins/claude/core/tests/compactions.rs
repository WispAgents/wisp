//! ADR-0096: each main-thread `compact_boundary` of a turn is counted in its result's `stats.compactions` (a
//! /compact, or Claude Code compacting on its own mid-turn), which long threads count and audit. v1 didn't report it,
//! so gate G6 drops that key from these fixtures (`meta.drop`) and checks the rest as v1 parsed it.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use serde_json::Value;
use wisp_claude_core::{ClaudeOptions, ClaudeParser};
use wisp_harness_kit::{Exit, HarnessEvent, NoDisk, RunPaths, replay};

const CWD: &str = "/tmp/wisp-compactions-run";

fn results(name: &str) -> Vec<Value> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let stdout = std::fs::read_to_string(dir.join(format!("{name}.jsonl"))).unwrap().replace("@CWD@", CWD);
    let mut parser = ClaudeParser::new(ClaudeOptions { paths: RunPaths::same(CWD), prior: None, unknown_prior: false });
    let lines: Vec<&str> = stdout.lines().collect();
    let exit = Exit { code: Some(0), ..Exit::default() };
    let events = replay(&mut parser, &lines, &exit, &mut NoDisk, |i| i64::try_from(i).unwrap_or(0));
    events.iter().map(HarnessEvent::to_json).filter(|e| e["type"] == "result").collect()
}

#[test]
fn a_turns_compactions_are_counted_in_its_stats() {
    for name in ["compact", "compact-real", "context-compact", "context-compact-more", "context-compact-sub"] {
        let r = results(name);
        assert_eq!(r.last().unwrap()["stats"]["compactions"], 1, "{name}: {:?}", r.last().unwrap()["stats"]);
    }
    let none = results("compact-none");
    assert!(none.last().unwrap()["stats"].get("compactions").is_none(), "nothing compacted: no count");
}
