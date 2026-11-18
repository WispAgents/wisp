//! The curated verbs that read and steer tasks (ls, show, wait, reply, btw), on v2's commands
//! (`runtime.*` over `POST /api/cmd`) and topics: no v1 route (ADR-0031).
use std::fmt::Write as _;
use std::io::Write;
use std::time::Duration;

use serde_json::{Value, json};

use crate::args::{Flags, LS_STATES};
use crate::client::{CliError, Client, Result};

/// The one place the CLI writes to stdout (libraries never do: plan §9.2).
pub fn say(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
}

pub fn out(flags: &Flags, v: &Value, text: &str) {
    if flags.json {
        say(&serde_json::to_string_pretty(v).unwrap_or_default());
    } else {
        say(text);
    }
}

pub fn need<'a>(v: Option<&'a String>, what: &str) -> Result<&'a str> {
    v.map(String::as_str).filter(|s| !s.trim().is_empty()).ok_or_else(|| CliError::said(format!("{what} required — see wisp --help")))
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// `runtime.create`'s args for a launch from the command line (the `default` profile fills what's left).
pub fn launch_args(f: &Flags, prompt: &str, cwd: &str) -> Value {
    let also: Vec<String> = f.also.iter().map(|p| p.display().to_string()).collect();
    let mut a = json!({ "prompt": prompt, "cwd": cwd });
    for (k, v) in [("agent", &f.agent), ("model", &f.model), ("effort", &f.effort), ("mode", &f.mode)] {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty() && *v != "auto") {
            a[k] = json!(v);
        }
    }
    if !also.is_empty() {
        a["dirs"] = json!(also);
    }
    if let Some(w) = f.worktree {
        a["worktree"] = json!(w);
    }
    // --yes: go past Ask near the limit's line (the user agreed)
    if f.yes {
        a["confirmLimit"] = json!(true);
    }
    a
}

/// Setup audit F4: a task started from a project's folder is that project's, as New task's is. Its cwd is spelled as
/// the project's path (the shell's `/private/var/…` is the project's `/var/…`, so the app files it under the project),
/// and it gets its own worktree when Worktree per task (`worktrees.worktree`) is on, unless `--worktree` or
/// `--no-worktree` said. Outside every project the arguments stay as they are.
pub async fn in_project(c: &Client, a: &mut Value) {
    let Some(cwd) = a.get("cwd").and_then(Value::as_str).map(str::to_owned) else { return };
    let Ok(projects) = c.cmd("resources.export", json!({ "kinds": ["project"] })).await else { return };
    let mut best: Option<(usize, String)> = None;
    for p in projects.as_array().into_iter().flatten() {
        let Some(path) = p.pointer("/spec/path").and_then(Value::as_str) else { continue };
        let root = path.trim_end_matches('/');
        for here in [Some(cwd.as_str()), cwd.strip_prefix("/private")].into_iter().flatten() {
            // the subfolder as this spelling has it (`is_within` may match `/private/var/…` to `/var/…` too)
            let Some(rest) = here.strip_prefix(root).filter(|r| r.is_empty() || r.starts_with('/')) else { continue };
            if !wisp_runtime_api::paths::is_within(here, root) {
                continue;
            }
            if best.as_ref().is_none_or(|(n, _)| root.len() > *n) {
                best = Some((root.len(), format!("{root}{rest}")));
            }
        }
    }
    let Some((_, spelled)) = best else { return };
    a["cwd"] = json!(spelled);
    if a.get("worktree").is_none() {
        let on = c.cmd("settings.get", json!({ "key": "worktrees.worktree" })).await.ok().is_none_or(|v| {
            let v = v.get("value").unwrap_or(&v);
            !(v == "off" || *v == json!(false))
        });
        a["worktree"] = json!(on);
    }
}

/// Until the task needs you (it isn't running), from its topic: no polling.
pub async fn wait(c: &Client, id: &str, timeout: Duration) -> Result<Value> {
    let done = crate::attach::until(c, id, |s| !matches!(s.get("state").and_then(Value::as_str), Some("running") | None));
    match tokio::time::timeout(timeout, done).await {
        Ok(v) => v,
        Err(_) => Ok(json!({ "id": id, "timedOut": true })),
    }
}

/// The task a command names: `last` is the newest one here; an id is checked, so a task that isn't there is said at
/// once instead of waited for (QA round 14 #7: `wisp attach last` hung).
pub async fn task_id(c: &Client, w: &str) -> Result<String> {
    if w != "last" {
        c.cmd("runtime.get", json!({ "id": w })).await?;
        return Ok(w.to_owned());
    }
    newest(c).await?.ok_or_else(|| CliError::said("no tasks yet: wisp \"<what to do>\" starts one"))
}

/// The newest task made (not archived), reading the list a page at a time, newest changed first (perf audit 2026-11-18
/// #10: it read every task, 2.3 MB at 5,000): a task changes when it's made, so once a page's oldest change is before
/// the newest making seen so far, no task further down was made later.
async fn newest(c: &Client) -> Result<Option<String>> {
    let made = |t: &Value| t.get("createdAt").or_else(|| t.get("created_at")).and_then(Value::as_i64).unwrap_or(0);
    let changed = |t: &Value| t.get("updatedAt").or_else(|| t.get("updated_at")).and_then(Value::as_i64).unwrap_or(0);
    let mut limit = 50u32;
    loop {
        let list = c.cmd("runtime.list", json!({ "archived": false, "limit": limit })).await?;
        let rows = list.as_array().cloned().unwrap_or_default();
        // made in the same millisecond: the ids are time-ordered (UUIDv7), the later one is newer
        let best = rows.iter().max_by_key(|t| (made(t), t.get("id").and_then(Value::as_str).unwrap_or_default().to_owned()));
        let short = rows.len() < limit as usize;
        let settled = best.is_some_and(|b| rows.last().is_some_and(|l| changed(l) < made(b)));
        if short || settled || limit >= 1 << 20 {
            return Ok(best.and_then(|t| t.get("id").and_then(Value::as_str)).map(str::to_owned));
        }
        limit *= 4;
    }
}

/// `wisp show <id>`: its state and its last answer, drawn for the terminal (`--json`: the task and the
/// presentation).
pub async fn show(c: &Client, f: &Flags, id: &str, summary: Option<Value>) -> Result<()> {
    let task = c.cmd("runtime.get", json!({ "id": id })).await?;
    let p = c.cmd("runtime.presentation", json!({ "id": id })).await.unwrap_or(Value::Null);
    let still =
        if summary.as_ref().and_then(|x| x.get("timedOut")).and_then(Value::as_bool) == Some(true) { ", still running" } else { "" };
    if f.json {
        let mut all = task.clone();
        if let Some(o) = all.as_object_mut() {
            o.insert("presentation".into(), p);
        }
        say(&serde_json::to_string_pretty(&all).unwrap_or_default());
        return Ok(());
    }
    let st = crate::cli::style();
    let family = crate::history_cli::lineage_line(c, id).await.map_or_else(String::new, |l| format!("\n{l}"));
    let blocks = p.get("blocks").and_then(Value::as_array).cloned().unwrap_or_default();
    let body = if blocks.is_empty() { String::new() } else { wisp_cli_core::Renderers::builtin().answer(&blocks, &st).join("\n") };
    let asks = asking(&task);
    say(&format!("◌ {}  [{}{still}]  {id}{family}\n\n{body}{asks}", s(&task, "title"), s(&task, "state")));
    Ok(())
}

/// The question the task waits on (an `ask_user`, a live approval), its context and options, and how to answer it from
/// here: a CLI user sees what the page shows (orch #11). '' when it asks nothing.
pub fn asking(task: &Value) -> String {
    let Some(ask) = task.pointer("/ext/wisp.runtime/ask") else { return String::new() };
    let q = ask.get("question").and_then(Value::as_str).unwrap_or_default().trim();
    if q.is_empty() {
        return String::new();
    }
    let head = if ask.get("permission").is_some() { "Needs your OK" } else { "Asks" };
    let mut out = format!("\n\n? {head}: {q}");
    if let Some(ctx) = ask.get("context").and_then(Value::as_str).map(str::trim).filter(|c| !c.is_empty()) {
        let _ = write!(out, "\n  {ctx}");
    }
    for (i, o) in ask.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).enumerate() {
        let _ = write!(out, "\n  {}. {o}", i + 1);
    }
    let id = task.get("id").and_then(Value::as_str).unwrap_or_default();
    let _ = write!(out, "\n  Answer: wisp reply {id} \"<your answer>\"");
    out
}

pub async fn ls(c: &Client, f: &Flags) -> Result<()> {
    let mut states: Vec<String> = Vec::new();
    for x in f.state.iter().flat_map(|st| st.split(',')).map(str::trim).filter(|x| !x.is_empty()) {
        let add: &[&str] = if x == "needs" { &["review", "waiting"] } else { &[x] };
        for a in add {
            if !states.iter().any(|y| y == a) {
                states.push((*a).to_owned());
            }
        }
    }
    if let Some(bad) =
        Some(states.iter().filter(|x| !LS_STATES.contains(&x.as_str())).cloned().collect::<Vec<_>>()).filter(|b| !b.is_empty())
    {
        return Err(CliError::said(format!("unknown state {}: use {} or needs (review + waiting)", bad.join(", "), LS_STATES.join(", "))));
    }
    // every task, archived ones too (`--all` keeps them below); `{archived: true}` is the archived ones only, which made
    // `ls --all` say "No sessions." with tasks there (QA round 13 #2)
    // what the server can narrow it reads there (perf audit 2026-11-18 #10, as v1 asked): live tasks unless `--all`, and
    // only `--limit` of them when no state or folder filter comes after
    let mut ask = serde_json::Map::new();
    if !f.all {
        ask.insert("archived".into(), json!(false));
    }
    if let Some(l) = f.limit.filter(|_| states.is_empty() && f.cwd.is_none()) {
        ask.insert("limit".into(), json!(l));
    }
    let list = c.cmd("runtime.list", Value::Object(ask)).await?;
    let cwd = f.cwd.as_ref().map(|p| p.display().to_string());
    let rows: Vec<&Value> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| states.is_empty() || states.iter().any(|st| st == s(x, "state")))
        .filter(|x| cwd.as_ref().is_none_or(|c| s(x, "cwd") == c.as_str()))
        .filter(|x| f.all || x.get("archived") != Some(&json!(true)))
        .take(f.limit.map_or(usize::MAX, |l| l as usize))
        .collect();
    let lines: Vec<String> = rows
        .iter()
        .map(|x| {
            let ask =
                x.get("question").and_then(Value::as_str).filter(|q| !q.is_empty()).map_or(String::new(), |q| format!("  — asks: {q}"));
            let proj = std::path::Path::new(s(x, "cwd")).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            format!(
                "{}  {:<7}  {:<8}  {:<16}  {}{ask}",
                s(x, "id"),
                s(x, "state"),
                s(x, "agent"),
                proj.chars().take(16).collect::<String>(),
                s(x, "title")
            )
        })
        .collect();
    out(f, &json!(rows), &if lines.is_empty() { "No sessions.".to_owned() } else { lines.join("\n") });
    Ok(())
}

#[allow(clippy::many_single_char_names)]
pub async fn reply(c: &Client, f: &Flags, args: &[String]) -> Result<()> {
    let id = need(args.first(), "session id")?;
    let text = args[1..].join(" ");
    let text = need(Some(&text), "text")?;
    let mut a = json!({ "id": id, "text": text });
    if f.yes {
        a["confirmLimit"] = json!(true);
    }
    if let Some(q) = f.queue {
        a["queue"] = json!(q);
    }
    if let Some(m) = &f.mode {
        a["mode"] = json!(m);
    }
    let pick: serde_json::Map<String, Value> = [("agent", &f.agent), ("model", &f.model), ("effort", &f.effort)]
        .into_iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| (k.to_owned(), json!(v))))
        .collect();
    if !pick.is_empty() {
        a["run"] = Value::Object(pick);
    }
    let r = c.cmd("runtime.followUp", a).await?;
    if !f.wait {
        let said = if r.get("queued").is_some_and(|v| !v.is_null() && *v != json!(false)) { "  ✓ queued" } else { "  ✓ sent" };
        out(f, &r, said);
        return Ok(());
    }
    let w = wait(c, id, Duration::from_secs(30 * 60)).await?;
    show(c, f, id, Some(w)).await
}

pub fn btw_out(f: &Flags, b: &Value) {
    let text = if b.get("pending") == Some(&json!(true)) {
        "  … still answering; it will show on the card".to_owned()
    } else {
        s(b, "answer").to_owned()
    };
    out(f, b, &text);
}

/// The side answer, once it's in (the task's `ext["wisp.runtime"].btw`, from its topic; at most 6 minutes).
pub async fn btw_answer(c: &Client, id: &str, btw: &str) -> Result<Value> {
    let entry = |t: &Value| {
        t.pointer("/ext/wisp.runtime/btw").and_then(Value::as_array).and_then(|a| a.iter().find(|x| s(x, "id") == btw)).cloned()
    };
    let done = crate::attach::until(c, id, |t| entry(t).is_some_and(|b| b.get("pending") != Some(&json!(true))));
    match tokio::time::timeout(Duration::from_secs(6 * 60), done).await {
        Ok(t) => Ok(entry(&t?).unwrap_or_else(|| json!({ "id": btw, "answer": "", "error": "It was dismissed." }))),
        Err(_) => Ok(json!({ "id": btw, "answer": "", "pending": true })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_args_leave_the_rest_to_the_default_profile() {
        let mut f = Flags::defaults(None);
        assert_eq!(launch_args(&f, "fix it", "/w"), json!({ "prompt": "fix it", "cwd": "/w" }));
        f.agent = Some("reviewer".into());
        f.worktree = Some(false);
        assert_eq!(launch_args(&f, "x", "/w"), json!({ "prompt": "x", "cwd": "/w", "agent": "reviewer", "worktree": false }));
    }

    /// orch #11: `wisp show` / `wisp wait` say what a waiting task asks, with its options and how to answer.
    #[test]
    fn a_waiting_tasks_question_is_shown_with_its_options() {
        let task = json!({ "id": "t1", "state": "waiting", "ext": { "wisp.runtime": { "ask": { "id": "a", "question": "Which database?", "context": "Both work.", "options": ["Postgres", "SQLite"] } } } });
        let out = asking(&task);
        assert!(out.contains("? Asks: Which database?") && out.contains("Both work."), "{out}");
        assert!(out.contains("1. Postgres") && out.contains("2. SQLite") && out.contains("wisp reply t1"), "{out}");
        let ok = json!({ "id": "t2", "ext": { "wisp.runtime": { "ask": { "id": "b", "question": "Allow npm publish?", "permission": { "tool": "Bash" } } } } });
        assert!(asking(&ok).contains("? Needs your OK: Allow npm publish?"));
        assert_eq!(asking(&json!({ "id": "t3", "ext": {} })), "", "nothing asked: nothing said");
    }
}
