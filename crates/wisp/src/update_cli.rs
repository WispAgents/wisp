//! `wisp update [apply]` (QA round 11 #1; v1 server/update.js's check and install, from the terminal): over
//! wisp.update's `update.check` and `update.install`. `wisp update check|install|view` are that plugin's verbs.
use serde_json::{Value, json};

use crate::cli::print_result;
use crate::client::{Client, Result};
use crate::scripted::say;

/// What a check found, in a line, and the exit code (1: it couldn't check).
fn said(v: &Value) -> (String, i32) {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    if !s("error").is_empty() {
        return (format!("  ✗ couldn't check for an update: {}", s("error")), 1);
    }
    if v["available"] == json!(true) {
        let latest = if s("latest").is_empty() { "a newer wisp" } else { s("latest") };
        return (format!("  ◌ wisp {latest} is out (this is {}): wisp update apply", s("current")), 0);
    }
    (format!("  ✓ wisp {} is the latest", s("current")), 0)
}

/// `wisp update`: is there a newer wisp.
pub async fn check(c: &Client, json: bool) -> Result<i32> {
    let v = c.cmd("update.check", json!({})).await?;
    if json {
        print_result(&v, true);
        return Ok(0);
    }
    let (line, code) = said(&v);
    say(&line);
    Ok(code)
}

/// `wisp update apply`: install it (npm or Homebrew, as wisp was installed).
pub async fn apply(c: &Client, json: bool) -> Result<i32> {
    print_result(&c.cmd("update.install", json!({})).await?, json);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_says_whether_there_is_a_newer_wisp_and_how_to_get_it() {
        assert_eq!(said(&json!({ "current": "2.0.0", "available": false })), ("  ✓ wisp 2.0.0 is the latest".into(), 0));
        assert_eq!(
            said(&json!({ "current": "2.0.0", "latest": "2.1.0", "available": true })),
            ("  ◌ wisp 2.1.0 is out (this is 2.0.0): wisp update apply".into(), 0)
        );
        assert_eq!(said(&json!({ "current": "2.0.0", "error": "offline" })), ("  ✗ couldn't check for an update: offline".into(), 1));
    }
}
