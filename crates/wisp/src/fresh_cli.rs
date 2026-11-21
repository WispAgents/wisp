//! `wisp fresh [<task>] [--print] [-f brief.md | -f -] [--json]` (ADR-0096): a new conversation on the same task when
//! the old one got long. `--print` shows the brief it would start from (the Notes, the files changed, the last turns)
//! so you can edit it; `-f` starts from your edited brief (`-`: from stdin); without either, from the brief as made.
//! With no task: the task this shell runs in (`WISP_TASK`). `-d`/`--detach` is taken, as launches take it: it never waits.
use std::io::Read as _;

use serde_json::{Value, json};

use crate::client::{CliError, Client, Result};
use crate::code::Opts;
use crate::scripted::say;

/// The task named in the words after `fresh`, else the shell's.
pub fn task_of(words: &[String], env_task: Option<&str>) -> std::result::Result<String, String> {
    match words {
        [] => env_task.filter(|t| !t.is_empty()).map(str::to_owned).ok_or_else(|| "which task? wisp fresh <task>".to_owned()),
        [t] => Ok(t.clone()),
        [_, w, ..] => Err(format!("unexpected {w:?} (wisp fresh <task> [--print] [-f brief.md])")),
    }
}

fn brief_from(path: &str) -> Result<String> {
    let mut s = String::new();
    if path == "-" {
        std::io::stdin().read_to_string(&mut s).map_err(|e| CliError::said(format!("stdin: {e}")))?;
    } else {
        s = std::fs::read_to_string(path).map_err(|e| CliError::said(format!("{path}: {e}")))?;
    }
    Ok(s)
}

/// What the terminal says after it started.
pub fn started(out: &Value, task: &str) -> String {
    match (out.get("turnId").and_then(Value::as_str), out.get("queued").and_then(Value::as_str)) {
        (Some(t), _) => format!("Fresh thread started on {task} (turn {t}). Follow it: wisp attach {task}"),
        (None, Some(_)) => format!("Fresh thread queued on {task}: it starts after the turn running now."),
        _ => format!("Fresh thread asked for on {task}."),
    }
}

pub async fn run(c: &Client, o: &Opts) -> Result<i32> {
    let task = task_of(&o.words, std::env::var("WISP_TASK").ok().as_deref()).map_err(CliError::said)?;
    if o.print {
        let b = c.cmd("threads.freshBrief", json!({ "id": task })).await?;
        say(&if o.json { b.to_string() } else { b.get("text").and_then(Value::as_str).unwrap_or_default().to_owned() });
        return Ok(0);
    }
    let mut args = json!({ "id": task });
    if let Some(f) = o.files.first() {
        args["brief"] = json!(brief_from(f)?);
    }
    let out = c.cmd("threads.fresh", args).await?;
    say(&if o.json { out.to_string() } else { started(&out, &task) });
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_takes_one_task_or_the_shells() {
        assert_eq!(task_of(&["abc123def456".into()], None).unwrap(), "abc123def456");
        assert_eq!(task_of(&[], Some("t9")).unwrap(), "t9");
        assert!(task_of(&[], None).is_err());
        assert!(task_of(&["a".into(), "b".into()], None).is_err());
        assert!(started(&json!({ "turnId": "u1" }), "t1").contains("wisp attach t1"));
        assert!(started(&json!({ "queued": "q1" }), "t1").contains("queued"));
    }
}
