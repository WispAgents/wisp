//! History as code (ADR-0029) and `wisp import` (ADR-0026):
//!
//! - `wisp ls --formula f.yml | --agent reviewer | --crew Ada | --workflow nightly | --since 7d
//!   [--project api]`: tasks by what made them (`runtime.history`).
//! - `wisp task history <id>`: its spec, its turns, its family (reruns, forks, a workflow's steps, what
//!   it was shared from).
//! - `wisp history export --since 30d > history.jsonl`: one line per task, scrubbed (secrets, secret env
//!   values, home paths) as a share is.
//! - `wisp import [v1 home]`: v1's config as a formula, its plan shown first, applied on yes; then the
//!   tasks' history (`runtime.import`).
use std::io::{IsTerminal, Write};
use std::path::Path;

use serde_json::{Value, json};
use wisp_cli_core::term::{self, BOLD, DIM, GREEN, paint};
use wisp_resources_api::{Plan, Resource};

use crate::client::{CliError, Client, Result};
use crate::code::Opts;
use crate::scripted::say;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// `7d`, `12h`, `30m`, `2w` → ms before `now`; a number alone is days.
pub fn since_ms(text: &str, now: i64) -> Result<i64> {
    let t = text.trim();
    let (n, unit) = t.split_at(t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len()));
    let n: i64 =
        n.parse().map_err(|_| CliError::usage(format!("--since {t}: a number of days (7d), hours (12h), minutes (30m) or weeks (2w)")))?;
    let ms = match unit {
        "" | "d" => 86_400_000,
        "h" => 3_600_000,
        "m" => 60_000,
        "w" => 7 * 86_400_000,
        _ => return Err(CliError::usage(format!("--since {t}: d, h, m or w"))),
    };
    Ok(now - n * ms)
}

fn now_ms() -> i64 {
    i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())).unwrap_or(0)
}

/// Whether `wisp ls` was asked a history question.
pub fn wants_history(o: &Opts) -> bool {
    !o.formulas.is_empty()
        || o.crew.is_some()
        || o.workflow.is_some()
        || o.since.is_some()
        || o.over.contains_key("agent")
        || o.project.is_some()
}

async fn query(c: &Client, o: &Opts, cwd: &Path) -> Result<Vec<Value>> {
    let mut profiles = Vec::new();
    for f in &o.formulas {
        let (text, name) =
            wisp_resources_core::formula::Source::load(&mut crate::code::Files { cwd: cwd.to_owned() }, f, None).map_err(CliError::said)?;
        for doc in wisp_resources_core::formula::documents(&text, wisp_resources_core::formula::format_of(&name)).map_err(CliError::said)? {
            profiles.extend(doc.get("agents").and_then(Value::as_array).cloned().unwrap_or_default());
        }
        if profiles.is_empty() {
            return Err(CliError::said(format!("{f} declares no agents to look for")));
        }
    }
    let since = o.since.as_deref().map(|t| since_ms(t, now_ms())).transpose()?;
    // `--crew ada`: the member's jobs carry its id, a formula's tasks its name; both are asked (audit B #10: "No tasks.")
    let crews = match &o.crew {
        Some(asked) => crew_words(&c.cmd("crew.list", json!({})).await.unwrap_or(Value::Null), asked),
        None => vec![String::new()],
    };
    let mut rows: Vec<Value> = Vec::new();
    for crew in crews {
        let crew = (!crew.is_empty()).then_some(crew);
        let args = json!({ "agent": o.over.get("agent"), "crew": crew, "workflow": o.workflow, "project": o.project, "since": since, "profiles": profiles });
        let v = c.cmd("runtime.history", args).await?;
        rows.extend(v.as_array().cloned().unwrap_or_default());
    }
    Ok(merged(rows))
}

/// The words `runtime.history` is asked for `--crew <asked>`: the member it names (by id, name or handle, any case)
/// as its id and its name; the word itself when no member has it.
pub fn crew_words(list: &Value, asked: &str) -> Vec<String> {
    let handle = |s: &str| s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    let found = list.as_array().into_iter().flatten().find(|m| {
        s(m, "id") == asked
            || s(m, "name").eq_ignore_ascii_case(asked)
            || (!s(m, "name").is_empty() && handle(s(m, "name")) == handle(asked))
    });
    match found {
        Some(m) => [s(m, "id"), s(m, "name")].into_iter().filter(|w| !w.is_empty()).map(str::to_owned).collect(),
        None => vec![asked.to_owned()],
    }
}

/// Rows of several asks as one list: each task once, newest first.
fn merged(rows: Vec<Value>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Value> = rows.into_iter().filter(|r| seen.insert(s(r, "id").to_owned())).collect();
    out.sort_by_key(|r| std::cmp::Reverse(r.get("createdAt").and_then(Value::as_i64).unwrap_or(0)));
    out
}

fn lineage_words(l: &Value) -> String {
    let mut w = Vec::new();
    for (k, what) in [
        ("rerunOf", "rerun of"),
        ("forkOf", "fork of"),
        ("workflowRun", "step of"),
        ("triggeredBy", "started by"),
        ("crewJob", "crew job"),
        ("sharedFrom", "shared from"),
    ] {
        if let Some(v) = l.get(k).and_then(Value::as_str) {
            w.push(format!("{what} {v}"));
        }
    }
    w.join(" · ")
}

/// `wisp ls` with history filters.
pub async fn ls(c: &Client, o: &Opts, cwd: &Path) -> Result<i32> {
    let rows = query(c, o, cwd).await?;
    if o.json {
        say(&serde_json::to_string_pretty(&rows).unwrap_or_default());
        return Ok(0);
    }
    let st = crate::cli::style();
    let fit = std::io::IsTerminal::is_terminal(&std::io::stdout());
    if rows.is_empty() {
        say("No tasks.");
    }
    for r in &rows {
        let agent =
            if s(r, "agent") == s(r, "harness") { s(r, "agent").to_owned() } else { format!("{} ({})", s(r, "agent"), s(r, "harness")) };
        let fam = lineage_words(&r["lineage"]);
        // its project, as plain `wisp ls` shows it (audit D7: the column was empty)
        let project = Some(s(r, "project")).filter(|p| !p.is_empty()).unwrap_or("no project");
        let head = format!(
            "{}  {:<7}  {:<18}  {:<16}  ",
            s(r, "id"),
            s(r, "state"),
            term::truncate(&agent, 18),
            project.chars().take(16).collect::<String>()
        );
        let fam = if fam.is_empty() { String::new() } else { format!("  {fam}") };
        if !fit {
            // piped (a script, `grep`): the whole row, as plain `wisp ls` prints it
            say(&format!("{head}{}{}", s(r, "title"), paint(&st, DIM, &fam)));
            continue;
        }
        // on a terminal the title gives way, not what made the task (`step of …`, `rerun of …`: what this listing is for)
        let room = st.width.max(60).saturating_sub(term::width(&head) + term::width(&fam)).max(12);
        let line = format!("{head}{}{}", term::truncate(s(r, "title"), room), paint(&st, DIM, &fam));
        say(&term::truncate(&line, st.width.max(60)));
    }
    Ok(0)
}

/// `wisp task history <id>`.
pub async fn task_history(c: &Client, id: &str, json_out: bool) -> Result<i32> {
    let spec = c.cmd("runtime.spec", json!({ "id": id })).await?;
    let turns = c.cmd("runtime.turns", json!({ "id": id })).await?;
    let all = c.cmd("runtime.history", json!({})).await.unwrap_or(json!([]));
    let family: Vec<&Value> = all
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| {
            ["rerunOf", "forkOf", "workflowRun", "sharedFrom"].iter().any(|k| r["lineage"].get(*k).and_then(Value::as_str) == Some(id))
        })
        .collect();
    if json_out {
        say(&serde_json::to_string_pretty(&json!({ "spec": spec, "turns": turns, "family": family })).unwrap_or_default());
        return Ok(0);
    }
    let st = crate::cli::style();
    say(&format!(
        "{} {}",
        paint(&st, BOLD, id),
        paint(
            &st,
            DIM,
            &format!(
                "{}{}",
                s(&spec, "agent"),
                spec.get("formula").and_then(Value::as_str).map(|h| format!(" · ran with @{h}")).unwrap_or_default()
            )
        )
    ));
    let fam = lineage_words(&spec["lineage"]);
    if !fam.is_empty() {
        say(&format!("  {fam}"));
    }
    if let Some(b) = spec.get("base").filter(|b| b.is_object()) {
        say(&format!("  from {} {}", s(b, "branch"), s(b, "sha").get(..10).unwrap_or("")));
    }
    for t in turns.as_array().into_iter().flatten() {
        let mark = if s(t, "status") == "done" { paint(&st, GREEN, "✓") } else { s(t, "status").to_owned() };
        say(&term::truncate(&format!("  {mark} {}  {}", t["idx"], s(t, "prompt").lines().next().unwrap_or("")), st.width.max(60)));
    }
    for f in family {
        say(&format!("  ↳ {}  {}  {}", s(f, "id"), s(f, "state"), lineage_words(&f["lineage"])));
    }
    Ok(0)
}

/// `wisp history export [--since 30d]`: JSONL, scrubbed.
pub async fn export(c: &Client, o: &Opts, cwd: &Path) -> Result<i32> {
    let rows = query(c, o, cwd).await?;
    let scrub = wisp_share_core::Scrub {
        env: wisp_share_core::secret_env(std::env::vars()),
        project: String::new(),
        home: std::env::var("HOME").unwrap_or_default(),
        ..wisp_share_core::Scrub::default()
    };
    let mut out = std::io::stdout().lock();
    for r in rows {
        let id = s(&r, "id").to_owned();
        let spec = c.cmd("runtime.spec", json!({ "id": id })).await.unwrap_or_default();
        let turns = c.cmd("runtime.turns", json!({ "id": id })).await.unwrap_or_default();
        let brief: Vec<Value> = turns.as_array().into_iter().flatten().map(|t| json!({ "idx": t["idx"], "prompt": t["prompt"], "status": t["status"], "text": t["text"], "stats": t["stats"], "startedAt": t["startedAt"], "endedAt": t["endedAt"] })).collect();
        let line = json!({ "task": r, "spec": spec, "turns": brief });
        let clean = wisp_share_core::scrub(&line, &scrub, "", &mut Vec::new());
        let _ = writeln!(out, "{clean}");
    }
    Ok(0)
}

/// The lineage line `wisp show` adds.
pub async fn lineage_line(c: &Client, id: &str) -> Option<String> {
    let spec = c.cmd("runtime.spec", json!({ "id": id })).await.ok()?;
    let mut parts = vec![lineage_words(&spec["lineage"])];
    if let Some(h) = spec.get("formula").and_then(Value::as_str) {
        parts.push(format!("ran with @{h}"));
    }
    if let Some(sha) = spec.pointer("/base/sha").and_then(Value::as_str) {
        parts.push(format!("from {}", sha.get(..10).unwrap_or(sha)));
    }
    let l: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    (!l.is_empty()).then(|| format!("  {}", l.join(" · ")))
}

/// Whether two project paths name one folder (stored canonical; macOS's `/tmp` is `/private/tmp`).
fn same_folder(a: &str, b: &str) -> bool {
    let canon = |p: &str| wisp_runtime_api::paths::canonical_project_path(p, None);
    let (a, b) = (canon(a), canon(b));
    a == b || a.strip_prefix("/private") == Some(b.as_str()) || b.strip_prefix("/private") == Some(a.as_str())
}

/// Setup audit F23: a v1 project whose folder is already a project here is that project: left out of the import (it
/// would be a second project for one folder, which the plan refuses), and the import's `project:` fields that named
/// it name the one here. The notes say which.
fn same_folder_projects(resources: &mut Vec<wisp_resources_api::Resource>, existing: &Value) -> Vec<String> {
    fn repoint(v: &mut Value, from: &str, to: &str) {
        match v {
            Value::Object(o) => {
                for (k, x) in o.iter_mut() {
                    if k == "project" && x.as_str() == Some(from) {
                        *x = json!(to);
                    } else {
                        repoint(x, from, to);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| repoint(x, from, to)),
            _ => {}
        }
    }
    let here: Vec<(String, String)> = existing
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| Some((r.get("name")?.as_str()?.to_owned(), r.pointer("/spec/path")?.as_str()?.to_owned())))
        .collect();
    let mut renamed: Vec<(String, String)> = Vec::new();
    resources.retain(|r| {
        if r.kind != "project" {
            return true;
        }
        let path = r.spec.get("path").and_then(Value::as_str).unwrap_or_default();
        match here.iter().find(|(_, p)| same_folder(path, p)) {
            Some((name, _)) => {
                renamed.push((r.name.clone(), name.clone()));
                false
            }
            None => true,
        }
    });
    for r in resources.iter_mut() {
        for (from, to) in &renamed {
            repoint(&mut r.spec, from, to);
        }
    }
    renamed
        .into_iter()
        .map(|(from, to)| {
            if from == to {
                format!("project {to} is here already")
            } else {
                format!("project {from}: its folder is {to} here, kept as {to}")
            }
        })
        .collect()
}

/// `runtime.launch` as it is with v1's pick over it: its `agent`, and its harness's entry in `models` and `efforts`
/// (each map kept, never replaced).
fn merged_pick(mut now: serde_json::Map<String, Value>, v1: &Value) -> Value {
    for (k, v) in v1.as_object().into_iter().flatten() {
        match (now.get_mut(k), v) {
            (Some(Value::Object(have)), Value::Object(add)) => have.extend(add.clone()),
            _ => drop(now.insert(k.clone(), v.clone())),
        }
    }
    Value::Object(now)
}

/// The plan of applying `resources` (nothing pruned).
async fn plan_of(c: &Client, resources: &[Resource]) -> Result<Plan> {
    serde_json::from_value(c.cmd("resources.plan", json!({ "resources": resources, "prune": false })).await?)
        .map_err(|e| CliError::said(e.to_string()))
}

/// The imported projects the plan refuses for their folder (`projects.<name>: No folder at …`, "… is a file"), each
/// with what it said.
fn missing_folder_projects(plan: &Plan, resources: &[Resource]) -> Vec<(String, String)> {
    resources
        .iter()
        .filter(|r| r.kind == "project")
        .filter_map(|r| {
            let at = format!("projects.{}: ", r.name);
            plan.problems
                .iter()
                .find_map(|p| p.strip_prefix(&at))
                .filter(|why| why.starts_with("No folder at") || why.contains("is a file, not a folder"))
                .map(|why| (r.name.clone(), why.to_owned()))
        })
        .collect()
}

/// `wisp import [v1 home] [--dry-run] [--yes]`.
pub async fn import(c: &Client, o: &Opts, home: &Path, cwd: &Path) -> Result<i32> {
    // the folder as this shell names it: the server reads paths from its own folder (audit H HS1)
    let v1 = o.words.first().map_or_else(|| home.to_owned(), |p| cwd.join(p));
    let file = v1.join("config.json");
    let text = std::fs::read_to_string(&file).map_err(|e| CliError::said(crate::words::no_v1_home(&v1, &file, &e)))?;
    let cfg: Value = serde_json::from_str(&text).map_err(|e| CliError::said(format!("{}: {e}", file.display())))?;
    let (mut resources, mut notes) = crate::v1_formula::formula_of(&cfg);
    // audit F setup F4: a connector's env vars and headers come with their values from v1's secrets.json
    let v1_secrets: Value =
        std::fs::read(v1.join("secrets.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    let (bring, said) = crate::v1_formula::bring_connector_secrets(&mut resources, &cfg, &v1_secrets);
    notes.extend(said);
    let existing = c.cmd("resources.export", json!({ "kinds": ["project"] })).await.unwrap_or(Value::Null);
    notes.extend(same_folder_projects(&mut resources, &existing));
    // New task's pick keeps what it has: v1's harness, and that harness's model and level, go in (setup audit B11)
    if let Some(r) = resources.iter_mut().find(|r| r.kind == "setting" && r.name == "runtime.launch")
        && let Ok(Value::Object(now)) = c.cmd("settings.get", json!({ "key": "runtime.launch" })).await
    {
        r.spec = merged_pick(now, &r.spec);
    }
    let st = crate::cli::style();
    let mut plan = plan_of(c, &resources).await?;
    // setup audit B2: a v1 project whose folder isn't here is left out, said, never imported (the plan refuses it)
    let gone = missing_folder_projects(&plan, &resources);
    if !gone.is_empty() {
        resources.retain(|r| !(r.kind == "project" && gone.iter().any(|(n, _)| *n == r.name)));
        notes.extend(gone.iter().map(|(n, why)| format!("project {n} not imported: {why}")));
        plan = plan_of(c, &resources).await?;
    }
    // the secrets the import brings aren't missing: they're set just before the apply
    plan.missing_secrets.retain(|m| !bring.iter().any(|(n, _)| n == m));
    let args = json!({ "resources": resources, "prune": false });
    say(&format!("{} {}", paint(&st, BOLD, "Import plan"), paint(&st, DIM, &format!("(from {})", v1.display()))));
    crate::code::show_plan(&plan, &st);
    for n in &notes {
        say(&format!("  {} {n}", paint(&st, DIM, "·")));
    }
    let sessions = v1.join("sessions");
    let n = std::fs::read_dir(&sessions)
        .map_or(0, |d| d.filter_map(std::result::Result::ok).filter(|e| e.path().extension().is_some_and(|x| x == "json")).count());
    say(&format!("  and the history of {n} task{} (tasks are history: they're imported, never planned)", if n == 1 { "" } else { "s" }));
    if o.dry_run {
        return Ok(i32::from(!plan.problems.is_empty()));
    }
    if !plan.problems.is_empty() {
        return Ok(1);
    }
    if !o.yes {
        if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
            eprintln!("  ✗ not a terminal: import with --yes");
            return Ok(1);
        }
        let mut line = String::new();
        let mut so = std::io::stdout().lock();
        let _ = write!(so, "\n  Import it? [y/N] ");
        let _ = so.flush();
        let _ = std::io::stdin().read_line(&mut line);
        if !matches!(line.trim(), "y" | "Y" | "yes") {
            say("  nothing imported");
            return Ok(1);
        }
    }
    for (name, value) in &bring {
        c.cmd("resources.secretSet", json!({ "name": name, "value": value })).await?;
    }
    let done = c.cmd("resources.apply", args).await?;
    say(&format!("  {} applied {} change{}", paint(&st, GREEN, "✓"), done["applied"], if done["applied"] == 1 { "" } else { "s" }));
    if n > 0 {
        let r = c.cmd("runtime.import", json!({ "dir": sessions.display().to_string() })).await?;
        let (said, failed) = import_words(&r, n);
        say(&said.iter().map(|l| l.replacen("✓", &paint(&st, GREEN, "✓"), 1)).collect::<Vec<_>>().join("\n"));
        if failed {
            return Ok(1);
        }
    }
    Ok(0)
}

/// What `runtime.import`'s report says, and whether the import failed: a file it couldn't read is printed, and a
/// plan that counted tasks the import didn't reach (none imported, none unchanged) is an error, never "0 imported"
/// (audit H HS1).
fn import_words(r: &Value, counted: usize) -> (Vec<String>, bool) {
    let n = |k: &str| r[k].as_u64().unwrap_or(0);
    let failed: Vec<&str> = r["failed"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut out = Vec::new();
    if n("imported") + n("skipped") > 0 {
        let s = if n("imported") == 1 { "" } else { "s" };
        out.push(format!("  ✓ {} task{s} imported, {} unchanged", n("imported"), n("skipped")));
    }
    out.extend(failed.iter().map(|f| format!("  ✗ not imported: {f}")));
    let none = counted > 0 && n("imported") + n("skipped") == 0;
    if none && failed.is_empty() {
        let s = if counted == 1 { "" } else { "s" };
        out.push(format!("  ✗ the plan counted {counted} task{s}, but none was imported"));
    }
    (out, none)
}

#[cfg(test)]
mod import_tests {
    use serde_json::json;

    use super::import_words;

    /// Audit H HS1: "✓ 0 tasks imported, 0 unchanged" after a plan that counted one, and `failed` never printed.
    #[test]
    fn an_import_that_reached_none_of_the_counted_tasks_fails_and_says_why_audit_h_hs1() {
        let (said, failed) = import_words(&json!({ "imported": 0, "skipped": 0, "failed": [] }), 1);
        assert_eq!((said, failed), (vec!["  ✗ the plan counted 1 task, but none was imported".to_owned()], true));
        let (said, failed) = import_words(&json!({ "imported": 0, "skipped": 0, "failed": ["a.json: not JSON"] }), 1);
        assert_eq!((said, failed), (vec!["  ✗ not imported: a.json: not JSON".to_owned()], true));
        let (said, failed) = import_words(&json!({ "imported": 1, "skipped": 0, "failed": ["b.json: not JSON"] }), 2);
        assert_eq!(said, ["  ✓ 1 task imported, 0 unchanged", "  ✗ not imported: b.json: not JSON"]);
        assert!(!failed, "some came in");
        assert_eq!(
            import_words(&json!({ "imported": 0, "skipped": 2, "failed": [] }), 2),
            (vec!["  ✓ 0 tasks imported, 2 unchanged".to_owned()], false)
        );
    }
}

#[cfg(test)]
mod crew_tests {
    use serde_json::json;

    use super::{crew_words, merged};

    /// Audit B #10: `wisp ls --crew ada` said "No tasks." with Ada's jobs there: they carry her id, not "ada".
    #[test]
    fn crew_asked_by_name_asks_for_the_members_id_and_name() {
        let list = json!([{ "id": "a1b2c3d4e5f6a7b8", "name": "Ada" }, { "id": "0f1e2d3c4b5a6978", "name": "Bob Lee" }]);
        assert_eq!(crew_words(&list, "ada"), ["a1b2c3d4e5f6a7b8", "Ada"]);
        assert_eq!(crew_words(&list, "bob-lee"), ["0f1e2d3c4b5a6978", "Bob Lee"]);
        assert_eq!(crew_words(&list, "0f1e2d3c4b5a6978"), ["0f1e2d3c4b5a6978", "Bob Lee"]);
        assert_eq!(crew_words(&list, "zed"), ["zed"], "no such member: the word as asked");
        let rows =
            merged(vec![json!({ "id": "a", "createdAt": 1 }), json!({ "id": "b", "createdAt": 3 }), json!({ "id": "a", "createdAt": 1 })]);
        assert_eq!(rows.iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), ["b", "a"], "each task once, newest first");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Setup audit B11: v1's pick goes over New task's own, keeping the other harnesses' models and levels.
    #[test]
    fn v1s_pick_merges_into_new_tasks_keeping_the_rest() {
        let now =
            json!({ "agent": "claude", "models": { "claude": "opus" }, "efforts": { "claude": "xhigh" }, "labels": { "claude": "Work" } });
        let v1 = json!({ "agent": "codex", "models": { "codex": "gpt-5-codex" } });
        assert_eq!(
            merged_pick(now.as_object().cloned().unwrap(), &v1),
            json!({ "agent": "codex", "models": { "claude": "opus", "codex": "gpt-5-codex" }, "efforts": { "claude": "xhigh" }, "labels": { "claude": "Work" } })
        );
    }

    /// Setup audit F23: importing a v1 project whose folder is a project here made a second project for it.
    #[test]
    fn a_v1_project_for_a_folder_that_is_a_project_here_is_that_one() {
        let mut rs = vec![
            wisp_resources_api::Resource::new("project", "repo", json!({ "path": "/tmp/qaB/repo" })),
            wisp_resources_api::Resource::new("project", "new-one", json!({ "path": "/tmp/qaB/other" })),
            wisp_resources_api::Resource::new("task", "nightly", json!({ "prompt": "x", "project": "repo" })),
        ];
        let existing = json!([{ "kind": "project", "name": "second-repo", "spec": { "path": "/private/tmp/qaB/repo" } }]);
        let notes = same_folder_projects(&mut rs, &existing);
        let names: Vec<&str> = rs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["new-one", "nightly"]);
        assert_eq!(rs[1].spec["project"], "second-repo");
        assert!(notes[0].contains("kept as second-repo"), "{notes:?}");
    }

    #[test]
    fn since_reads_days_hours_minutes_weeks() {
        assert_eq!(since_ms("7d", 1_000_000_000).unwrap(), 1_000_000_000 - 7 * 86_400_000);
        assert_eq!(since_ms("12h", 0).unwrap(), -12 * 3_600_000);
        assert_eq!(since_ms("3", 0).unwrap(), -3 * 86_400_000);
        assert!(since_ms("soon", 0).is_err());
    }
}
