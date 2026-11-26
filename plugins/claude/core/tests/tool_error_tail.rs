//! Audit F2: a failed call's error kept only its first 400 characters, so a test run's or a build's failure lost the
//! end that says why. The parser passes it whole; the runtime keeps its start and end (`clip_output`).
#![allow(clippy::unwrap_used)]

use wisp_claude_core::{ClaudeOptions, ClaudeParser};
use wisp_harness_kit::{Exit, HarnessEvent, NoDisk, RunPaths, replay};

#[test]
fn a_long_failed_tool_result_keeps_its_last_line() {
    let body = format!("{}assertion failed: TAIL_MARKER", "running test …\n".repeat(350));
    let call = r#"{"type":"assistant","session_id":"s1","uuid":"u1","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cargo test"}}]}}"#;
    let result = serde_json::json!({"type":"user","session_id":"s1","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":true,"content":body}]}}).to_string();
    let mut parser = ClaudeParser::new(ClaudeOptions { paths: RunPaths::same("/tmp/wisp-f2"), prior: None, unknown_prior: false });
    let exit = Exit { code: Some(0), signal: None, stderr: String::new(), stopped: false, stop_requested: false };
    let evs = replay(&mut parser, &[call, &result], &exit, &mut NoDisk, |i| i64::try_from(i).unwrap_or(0));
    let (text, id) = evs
        .iter()
        .find_map(|e| if let HarnessEvent::ToolError { text, id, .. } = e { Some((text.clone(), id.clone())) } else { None })
        .unwrap();
    assert!(text.len() > 4000 && text.ends_with("assertion failed: TAIL_MARKER"), "{}", text.len());
    assert_eq!(id.as_deref(), Some("toolu_1"));
}
