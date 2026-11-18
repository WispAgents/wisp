//! Audit A (2026-11-18), end to end on a real `wisp serve` with the fake agent: what automations, recipes and goals got
//! wrong, each checked where it reached the agent (its recorded argv and folder), the task record and the CLI.
//!   - orch #14: a trigger names its project by name (`project: proj`, as a formula does): it runs in that project's
//!     folder; a name no project has is refused at apply (it used to save, then start nothing, saying nothing).
//!   - orch #15: `trigger test` on a schedule tries its next run (it tried nothing: "(none)"); `apply --prune` with
//!     `triggers: []` removes the last trigger (it pruned nothing), and an emptied formula says how to remove one.
//!   - orch #16: `wisp run <name>` for no recipe says so (it launched an agent on "nosuchrecipe"); `--wait` waits
//!     for the recipe's task; the recipe runs in its own project, not where the CLI is.
//!   - tasks #15: Drop goal on a goal that stalled takes its "Keep going?" back: the task stops Asking.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, cwd: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The folders the fake agent ran in for a prompt containing `prompt`.
fn run_dirs(rec: &Path, prompt: &str) -> Vec<String> {
    let Ok(files) = std::fs::read_dir(rec) else { return Vec::new() };
    files
        .filter_map(|f| f.ok().map(|f| f.path()))
        .flat_map(|p| common::records(&p))
        .filter(|v| {
            v["stdin"].as_str().is_some_and(|s| s.contains(prompt))
                || v["argv"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str().is_some_and(|x| x.contains(prompt))))
        })
        .filter_map(|v| v["cwd"].as_str().map(str::to_owned))
        .collect()
}

fn canonical(p: &Path) -> std::path::PathBuf {
    wisp_proc::canonical_blocking(p).unwrap()
}

fn setup(name: &str) -> (std::path::PathBuf, common::Serve, Api, u16) {
    let dir = temp(name);
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    std::fs::write(dir.join("proj/math.js"), "export const add = (a, b) => a + b;\n").unwrap();
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    (dir, serve, api, port)
}

async fn finished(api: &Api, id: &str) {
    api.until(id, "the turn ends", |t| t.last().is_some_and(ended)).await;
}

fn triggers(api_out: &Value) -> Vec<String> {
    api_out.as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_trigger_named_by_its_project_runs_in_that_folder_and_an_unknown_name_is_refused_audit_a_14() {
    let (dir, _serve, api, _port) = setup("serve-auditA-14");
    let proj = dir.join("proj");
    let r = api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [
                { "kind": "project", "name": "proj", "spec": { "path": proj } },
                { "kind": "trigger", "name": "named", "spec": { "on": "schedule", "cron": "0 3 * * *", "prompt": "Tidy NAMED14", "project": "proj" } }
            ] }),
        )
        .await;
    assert_eq!(r["ok"], true, "{r}");
    // kept as the project's folder: the engine compares folders and starts its task in one
    let ls = api.cmd("automations.trigger.ls", json!({})).await;
    let spec = ls.as_array().unwrap().iter().find(|t| t["name"] == "named").map(|t| t["spec"].clone()).unwrap();
    assert_eq!(canonical(Path::new(spec["project"].as_str().unwrap())), canonical(&proj), "{spec}");
    let out = api.cmd("automations.trigger.fire", json!({ "name": "named" })).await;
    let id = out["session"].as_str().unwrap_or_else(|| panic!("fired nothing: {out}")).to_owned();
    finished(&api, &id).await;
    let ran = run_dirs(&dir.join("rec"), "Tidy NAMED14");
    assert_eq!(ran.len(), 1, "{ran:?}");
    assert_eq!(Path::new(&ran[0]), canonical(&proj), "ran in the named project's folder");
    // a name no project has: refused, and nothing is left half made
    let r = api
        .try_cmd("resources.apply", json!({ "resources": [{ "kind": "trigger", "name": "lost", "spec": { "on": "schedule", "cron": "0 3 * * *", "prompt": "x", "project": "nope" } }] }))
        .await;
    assert_eq!(r["ok"], false, "{r}");
    assert!(r.to_string().contains("no project is named \\\"nope\\\""), "{r}");
    assert!(r.to_string().contains("proj"), "it says which there are: {r}");
    assert!(!triggers(&api.cmd("automations.trigger.ls", json!({})).await).contains(&"lost".to_owned()));
}

#[tokio::test(flavor = "multi_thread")]
async fn trigger_test_tries_a_schedules_next_run_and_prune_empties_a_section_left_empty_audit_a_15() {
    let (dir, _serve, api, port) = setup("serve-auditA-15");
    let proj = dir.join("proj");
    let f = dir.join("wisp.yml");
    let formula = |body: &str| std::fs::write(&f, format!("wisp: 1\n{body}")).unwrap();
    formula(&format!(
        "triggers:\n  - name: minutely\n    on: schedule\n    cron: \"* * * * *\"\n    prompt: Tidy MINUTELY15\n    project: {}\n    enabled: false\n",
        proj.display()
    ));
    let (code, out, err) = wisp(&dir, &dir, port, &["apply", "-f", f.to_str().unwrap(), "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    // the dry run: its next minute, the prompt it would send, nothing started
    let tried = api.cmd("automations.trigger.test", json!({ "name": "minutely" })).await;
    let list = tried.as_array().unwrap();
    assert_eq!(list.len(), 1, "its next run: {tried}");
    assert!(list[0]["summary"].as_str().unwrap().starts_with("its next run"), "{tried}");
    assert!(list[0]["prompt"].as_str().unwrap().contains("Tidy MINUTELY15"), "{tried}");
    let (code, out, err) = wisp(&dir, &dir, port, &["trigger", "test", "minutely"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("its next run") && !out.contains("(none)"), "{out}");
    assert!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().is_empty(), "a dry run starts nothing");
    // an emptied file names no section: nothing to prune, and it says how to remove one
    formula("");
    let (code, out, err) = wisp(&dir, &dir, port, &["apply", "-f", f.to_str().unwrap(), "--yes", "--prune"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("triggers: []"), "says how: {out}");
    assert!(triggers(&api.cmd("automations.trigger.ls", json!({})).await).contains(&"minutely".to_owned()));
    // `triggers: []` names it with nothing left in it: --prune removes the trigger
    formula("triggers: []\n");
    let (code, out, err) = wisp(&dir, &dir, port, &["apply", "-f", f.to_str().unwrap(), "--yes", "--prune"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("1 to remove"), "{out}");
    assert!(!triggers(&api.cmd("automations.trigger.ls", json!({})).await).contains(&"minutely".to_owned()), "pruned");
    // a formula with a problem never says "1 to add" above it
    formula("triggers:\n  - name: broken\n    prompt: no source\n");
    let (code, out, _) = wisp(&dir, &dir, port, &["apply", "-f", f.to_str().unwrap(), "--dry-run"]);
    assert_ne!(code, 0, "{out}");
    assert!(!out.contains("1 to add"), "{out}");
    assert!(out.contains("nothing can be applied"), "{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recipe_run_from_the_cli_refuses_an_unknown_name_waits_and_runs_in_its_project_audit_a_16() {
    let (dir, _serve, api, port) = setup("serve-auditA-16");
    let proj = dir.join("proj");
    let elsewhere = dir.join("elsewhere");
    api.cmd(
        "automations.task.set",
        json!({ "name": "fixmath", "prompt": "fix the {thing} in math.js RECIPE16", "project": proj, "agent": "claude" }),
    )
    .await;
    // no recipe of that name: said, and no agent is sent "nosuchrecipe"
    for args in [&["run", "nosuchrecipe"][..], &["run", "nosuchrecipe", "thing=x"], &["run", "nosuchrecipe", "--wait"]] {
        let (code, out, err) = wisp(&dir, &elsewhere, port, args);
        assert_ne!(code, 0, "{args:?}: {out}{err}");
        assert!(err.contains("no recipe or workflow named nosuchrecipe"), "{args:?}: {out}{err}");
    }
    assert!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().is_empty(), "nothing was launched");
    // --wait: back once the task needs you, with what it did
    let at = Instant::now();
    let (code, out, err) = wisp(&dir, &elsewhere, port, &["run", "fixmath", "thing=sign", "--wait"]);
    assert_eq!(code, 0, "{out}{err}");
    let list = api.cmd("runtime.list", json!({})).await;
    let task = &list.as_array().unwrap()[0];
    assert_ne!(task["state"], "running", "--wait returned before the task ended ({:?}): {list}", at.elapsed());
    let ran = run_dirs(&dir.join("rec"), "fix the sign in math.js RECIPE16");
    assert_eq!(ran.len(), 1, "{ran:?}");
    assert_eq!(Path::new(&ran[0]), canonical(&proj), "the recipe's project, not where the CLI ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn drop_goal_takes_its_keep_going_question_back_audit_a_tasks_15() {
    let (dir, _serve, api, _port) = setup("serve-auditA-t15");
    // the fake never says MET: one round and it stalls, asking whether to keep going
    let made = api
        .cmd(
            "runtime.create",
            json!({ "prompt": "fix the add bug", "cwd": dir.join("proj"), "agent": "claude", "worktree": false, "goal": { "text": "add adds", "maxRounds": 1 } }),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let at = Instant::now();
    let asked = loop {
        let t = api.cmd("runtime.get", json!({ "id": id })).await;
        if t["state"] == "waiting" {
            break t;
        }
        assert!(at.elapsed() < Duration::from_secs(60), "the goal never stalled: {t}");
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    let q = asked.pointer("/ext/wisp.runtime/ask/question").and_then(Value::as_str).unwrap_or_default().to_owned();
    assert!(q.contains("Keep going?"), "{asked}");
    api.cmd("goal.drop", json!({ "id": id })).await;
    let t = api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!(t["state"], "review", "the task stops Asking: {t}");
    assert!(t.pointer("/ext/wisp.runtime/ask").is_none(), "the question is gone: {t}");
    // another question put since stays (`unask` takes back only its own)
    api.cmd("runtime.ask", json!({ "id": id, "question": "Which database?" })).await;
    let r = api.cmd("runtime.unask", json!({ "id": id, "question": q })).await;
    assert_eq!(r["withdrawn"], false, "{r}");
    assert_eq!(api.cmd("runtime.get", json!({ "id": id })).await["state"], "waiting");
}
