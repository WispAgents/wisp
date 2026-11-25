//! Audit fork-turn-includes-later: Claude Code shares one session id across resumed turns, so a fork from an earlier
//! turn resumed the whole conversation, later turns included. Each main-thread assistant message says where the
//! conversation is (`Session.at`, its uuid), so a turn ends knowing its own resume point; a subagent's message doesn't.
#![allow(clippy::unwrap_used)]

use wisp_claude_core::{ClaudeOptions, ClaudeParser};
use wisp_harness_kit::{Exit, HarnessEvent, NoDisk, RunPaths, replay};

fn points(lines: &[&str]) -> Vec<Option<String>> {
    let mut parser = ClaudeParser::new(ClaudeOptions { paths: RunPaths::same("/tmp/wisp-at"), prior: None, unknown_prior: false });
    let exit = Exit { code: Some(0), signal: None, stderr: String::new(), stopped: false, stop_requested: false };
    replay(&mut parser, lines, &exit, &mut NoDisk, |i| i64::try_from(i).unwrap_or(0))
        .into_iter()
        .filter_map(|e| match e {
            HarnessEvent::Session { at, .. } => Some(at),
            _ => None,
        })
        .collect()
}

#[test]
fn a_main_thread_assistant_message_says_where_the_conversation_is_and_a_subagents_doesnt() {
    let init = r#"{"type":"system","subtype":"init","session_id":"s1","uuid":"u-init"}"#;
    let main = r#"{"type":"assistant","session_id":"s1","uuid":"u-main","parent_tool_use_id":null,"message":{"content":[{"type":"text","text":"hi"}]}}"#;
    let sub = r#"{"type":"assistant","session_id":"s1","uuid":"u-sub","parent_tool_use_id":"toolu_1","message":{"content":[{"type":"text","text":"sub"}]}}"#;
    let user = r#"{"type":"user","session_id":"s1","uuid":"u-user","message":{"content":[{"type":"text","text":"x"}]}}"#;
    assert_eq!(points(&[init, main, sub, user]), vec![None, Some("u-main".to_owned()), None, None]);
}
