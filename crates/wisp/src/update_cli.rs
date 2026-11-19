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
    // never "the latest" without a check (audit F setup F5): a git clone doesn't check, nor a copy whose checks are
    // off; Settings → Updates says the same
    if s("method") == "git" {
        return (format!("  · wisp {} is a git clone: pull and restart to update", s("current")), 0);
    }
    if v["enabled"] == json!(false) {
        return (format!("  · wisp {}: update checks are off on this install, so it wasn't checked", s("current")), 0);
    }
    if v.get("checked_at").is_none_or(Value::is_null) {
        return (format!("  · wisp {}: not checked yet", s("current")), 0);
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
        assert_eq!(
            said(&json!({ "current": "2.0.0", "available": false, "enabled": true, "checked_at": 1 })),
            ("  ✓ wisp 2.0.0 is the latest".into(), 0)
        );
        assert_eq!(
            said(&json!({ "current": "2.0.0", "latest": "2.1.0", "available": true })),
            ("  ◌ wisp 2.1.0 is out (this is 2.0.0): wisp update apply".into(), 0)
        );
        assert_eq!(said(&json!({ "current": "2.0.0", "error": "offline" })), ("  ✗ couldn't check for an update: offline".into(), 1));
    }

    /// Audit F setup F5: on a git clone (or with checks off) `wisp update` said "✓ wisp 2.0.0 is the latest" while
    /// `--json` had `enabled false`, `checked_at null`, `latest null`, and Settings → Updates said checks are off.
    #[test]
    fn update_never_says_latest_without_a_check_audit_f_setup_f5() {
        let clone =
            said(&json!({ "current": "2.0.0", "available": false, "enabled": false, "method": "git", "checked_at": null, "latest": null }));
        assert_eq!(clone, ("  · wisp 2.0.0 is a git clone: pull and restart to update".into(), 0));
        let off = said(&json!({ "current": "2.0.0", "available": false, "enabled": false, "method": "npm", "checked_at": null }));
        assert!(off.0.contains("checks are off") && !off.0.contains("latest"), "{off:?}");
        let never = said(&json!({ "current": "2.0.0", "available": false, "enabled": true, "method": "npm", "checked_at": null }));
        assert!(never.0.contains("not checked yet") && !never.0.contains("latest"), "{never:?}");
    }
}
