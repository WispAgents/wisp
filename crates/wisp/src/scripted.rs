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

/// A word the command can't run without (`wisp show` with no id): a usage error, exit 2, never a refusal's 1.
pub fn need<'a>(v: Option<&'a String>, what: &str) -> Result<&'a str> {
    v.map(String::as_str).filter(|s| !s.trim().is_empty()).ok_or_else(|| CliError::usage(format!("{what} required — see wisp --help")))
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Setup audit B10: `wisp "x" --agent frob` made a task that ended Blocked "no harness frob". What the CLI says instead,
/// before anything starts: None when `--agent` is a harness here, an agent profile, `auto`, or not given (or the lists
/// can't be read: the server's own check stands).
pub async fn unknown_agent(c: &Client, agent: Option<&str>) -> Option<String> {
    // `--agent auto` with Auto off ran the default harness silently (audit I9): said, as a crew member's is (HS7)
    if agent.map(str::trim) == Some("auto") {
        let on = c.cmd("settings.get", json!({ "key": "runtime.auto" })).await.is_ok_and(|v| v == json!(true) || v["value"] == json!(true));
        return (!on).then(|| {
            "  ✗ Auto is off: name a harness with --agent, or leave it out for your default (Settings → Agents turns Auto on). Nothing was launched".to_owned()
        });
    }
    let name = agent.map(str::trim).filter(|a| !a.is_empty() && *a != "auto")?;
    let ids = |v: Value, k: &str| -> Vec<String> {
        v.as_array().into_iter().flatten().filter_map(|x| x.get(k).and_then(Value::as_str).map(str::to_owned)).collect()
    };
    let harnesses = ids(c.cmd("runtime.agents", json!({})).await.ok()?, "id");
    let profiles = ids(c.cmd("runtime.profiles", json!({})).await.unwrap_or(Value::Null), "name");
    if harnesses.iter().chain(&profiles).any(|h| h == name) {
        return None;
    }
    let also = if profiles.is_empty() { String::new() } else { format!(", or an agent: {}", profiles.join(", ")) };
    Some(format!("  ✗ no harness {name}: --agent takes {}{also}", harnesses.join(", ")))
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
    // a goal declared by name (wisp.goal refuses one no goal has, naming them), or what done means in words
    if let Some(g) = f.goal.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        a["goal"] = json!({ "name": g });
    } else if let Some(g) = f.goal_text.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        a["goal"] = json!(g);
    }
    // through a software factory (ADR-0114): wisp.factory refuses a name no factory has, naming them
    if let Some(name) = &f.factory {
        a["factory"] = json!(name.trim());
    }
    // --yes: go past Ask near the limit's line (the user agreed)
    if f.yes {
        a["confirmLimit"] = json!(true);
    }
    a
}

/// Setup audit F4: a task started from a project's folder is that project's, as New task's is. Its cwd is spelled as
/// the project's path (the shell's `/private/var/…` is the project's `/var/…`, so the app files it under the project),
/// and it gets its own worktree ([`own_worktree`]) unless `--worktree` or `--no-worktree` said. Outside every project the
/// arguments stay as they are, except in a git repo, which gets its own worktree all the same (audit E1).
pub async fn in_project(c: &Client, a: &mut Value) {
    if a.get("cwd").and_then(Value::as_str).is_none() {
        return;
    }
    let Ok(projects) = c.cmd("resources.export", json!({ "kinds": ["project"] })).await else { return };
    file_in(a, &projects);
}

/// [`in_project`] given the projects (`resources.export`'s list): the cwd spelled as its project's, and the worktree.
pub fn file_in(a: &mut Value, projects: &Value) {
    let Some(cwd) = a.get("cwd").and_then(Value::as_str).map(str::to_owned) else { return };
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
    match best {
        Some((_, spelled)) => a["cwd"] = json!(spelled),
        // audit E1: a git repo wisp doesn't know as a project still isn't edited in place (v1 launch.js:167 gave any git
        // repo a worktree); a plain folder stays as it is
        None if in_git_repo(std::path::Path::new(&cwd)) => {}
        None => return,
    }
    if a.get("worktree").is_none() {
        a["worktree"] = json!(own_worktree(a));
    }
}

/// Whether `dir` is inside a git checkout (a `.git` folder, or a worktree's `.git` file, here or above).
pub fn in_git_repo(dir: &std::path::Path) -> bool {
    dir.ancestors().any(|d| d.join(".git").exists())
}

/// The CLI's (and `wisp mcp`'s) own default (audit C4): a worktree of its own in a project, except in Plan mode (nothing
/// is written), whatever New task's Where last picked. The launcher's In place is sticky for New task, as v1's toggle
/// was (`defaults.worktree`), and five CLI launches after it edited the user's checkout; `--no-worktree` runs in place.
pub fn own_worktree(a: &Value) -> bool {
    a.get("mode").and_then(Value::as_str) != Some("plan")
}

/// Whether a task is where `--wait` and `wisp wait` stop: it asks you something, it's done, or its turn ended and, when
/// it has a goal, the goal is judged (met, paused, or out of rounds). Audit I11: `wisp run <task> --formula F --wait`
/// printed `review` and exited while its goal was still being judged; seconds later it asked "The goal can't be judged…".
pub fn settled(t: &Value) -> bool {
    match t.get("state").and_then(Value::as_str) {
        None | Some("running") => false,
        Some("waiting" | "done" | "failed" | "blocked") => true,
        Some(_) => {
            let goal = t.pointer("/ext/wisp.goal");
            let has_goal = goal.is_some() || t.pointer("/ext/wisp.runtime/spec/goal").is_some_and(|g| !g.is_null());
            !has_goal || matches!(goal.and_then(|g| g.get("state")).and_then(Value::as_str), Some("met" | "paused" | "stalled"))
        }
    }
}

/// Until the task needs you ([`settled`]), from its topic: no polling.
pub async fn wait(c: &Client, id: &str, timeout: Duration) -> Result<Value> {
    let done = crate::attach::until(c, id, settled);
    match tokio::time::timeout(timeout, done).await {
        Ok(v) => v,
        Err(_) => Ok(json!({ "id": id, "timedOut": true })),
    }
}

/// The task a command names: `last` is the newest one here; an id is checked, so a task that isn't there is said at
/// once instead of waited for (QA round 14 #7: `wisp attach last` hung).
pub async fn task_id(c: &Client, w: &str) -> Result<String> {
    if w != "last" {
        c.cmd("runtime.get", json!({ "id": w })).await.map_err(CliError::not_found)?;
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
    shown(c, f, id, summary).await.map(|_| ())
}

/// What `wisp wait` and `--wait` exit with, from where the task stopped ([`settled`]): 0 it finished (done, or its turn
/// ended for review), 1 it failed, was stopped or blocked (its usage limit too), [`crate::cli::WAIT_ASKS`] it waits on
/// you (a question, a permission, a turn held before it started), 124 the wait timed out (as `timeout(1)` says it).
/// Each exited 0: a script couldn't tell a failed task from a finished one; waiting was then 2, a usage error's code.
pub fn wait_exit(task: &Value, last: Option<&Value>, timed_out: bool) -> i32 {
    let turn = |k: &str| last.and_then(|t| t.get(k)).filter(|v| !v.is_null());
    let status = turn("status").and_then(Value::as_str).unwrap_or_default();
    match state_of(task).as_str() {
        _ if timed_out => 124,
        _ if status == "held" => crate::cli::WAIT_ASKS,
        "waiting" => crate::cli::WAIT_ASKS,
        "error" | "failed" | "blocked" | "stopped" => 1,
        _ if matches!(status, "error" | "stopped") || turn("limit").is_some() => 1,
        _ => 0,
    }
}

/// `--json` for `wisp show` and a wait: the task with its presentation, and `timedOut` when the wait ran out (the task
/// still running: its state alone read as one that had settled).
pub fn shown_json(task: &Value, presentation: Value, summary: Option<&Value>) -> Value {
    let mut all = task.clone();
    if let Some(o) = all.as_object_mut() {
        o.insert("presentation".into(), presentation);
        if timed_out(summary) {
            o.insert("timedOut".into(), Value::Bool(true));
        }
    }
    all
}

fn timed_out(summary: Option<&Value>) -> bool {
    summary.and_then(|x| x.get("timedOut")).and_then(Value::as_bool) == Some(true)
}

/// How long `wisp wait` and `--wait` wait: `--timeout` in seconds, or with a unit (`30s`, `10m`, `2h`); 30 minutes when
/// not given. Err: the words for a value that's none of those (a usage error).
pub fn wait_timeout(v: Option<&str>) -> std::result::Result<Duration, String> {
    let Some(v) = v.map(str::trim) else { return Ok(Duration::from_secs(30 * 60)) };
    let (n, unit) = match v.find(|c: char| !c.is_ascii_digit()) {
        Some(i) => v.split_at(i),
        None => (v, "s"),
    };
    let per = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => 0,
    };
    match n.parse::<u64>() {
        Ok(n) if n > 0 && per > 0 => Ok(Duration::from_secs(n.saturating_mul(per))),
        _ => Err(format!("--timeout {v}: seconds, or 30s, 10m, 2h — see wisp --help")),
    }
}

/// [`show`], and the code a wait exits with ([`wait_exit`]).
pub async fn shown(c: &Client, f: &Flags, id: &str, summary: Option<Value>) -> Result<i32> {
    let task = c.cmd("runtime.get", json!({ "id": id })).await.map_err(CliError::not_found)?;
    let p = c.cmd("runtime.presentation", json!({ "id": id })).await.unwrap_or(Value::Null);
    let late = timed_out(summary.as_ref());
    let still = if late { ", still running" } else { "" };
    let last = c.cmd("runtime.turns", json!({ "id": id, "limit": 1 })).await.ok();
    let code = wait_exit(&task, last.as_ref().and_then(Value::as_array).and_then(|t| t.last()), late);
    if f.json {
        say(&serde_json::to_string_pretty(&shown_json(&task, p, summary.as_ref())).unwrap_or_default());
        return Ok(code);
    }
    let st = crate::cli::style();
    let family = crate::history_cli::lineage_line(c, id).await.map_or_else(String::new, |l| format!("\n{l}"));
    let mut blocks = p.get("blocks").and_then(Value::as_array).cloned().unwrap_or_default();
    // a run at its usage limit: the hero says so in words; the CLI's raw stderr under it (`…limit reached|1767225600`,
    // an epoch) is for the page's card, never the terminal (audit C #13)
    if last.as_ref().and_then(Value::as_array).and_then(|t| t.last()).is_some_and(|t| t.get("limit").is_some_and(|l| !l.is_null())) {
        blocks.retain(|b| b.get("type").and_then(Value::as_str) != Some("terminal"));
    }
    let body = if blocks.is_empty() { String::new() } else { wisp_cli_core::Renderers::builtin().answer(&blocks, &st).join("\n") };
    let asks = asking(&task);
    let held =
        last.as_ref().and_then(Value::as_array).and_then(|t| t.last()).map_or_else(String::new, |t| crate::show_words::holding(id, t));
    let goal = crate::show_words::goal_line(&task);
    let factory = crate::show_words::factory_line(&task);
    let head = format!("◌ {}  [{}{still}]  {id}{family}{goal}{factory}", s(&task, "title"), s(&task, "state"));
    say(&show_text(&head, &[&body, &asks, &held]));
    Ok(code)
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
    if ask.get("setup").is_some_and(Value::is_object) {
        // a setup card (ADR-0113): Allow runs what it shows; anything you type is the Deny, with your words
        let _ = write!(out, "\n  Answer: wisp reply {id} Allow, or wisp reply {id} Deny (or say why not: wisp reply {id} \"<why not>\")");
    } else {
        let _ = write!(out, "\n  Answer: wisp reply {id} \"<your answer>\"");
    }
    out
}

/// `wisp ls` shows this many of the newest tasks unless `--limit` or `--all` says otherwise (v1's default; perf audit
/// 2026-11-19 #5: every one of 5,000 was read and printed).
pub const LS_DEFAULT: u32 = 50;

/// How many tasks `wisp ls` shows: `--limit n`, everything with `--all`, else the newest [`LS_DEFAULT`].
pub fn ls_limit(f: &Flags) -> Option<u32> {
    if f.all { f.limit } else { Some(f.limit.unwrap_or(LS_DEFAULT)) }
}

/// `--state a,b` as the states it names (`needs` is review + waiting), each once. Err: one no task is in.
pub fn ls_states(state: Option<&str>) -> Result<Vec<String>> {
    let mut states: Vec<String> = Vec::new();
    for x in state.iter().flat_map(|st| st.split(',')).map(str::trim).filter(|x| !x.is_empty()) {
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
        return Err(CliError::usage(format!("unknown state {}: use {} or needs (review + waiting)", bad.join(", "), LS_STATES.join(", "))));
    }
    Ok(states)
}

/// What `wisp ls` keeps of the task rows, and `wisp ls --watch` at each redraw (audit cli-r6 M2: the watch took only
/// `--all` and showed every task whatever `--state`, `--agent` or `--limit` said). A row is a `runtime.list` task or a
/// list topic's row (its columns, then its `body`); `--agent`, `--crew`, `--project` and `--since` as `runtime.history`
/// reads them for plain `wisp ls`.
#[derive(Debug, Default, Clone)]
pub struct RowFilter {
    pub states: Vec<String>,
    pub cwd: Option<String>,
    /// archived ones too
    pub all: bool,
    pub agent: Option<String>,
    /// `--crew`: the words a row's member id or name may match, any case; [`crate::history_cli::crew_words`] turns
    /// what was asked into them (id, name or handle through `crew.list`), as plain `wisp ls --crew` asks for them
    pub crew: Vec<String>,
    pub project: Option<String>,
    /// made at or after (ms)
    pub since: Option<i64>,
    pub limit: Option<usize>,
}

impl RowFilter {
    pub fn keep(&self, x: &Value) -> bool {
        let get = |k: &str| x.get(k).filter(|v| !v.is_null()).or_else(|| x.pointer(&format!("/body/{k}")).filter(|v| !v.is_null()));
        let text = |k: &str| get(k).and_then(Value::as_str).unwrap_or("");
        let ext = |p: &str| x.pointer(&format!("/ext/{p}")).or_else(|| x.pointer(&format!("/body/ext/{p}"))).and_then(Value::as_str);
        let made = get("createdAt").or_else(|| get("created_at")).and_then(Value::as_i64).unwrap_or(0);
        let cwd = text("cwd");
        (self.states.is_empty() || self.states.iter().any(|st| st == text("state")))
            && self.cwd.as_ref().is_none_or(|c| cwd == c.as_str())
            && (self.all || get("archived") != Some(&json!(true)))
            && self.agent.as_ref().is_none_or(|a| text("agent") == a.as_str())
            && (self.crew.is_empty()
                || self.crew.iter().any(|c| {
                    ext("wisp.crew/id").is_some_and(|i| i.eq_ignore_ascii_case(c))
                        || ext("wisp.crew/name").is_some_and(|n| n.eq_ignore_ascii_case(c))
                }))
            && self.project.as_ref().is_none_or(|p| cwd == p.as_str() || cwd.ends_with(&format!("/{p}")) || project_of(x) == *p)
            && self.since.is_none_or(|t| made >= t)
    }

    /// `wisp ls --watch`'s: the flags plain `wisp ls` takes (`o`: the ones only its history list reads), and as many
    /// rows as `room` (the terminal's lines under its header) unless `--limit` says. `members` is `crew.list`'s answer,
    /// read only when `--crew` was given, so `--crew <handle>` narrows to the member plain `wisp ls --crew` finds.
    pub fn watching(f: &Flags, o: &crate::code::Opts, members: &Value, room: usize, now: i64) -> Result<Self> {
        let since = o.since.as_deref().map(|t| crate::history_cli::since_ms(t, now)).transpose()?;
        Ok(Self {
            states: ls_states(f.state.as_deref())?,
            cwd: None,
            all: f.all,
            agent: f.agent.clone().or_else(|| o.over.get("agent").and_then(Value::as_str).map(str::to_owned)),
            crew: o.crew.as_deref().map(|c| crate::history_cli::crew_words(members, c)).unwrap_or_default(),
            project: o.project.clone(),
            since,
            limit: Some(f.limit.map_or(room, |l| l as usize)),
        })
    }

    /// The rows it keeps, in their order, at most `limit` of them.
    pub fn rows<'a>(&self, rows: impl IntoIterator<Item = &'a Value>) -> Vec<&'a Value> {
        rows.into_iter().filter(|x| self.keep(x)).take(self.limit.unwrap_or(usize::MAX)).collect()
    }
}

pub async fn ls(c: &Client, f: &Flags) -> Result<()> {
    let states = ls_states(f.state.as_deref())?;
    // every task, archived ones too (`--all` keeps them below); `{archived: true}` is the archived ones only, which made
    // `ls --all` say "No sessions." with tasks there (QA round 13 #2)
    // what the server can narrow it reads there (perf audit 2026-11-18 #10, as v1 asked): live tasks unless `--all`, and
    // only `--limit` of them when no state or folder filter comes after
    let mut ask = serde_json::Map::new();
    if !f.all {
        ask.insert("archived".into(), json!(false));
    }
    let limit = ls_limit(f);
    if let Some(l) = limit.filter(|_| states.is_empty() && f.cwd.is_none()) {
        ask.insert("limit".into(), json!(l));
    }
    let list = c.cmd("runtime.list", Value::Object(ask)).await?;
    let cwd = f.cwd.as_ref().map(|p| p.display().to_string());
    let keep = RowFilter { states, cwd, all: f.all, limit: limit.map(|l| l as usize), ..RowFilter::default() };
    let rows = keep.rows(list.as_array().into_iter().flatten());
    // the default cut said so, with how to see the rest
    let more = if f.limit.is_none() && !f.all && limit.is_some_and(|l| rows.len() == l as usize) {
        format!("\n… the newest {} shown: --limit N or --all for more", rows.len())
    } else {
        String::new()
    };
    let lines = ls_lines(&rows);
    // the same words as `wisp ls --crew` (audit D11: "No sessions." here, "No tasks." there)
    out(f, &json!(rows), &if lines.is_empty() { "No tasks.".to_owned() } else { format!("{}{more}", lines.join("\n")) });
    Ok(())
}

/// Who does a row's work: its harness, or `Factory · <name>` for a factory run, which runs no agent of its own (ADR-0114
/// rule 14; audit G15: `wisp ls` said "claude").
pub fn agent_of(x: &Value) -> String {
    let factory = ["/factory/name", "/summary/factory/name", "/ext/wisp.factory/factory"]
        .iter()
        .find_map(|p| x.pointer(p).and_then(Value::as_str).filter(|n| !n.is_empty()));
    match factory {
        Some(name) => format!("Factory · {name}"),
        None if ["/factory", "/summary/factory"].iter().any(|p| x.pointer(p).is_some_and(Value::is_object)) => {
            "Factory · default".to_owned()
        }
        None => s(x, "agent").to_owned(),
    }
}

/// A row's state as `wisp ls` prints it: a factory run's own (audit G15: a run you stopped printed `review`, the state
/// its task is left in, while the app and `wisp show` said Stopped), else the task's.
pub fn state_of(x: &Value) -> String {
    ["/factory/state", "/summary/factory/state"]
        .iter()
        .find_map(|p| x.pointer(p).and_then(Value::as_str).filter(|v| !v.is_empty()))
        .map_or_else(|| s(x, "state").to_owned(), str::to_owned)
}

/// `wisp ls`'s rows, each column as wide as its longest value (audit E15: "antigravity" pushed its row's project two
/// characters right of every other row's, past the agent column's fixed 8).
fn ls_lines(rows: &[&Value]) -> Vec<String> {
    let aw = rows.iter().map(|x| agent_of(x).chars().count()).max().unwrap_or(0).max(8);
    let sw = rows.iter().map(|x| state_of(x).chars().count()).max().unwrap_or(0).max(7);
    rows.iter()
        .map(|x| {
            // one line per task: a question's own lines go on one (audit I, smaller notes: a guard task took three)
            let ask = x
                .get("question")
                .and_then(Value::as_str)
                .map(|q| q.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|q| !q.is_empty())
                .map_or(String::new(), |q| format!("  — asks: {q}"));
            let proj = project_of(x);
            format!(
                "{}  {:<sw$}  {:<aw$}  {:<16}  {}{ask}",
                s(x, "id"),
                state_of(x),
                agent_of(x),
                proj.chars().take(16).collect::<String>(),
                s(x, "title").split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

/// `wisp show`'s text: its head, then its answer, question and hold, one blank line between those there are (audit
/// E15: three empty lines came before a hold, a usage limit or a question when the answer was empty).
fn show_text(head: &str, parts: &[&str]) -> String {
    let mut out = head.to_owned();
    for p in parts.iter().map(|p| p.trim_matches('\n')).filter(|p| !p.trim().is_empty()) {
        out.push_str("\n\n");
        out.push_str(p);
    }
    out
}

/// The project a list row is in: its worktree's project (`summary.project`, the repo it was made from; the worktree's
/// own folder is `proj-01a1f…`, audit C #15), else its folder's name.
fn project_of(row: &Value) -> String {
    let named = |v: Option<&Value>| v.and_then(Value::as_str).filter(|p| !p.is_empty()).map(str::to_owned);
    named(row.pointer("/summary/project")).or_else(|| named(row.get("project"))).unwrap_or_else(|| {
        // a task made in wisp's scratch folder (`<home>/scratch/<id>`, Cast a plugin) is in no project (audit D7)
        let at = std::path::Path::new(s(row, "cwd"));
        let scratch = at.parent().and_then(std::path::Path::file_name).is_some_and(|n| n == "scratch");
        let name = at.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty() && !scratch);
        name.unwrap_or_else(|| "no project".to_owned())
    })
}

/// `wisp reply <id> <text>`; with `--wait`, until the task settles or `limit` runs out, exiting as `wisp wait` does
/// ([`wait_exit`]). Without it, 0 once it's sent.
#[allow(clippy::many_single_char_names)]
pub async fn reply(c: &Client, f: &Flags, args: &[String], limit: Duration) -> Result<i32> {
    let id = need(args.first(), "session id")?;
    let text = args[1..].join(" ");
    let text = need(Some(&text), "text")?;
    // a setup card open on the task (ADR-0113), or a question about a safety guard your harness asked for (ADR-0115):
    // your reply answers it, naming it (audit G4: it was queued behind the card as a follow-up, and "Allow" later ran as
    // the agent's prompt; a guard's question takes no follow-up at all)
    if let Ok(task) = c.cmd("runtime.get", json!({ "id": id })).await
        && let Some(ask) = task
            .pointer("/ext/wisp.runtime/ask")
            .filter(|a| a.get("setup").is_some_and(Value::is_object) || a.get("guard") == Some(&json!(true)))
    {
        let r = c.cmd("runtime.answer", json!({ "id": id, "text": text, "askId": s(ask, "id") })).await?;
        out(f, &r, &format!("  ✓ {}", Some(s(&r, "said")).filter(|t| !t.is_empty()).unwrap_or("answered")));
        return Ok(0);
    }
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
    let r = c.cmd("runtime.followUp", a).await.map_err(CliError::not_found)?;
    if !f.wait {
        // an interrupting follow-up says so (audit I3: "✓ queued" was printed for one that stopped the running turn)
        let said = if r.get("interrupts") == Some(&json!(true)) {
            "  ✓ sent: it interrupts the running turn and goes next (--queue waits behind it instead)"
        } else if r.get("queued").is_some_and(|v| !v.is_null() && *v != json!(false)) {
            "  ✓ queued"
        } else {
            "  ✓ sent"
        };
        out(f, &r, said);
        return Ok(0);
    }
    let w = wait(c, id, limit).await?;
    shown(c, f, id, Some(w)).await
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

    /// Audit C4: New task's In place (`worktrees.worktree = off`) is sticky for New task, as in v1; a CLI or `wisp mcp`
    /// launch in a project keeps its own default, a worktree, except in Plan mode.
    #[test]
    fn a_cli_launch_in_a_project_gets_its_own_worktree_whatever_new_task_picked_audit_c4() {
        assert!(own_worktree(&json!({ "prompt": "x", "cwd": "/p" })));
        assert!(own_worktree(&json!({ "prompt": "x", "mode": "edits" })));
        assert!(!own_worktree(&json!({ "prompt": "x", "mode": "plan" })));
    }

    /// Audit E1: a git repo that isn't a project is still a git repo (a worktree's `.git` is a file); a plain folder isn't.
    #[test]
    fn a_git_checkout_is_found_from_any_folder_inside_it_audit_e1() {
        let root = std::env::temp_dir().join(format!("wisp-in-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("repo/.git")).unwrap_or_default();
        std::fs::create_dir_all(root.join("repo/src/deep")).unwrap_or_default();
        std::fs::create_dir_all(root.join("tree/src")).unwrap_or_default();
        std::fs::write(root.join("tree/.git"), "gitdir: /x/.git/worktrees/tree\n").unwrap_or_default();
        std::fs::create_dir_all(root.join("plain")).unwrap_or_default();
        assert!(in_git_repo(&root.join("repo/src/deep")));
        assert!(in_git_repo(&root.join("tree/src")));
        assert!(!in_git_repo(&root.join("plain")));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Audit E15: an 11-character agent kept every row's columns lined up; `wisp show` had one blank line before a hold.
    /// Audit I11: `--wait` stopped at `review` while the task's goal was still being judged.
    #[test]
    fn wait_goes_through_a_goals_judging_audit_i11() {
        use serde_json::json;
        let goal = |state: &str, g: &str| json!({ "state": state, "ext": { "wisp.goal": { "state": g }, "wisp.runtime": { "spec": { "goal": { "text": "green" } } } } });
        assert!(!super::settled(&json!({ "state": "running" })));
        assert!(super::settled(&json!({ "state": "review" })), "no goal: its turn's end");
        assert!(!super::settled(&goal("review", "")), "the turn ended, judging not begun");
        assert!(!super::settled(&goal("review", "judging")));
        assert!(
            !super::settled(&json!({ "state": "review", "ext": { "wisp.runtime": { "spec": { "goal": "green" } } } })),
            "a goal not yet written"
        );
        for g in ["met", "paused", "stalled"] {
            assert!(super::settled(&goal("review", g)), "{g}");
        }
        assert!(super::settled(&goal("waiting", "judging")), "it asks you: stop");
        assert!(super::settled(&goal("done", "")));
    }

    /// `wisp wait` and `--wait` exited 0 whatever became of the task (failed, stopped, timed out): a script couldn't
    /// tell. Each end has its own code, and `--json` says a wait timed out.
    #[test]
    fn wait_exits_with_how_the_task_ended() {
        let t = |state: &str| json!({ "id": "t", "state": state });
        let turn = |status: &str| json!({ "status": status });
        assert_eq!(wait_exit(&t("done"), Some(&turn("done")), false), 0);
        assert_eq!(wait_exit(&t("review"), Some(&turn("done")), false), 0);
        assert_eq!(wait_exit(&t("review"), None, false), 0);
        assert_eq!(wait_exit(&t("error"), Some(&turn("error")), false), 1, "failed");
        assert_eq!(wait_exit(&t("review"), Some(&turn("error")), false), 1, "its turn failed");
        assert_eq!(wait_exit(&t("review"), Some(&turn("stopped")), false), 1, "stopped");
        assert_eq!(wait_exit(&t("review"), Some(&json!({ "status": "done", "limit": { "at": 1 } })), false), 1, "at its usage limit");
        let factory = json!({ "id": "t", "state": "review", "factory": { "name": "gated", "state": "stopped" } });
        assert_eq!(wait_exit(&factory, None, false), 1, "a stopped factory run");
        assert_eq!(wait_exit(&t("waiting"), Some(&turn("running")), false), 3, "a question or a permission");
        assert_eq!(wait_exit(&t("review"), Some(&turn("held")), false), 3, "held before it started");
        assert_ne!(crate::cli::WAIT_ASKS, crate::cli::USAGE_EXIT, "waits on you is never a usage error's code");
        assert_eq!(wait_exit(&t("running"), Some(&turn("running")), true), 124, "timed out");
        let timed = shown_json(&t("running"), json!(null), Some(&json!({ "id": "t", "timedOut": true })));
        assert_eq!((timed["timedOut"].as_bool(), timed["state"].as_str()), (Some(true), Some("running")));
        assert!(shown_json(&t("done"), json!(null), Some(&t("done"))).get("timedOut").is_none());
        assert!(shown_json(&t("done"), json!(null), None).get("presentation").is_some());
    }

    /// cli-r6 M2: `wisp ls` and `wisp ls --watch` keep rows through one filter: state, agent, crew, project, since,
    /// archived and limit, on `runtime.list` tasks and on the list topic's rows (columns, then `body`) alike.
    #[test]
    fn one_row_filter_narrows_ls_and_ls_watch() {
        let rows = [
            json!({ "id": "a", "state": "running", "agent": "claude", "cwd": "/w/api", "createdAt": 30 }),
            json!({ "id": "b", "state": "waiting", "agent": "codex", "created_at": 20, "body": { "cwd": "/w/web", "ext": { "wisp.crew": { "id": "m1", "name": "Ada" } } } }),
            json!({ "id": "c", "state": "review", "agent": "codex", "cwd": "/w/api", "createdAt": 10, "archived": true }),
            json!({ "id": "d", "state": "done", "agent": "claude", "cwd": "/w/web", "createdAt": 5 }),
        ];
        let ids = |f: &RowFilter| f.rows(rows.iter()).iter().map(|r| r["id"].as_str().unwrap_or_default()).collect::<Vec<_>>();
        assert_eq!(ids(&RowFilter::default()), ["a", "b", "d"], "archived ones only with --all");
        assert_eq!(ids(&RowFilter { all: true, ..RowFilter::default() }), ["a", "b", "c", "d"]);
        let needs = ls_states(Some("needs")).unwrap();
        assert_eq!(ids(&RowFilter { states: needs, all: true, ..RowFilter::default() }), ["b", "c"]);
        assert_eq!(ids(&RowFilter { states: vec!["running".into()], ..RowFilter::default() }), ["a"]);
        assert_eq!(ids(&RowFilter { agent: Some("codex".into()), ..RowFilter::default() }), ["b"]);
        assert_eq!(ids(&RowFilter { crew: vec!["ada".into()], ..RowFilter::default() }), ["b"], "a member by name, any case");
        // `--crew ADA`, `m1` (a name in any case, an id): resolved through crew.list to the member's id and name, as plain `wisp ls --crew`
        let members = json!([{ "id": "m1", "name": "Ada" }, { "id": "m2", "name": "Bob" }]);
        let crew = crate::history_cli::crew_words(&members, "ADA");
        assert_eq!(ids(&RowFilter { crew, ..RowFilter::default() }), ["b"], "by the member crew.list names");
        let crew = crate::history_cli::crew_words(&members, "m1");
        assert_eq!(ids(&RowFilter { crew, ..RowFilter::default() }), ["b"], "by its id");
        let crew = crate::history_cli::crew_words(&members, "bob");
        assert!(ids(&RowFilter { crew, ..RowFilter::default() }).is_empty(), "another member's tasks only");
        assert_eq!(ids(&RowFilter { project: Some("web".into()), ..RowFilter::default() }), ["b", "d"]);
        assert_eq!(ids(&RowFilter { since: Some(20), ..RowFilter::default() }), ["a", "b"]);
        assert_eq!(ids(&RowFilter { limit: Some(2), ..RowFilter::default() }), ["a", "b"]);
        assert_eq!(ids(&RowFilter { limit: Some(1), agent: Some("claude".into()), ..RowFilter::default() }), ["a"]);
        assert_eq!(ls_states(Some("bogus")).unwrap_err().exit_code(), crate::cli::USAGE_EXIT);
    }

    /// `wisp show` with no id is a usage error (2), and a task that isn't there its own code (4), never a refusal's 1.
    #[test]
    fn a_missing_id_is_a_usage_error_and_an_unknown_task_is_not_found() {
        let e = need(None, "session id").unwrap_err();
        assert!(matches!(e, CliError::Usage(_)), "{e:?}");
        assert_eq!(e.exit_code(), crate::cli::USAGE_EXIT);
        assert_eq!(need(Some(&"  ".to_owned()), "text").unwrap_err().exit_code(), 2);
        assert_eq!(need(Some(&"t1".to_owned()), "session id").unwrap(), "t1");
        let gone = CliError::Http { status: 404, message: "no task t9".into(), body: json!({}) }.not_found();
        assert!(matches!(&gone, CliError::NotFound(m) if m == "no task t9"), "{gone:?}");
        assert_eq!(gone.exit_code(), crate::cli::NOT_FOUND);
        assert_eq!(crate::cli::NOT_FOUND, 4);
        let refused = CliError::Http { status: 409, message: "busy".into(), body: json!({}) }.not_found();
        assert_eq!(refused.exit_code(), 1, "any other refusal stays 1");
        let codes = [0, 1, crate::cli::USAGE_EXIT, crate::cli::WAIT_ASKS, crate::cli::NOT_FOUND, 124];
        assert_eq!(codes.iter().collect::<std::collections::BTreeSet<_>>().len(), codes.len(), "one meaning each");
    }

    #[test]
    fn wait_timeout_takes_seconds_or_a_unit_and_defaults_to_thirty_minutes() {
        assert_eq!(wait_timeout(None), Ok(Duration::from_secs(1800)));
        assert_eq!(wait_timeout(Some("45")), Ok(Duration::from_secs(45)));
        assert_eq!(wait_timeout(Some("30s")), Ok(Duration::from_secs(30)));
        assert_eq!(wait_timeout(Some("10m")), Ok(Duration::from_secs(600)));
        assert_eq!(wait_timeout(Some("2h")), Ok(Duration::from_secs(7200)));
        for bad in ["", "0", "-5", "10x", "m", "1.5h", "ten"] {
            assert!(wait_timeout(Some(bad)).is_err(), "{bad}");
        }
        let v: Vec<String> = ["wait", "abc123abc123", "--timeout", "10m"].iter().map(|s| (*s).to_owned()).collect();
        let p = crate::args::parse(&v, None, std::path::Path::new("/h"), std::path::Path::new("/w")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((p.command, p.flags.timeout.as_deref()), (Some("wait"), Some("10m")));
    }

    /// Audit G15: `wisp ls` printed `review` for a factory run you stopped (`01a1faa27cca…  review  Factory · gated`),
    /// the state its task is left in, while the app and `wisp show` said Stopped. It prints the run's own state.
    #[test]
    fn ls_says_a_stopped_factory_run_is_stopped_audit_g15() {
        let run = serde_json::json!({ "id": "01a1faa27cca", "state": "review", "title": "gated", "factory": { "name": "gated", "state": "stopped" } });
        let task = serde_json::json!({ "id": "01a1faa27ccb", "state": "review", "agent": "claude", "title": "fix" });
        assert_eq!(super::state_of(&run), "stopped");
        assert_eq!(super::state_of(&task), "review");
        let lines = super::ls_lines(&[&run, &task]);
        assert!(lines[0].contains("stopped") && lines[0].contains("Factory · gated") && !lines[0].contains("review"), "{lines:?}");
        assert!(lines[1].contains("review"), "{lines:?}");
    }

    #[test]
    fn ls_columns_line_up_and_show_has_one_blank_line_between_parts_audit_e15() {
        let a = json!({ "id": "01a", "state": "review", "agent": "antigravity", "cwd": "/w/proj", "title": "A" });
        let b = json!({ "id": "01b", "state": "running", "agent": "codex", "cwd": "/w/proj", "title": "B" });
        let lines = ls_lines(&[&a, &b]);
        let col = |l: &str| l.find("proj").unwrap();
        assert_eq!(col(&lines[0]), col(&lines[1]), "{lines:?}");
        let text = show_text("◌ T  [review]  01a", &["", "", "\n\n◷ Held before it started"]);
        assert_eq!(text, "◌ T  [review]  01a\n\n◷ Held before it started");
        assert_eq!(show_text("◌ T", &["body", "\n\n? Asks: x", ""]), "◌ T\n\nbody\n\n? Asks: x");
    }

    #[test]
    fn launch_args_leave_the_rest_to_the_default_profile() {
        let mut f = Flags::defaults(None);
        assert_eq!(launch_args(&f, "fix it", "/w"), json!({ "prompt": "fix it", "cwd": "/w" }));
        f.agent = Some("reviewer".into());
        f.worktree = Some(false);
        assert_eq!(launch_args(&f, "x", "/w"), json!({ "prompt": "x", "cwd": "/w", "agent": "reviewer", "worktree": false }));
    }

    /// ADR-0110: `--goal NAME` names a declared goal (wisp.goal checks it), `--goal-text` says it in words.
    #[test]
    fn a_launch_takes_a_declared_goal_by_name_or_a_goal_in_words() {
        let mut f = Flags::defaults(None);
        f.goal = Some("green-build".into());
        assert_eq!(launch_args(&f, "x", "/w")["goal"], json!({ "name": "green-build" }));
        f.goal = None;
        f.goal_text = Some("tests pass".into());
        assert_eq!(launch_args(&f, "x", "/w")["goal"], json!("tests pass"));
        let p = crate::args::parse(
            &["fix it".into(), "--goal".into(), "green-build".into()],
            None,
            std::path::Path::new("/h"),
            std::path::Path::new("/w"),
        )
        .unwrap();
        assert_eq!(p.flags.goal.as_deref(), Some("green-build"));
    }

    /// ADR-0114: `--factory NAME` runs it through that factory, `--factory` alone through `default`; a word that can't be
    /// a name (it has spaces) stays the prompt, and `wisp run <task> --factory` lays it over the declared task.
    #[test]
    fn factory_flag_names_a_factory_and_alone_is_default() {
        let parse = |a: &[&str]| {
            crate::args::parse(
                &a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
                None,
                std::path::Path::new("/h"),
                std::path::Path::new("/w"),
            )
            .unwrap()
        };
        let p = parse(&["fix it", "--factory", "careful"]);
        assert_eq!((p.flags.factory.as_deref(), p.words.as_slice()), (Some("careful"), ["fix it".to_owned()].as_slice()));
        assert_eq!(launch_args(&p.flags, "fix it", "/w")["factory"], json!("careful"));
        let p = parse(&["fix it", "--factory"]);
        assert_eq!(p.flags.factory.as_deref(), Some(""), "alone: default");
        let p = parse(&["--factory", "fix the add bug", "--wait"]);
        assert_eq!(
            (p.flags.factory.as_deref(), p.words.as_slice(), p.flags.wait),
            (Some(""), ["fix the add bug".to_owned()].as_slice(), true)
        );
        let p = parse(&["--factory=careful", "x"]);
        assert_eq!(p.flags.factory.as_deref(), Some("careful"));
        assert!(launch_args(&Flags::defaults(None), "x", "/w").get("factory").is_none());
        let o = crate::code::opts(&["release".into(), "--factory".into(), "careful".into()]);
        assert_eq!(o.over.get("factory"), Some(&json!("careful")));
        let o = crate::code::opts(&["release".into(), "--factory".into()]);
        assert_eq!(o.over.get("factory"), Some(&json!("")));
    }

    /// perf audit 2026-11-19 #5: `wisp ls` shows the newest 50 (v1's default), `--limit n` that many, `--all` all.
    #[test]
    fn ls_shows_the_newest_fifty_unless_told() {
        let mut f = Flags::defaults(None);
        assert_eq!(ls_limit(&f), Some(LS_DEFAULT));
        f.limit = Some(7);
        assert_eq!(ls_limit(&f), Some(7));
        f.all = true;
        assert_eq!(ls_limit(&f), Some(7), "--all with --limit: that many, archived too");
        f.limit = None;
        assert_eq!(ls_limit(&f), None, "--all: every task");
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

    /// Audit D7: a task with no project (Cast a plugin, run in `<home>/scratch/<id>`) read `01a1f42ce94d72ba` in `wisp
    /// ls`'s project column. A scratch folder is no project; a plain folder still shows its name.
    #[test]
    fn a_scratch_task_shows_no_project_and_a_plain_folder_its_name_audit_d7() {
        let row = |cwd: &str| serde_json::json!({ "id": "a", "cwd": cwd, "project": null });
        assert_eq!(super::project_of(&row("/h/.wisp/scratch/01a1f42ce94d72ba")), "no project");
        assert_eq!(super::project_of(&row("")), "no project");
        assert_eq!(super::project_of(&row("/w/app")), "app");
        assert_eq!(super::project_of(&serde_json::json!({ "cwd": "/w/app-01a1", "summary": { "project": "app" } })), "app");
    }

    /// Audit I (smaller notes): `wisp ls` printed a guard task over three lines (its question's own lines). Each task is
    /// one line.
    #[test]
    fn a_task_asking_a_question_of_several_lines_is_one_line_audit_i() {
        let g = serde_json::json!({ "id": "g1", "state": "waiting", "agent": "claude", "cwd": "/w/app", "title": "Your harness wants to x",
            "question": "Your harness wants to x.\nworktrees.projectAsk: on → off\nAllow it only if you asked for this." });
        let lines = ls_lines(&[&g]);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains('\n'), "{}", lines[0]);
        assert!(
            lines[0].ends_with("asks: Your harness wants to x. worktrees.projectAsk: on → off Allow it only if you asked for this."),
            "{}",
            lines[0]
        );
    }
}
