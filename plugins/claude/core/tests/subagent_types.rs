//! v1 #785: each turn keeps Claude Code's own subagents (its Agent/Task runs) by type, with what they read and wrote,
//! so the task usage card can list them. Replayed from a captured stream (`fixtures/subagents.jsonl`: two background
//! web-fetch agents, `task_started` naming their type, their messages carrying `parent_tool_use_id`).
#![allow(clippy::unwrap_used)]

use serde_json::{Value, json};
use wisp_claude_core::{ClaudeOptions, ClaudeParser};
use wisp_harness_kit::{Exit, HarnessEvent, NoDisk, RunPaths, replay};

fn subagents(lines: &[&str]) -> Vec<Option<Value>> {
    let mut parser =
        ClaudeParser::new(ClaudeOptions { paths: RunPaths::same("/tmp/wisp-subagent-types"), prior: None, unknown_prior: false });
    let exit = Exit { code: Some(0), signal: None, stderr: String::new(), stopped: false, stop_requested: false };
    replay(&mut parser, lines, &exit, &mut NoDisk, |i| i64::try_from(i).unwrap_or(0))
        .into_iter()
        .filter_map(|e| match e {
            HarnessEvent::Result(r) => Some(r.stats.and_then(|s| s.extra.get("subagents").cloned())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_turn_keeps_claude_codes_subagents_by_type_from_the_captured_stream_v1_785() {
    let lines: Vec<&str> = include_str!("fixtures/subagents.jsonl").lines().collect();
    // two runs of one type: one row, their reads summed, their output from modelUsage past the main thread's
    let rows = subagents(&lines);
    assert_eq!(
        rows,
        vec![Some(
            json!([{ "type": "web-fetch", "calls": 2, "read": 2100, "output": 27000, "model": "claude-opus-5-5", "same": true, "instead": "WebFetch" }])
        )]
    );
    // a turn that ran no subagent says none (not an empty list)
    let init = r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-opus-5-5","tools":["Read"]}"#;
    let result = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"s1","usage":{"output_tokens":5}}"#;
    assert_eq!(subagents(&[init, result]), vec![None]);
}

/// TRY-24: a call a subagent made carries that subagent's call as its `parent` (its row's current step); the main
/// thread's calls carry none. v1 had no such field (the G6 goldens drop it).
#[test]
fn a_subagents_own_calls_name_it_as_their_parent() {
    let lines: Vec<&str> = include_str!("fixtures/subagents.jsonl").lines().collect();
    let mut parser =
        ClaudeParser::new(ClaudeOptions { paths: RunPaths::same("/tmp/wisp-subagent-types"), prior: None, unknown_prior: false });
    let exit = Exit { code: Some(0), signal: None, stderr: String::new(), stopped: false, stop_requested: false };
    let tools: Vec<(Option<String>, Option<String>)> = replay(&mut parser, &lines, &exit, &mut NoDisk, |i| i64::try_from(i).unwrap_or(0))
        .into_iter()
        .filter_map(|e| match e {
            HarnessEvent::Tool(t) => Some((t.id, t.parent)),
            _ => None,
        })
        .collect();
    let nested: Vec<_> = tools.iter().filter(|(_, p)| p.is_some()).collect();
    assert!(!nested.is_empty(), "{tools:?}");
    // every parent is one of the run's Agent calls, made before it
    for (i, (_, p)) in tools.iter().enumerate().filter(|(_, (_, p))| p.is_some()) {
        assert!(tools[..i].iter().any(|(id, _)| id == p), "{p:?} at #{i} names no earlier call: {tools:?}");
    }
    assert!(tools.iter().any(|(_, p)| p.is_none()), "the main thread's calls have no parent");
}
