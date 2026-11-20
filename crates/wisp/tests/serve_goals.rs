//! Goals declared by name and `tasks.default` (ADR-0110, owner 2026-11-19, owner-list row 60), end to end on `wisp
//! serve` with the fake harness: a formula declares `goals:` (its `default` every goal's judges, vote and limit) and
//! `tasks:` (its `default` every task's fields); a task names a goal; `wisp validate` names a goal no one declared; a
//! run of the task takes the defaults and the declared goal; `default` is never a task to run; and a goal is attached
//! by name when running (`wisp "…" --goal`, `wisp goal set`, `wisp mcp`'s `wisp_launch`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env("COLUMNS", "100")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const FORMULA: &str = "wisp: 1
goals:
  default: { judges: [claude], vote: all, maxRounds: 1 }
  green-build: { text: npm test passes and the build is green, maxRounds: 2 }
tasks:
  default: { agent: claude, mode: auto, worktree: false }
  fix-build: { prompt: Make the build pass, goal: green-build }
  inline: { prompt: Tidy up, mode: plan, goal: { text: the linter is quiet } }
";

/// The goal a task was made with (its spec's), and its list row's goal view.
async fn goal_of(api: &Api, id: &str) -> (Value, Value) {
    let t = api.cmd("runtime.get", json!({ "id": id })).await;
    let row = api.cmd("runtime.list", json!({})).await;
    let view = row
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["id"] == id)
        .and_then(|r| r.get("summary").and_then(|s| s.get("goal")).or_else(|| r.get("goal")).cloned())
        .unwrap_or_default();
    (t["ext"]["wisp.runtime"]["spec"]["goal"].clone(), view)
}

#[tokio::test(flavor = "multi_thread")]
async fn goals_declared_by_name_and_tasks_default_from_formula_to_run() {
    let dir = temp("serve-goals");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let proj_s = proj.display().to_string();

    // a task naming a goal no one declared: validate says which, and what there is
    std::fs::write(dir.join("bad.yml"), format!("{FORMULA}  typo: {{ prompt: x, goal: green-buld }}\n")).unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "bad.yml"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        format!("{out}{err}").contains("tasks.typo: no goal named green-buld: did you mean green-build? goals has green-build"),
        "{out}{err}"
    );
    // the defaults hold no goal text, and no task's own fields
    std::fs::write(
        dir.join("worse.yml"),
        "wisp: 1\ngoals:\n  default: { text: nope }\ntasks:\n  default: { prompt: x, schedule: \"0 2 * * *\" }\n",
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "worse.yml"]);
    let said = format!("{out}{err}");
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("goals.default") && said.contains("has no text"), "{said}");
    assert!(said.contains("prompt is each task's own") && said.contains("schedule is each task's own"), "{said}");

    std::fs::write(dir.join("wisp.yml"), FORMULA).unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "wisp.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    for k in ["goal/default", "goal/green-build", "task/default", "task/fix-build", "task/inline"] {
        assert!(out.contains(&format!("+ {k}")), "{k}: {out}");
    }
    // `default` is a resource, never a recipe to list or run
    let tasks = api.cmd("automations.tasks", json!({})).await;
    let names: Vec<&str> = tasks.as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, ["fix-build", "inline"]);
    let refused = api.try_cmd("automations.task.run", json!({ "name": "default" })).await;
    assert!(refused.to_string().contains("isn't a task to run"), "{refused}");
    let declared = api.cmd("goal.declared", json!({})).await;
    assert_eq!(declared, json!([{ "name": "green-build", "text": "npm test passes and the build is green", "maxRounds": 2 }]));
    // a goal taken away from under a task that names it (destroy, --prune) is refused too, naming the task
    let plan = api.cmd("resources.plan", json!({ "resources": [], "destroy": ["goal.green-build"] })).await;
    assert!(plan["problems"].to_string().contains("tasks.fix-build: no goal named green-build"), "{plan}");
    // applied again: nothing changes (the defaults read back as written)
    let (_, out, _) = wisp(&dir, port, &["diff", "-f", "wisp.yml"]);
    assert!(out.contains("0 to add, 0 to change, 0 to remove"), "{out}");

    // a run takes tasks.default under its own fields, and the declared goal it names
    let made = api.cmd("automations.task.run", json!({ "name": "fix-build", "over": { "project": proj_s } })).await;
    let id = made["id"].as_str().unwrap_or_else(|| panic!("{made}")).to_owned();
    let t = api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!((t["agent"].as_str(), t["mode"].as_str()), (Some("claude"), Some("auto")), "tasks.default's agent and mode: {t}");
    let (spec, view) = goal_of(&api, &id).await;
    assert_eq!(spec["name"], "green-build", "{spec}");
    assert_eq!(spec["text"], "npm test passes and the build is green", "{spec}");
    assert_eq!(view["vote"], "all", "goals.default's vote fills what the goal doesn't say: {view}");
    let inline = api.cmd("automations.task.run", json!({ "name": "inline", "over": { "project": proj_s } })).await;
    let iid = inline["id"].as_str().unwrap().to_owned();
    let t = api.cmd("runtime.get", json!({ "id": iid })).await;
    assert_eq!(t["mode"], "plan", "the task's own mode wins over tasks.default's: {t}");
    assert_eq!(goal_of(&api, &iid).await.0["text"], "the linter is quiet");

    // wisp "…" --goal NAME, --goal-text, and a name no goal has
    let (code, out, err) = wisp(&dir, port, &["fix the add bug", "--goal", "green-build", "--cwd", &proj_s, "--no-worktree", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let cid =
        serde_json::from_str::<Value>(&out).ok().and_then(|v| v[0]["id"].as_str().map(str::to_owned)).unwrap_or_else(|| panic!("{out}"));
    assert_eq!(goal_of(&api, &cid).await.0["text"], "npm test passes and the build is green");
    let (code, out, err) = wisp(&dir, port, &["fix it", "--goal", "nope", "--cwd", &proj_s, "--no-worktree"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("no goal named nope: goals has green-build"), "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["fix it", "--goal-text", "the add test passes", "--cwd", &proj_s, "--no-worktree", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let wid = serde_json::from_str::<Value>(&out).unwrap()[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(goal_of(&api, &wid).await.0, json!("the add test passes"));

    // wisp goal set <task> <name>: the declared goal on a task already made
    let (code, out, err) = wisp(&dir, port, &["goal", "set", &wid, "green-build"]);
    assert_eq!(code, 0, "{out}{err}");
    let st = api.cmd("runtime.get", json!({ "id": wid })).await["ext"]["wisp.goal"].clone();
    assert_eq!((st["text"].as_str(), st["maxRounds"].as_u64()), (Some("npm test passes and the build is green"), Some(2)), "{st}");

    // wisp mcp's wisp_launch: a declared goal's name, or words
    let mut child = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .arg("mcp")
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .current_dir(&proj)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }))
            .unwrap();
        let launch = json!({ "prompt": "fix it", "agent": "claude", "worktree": false, "goal": "green-build" });
        writeln!(
            input,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "wisp_launch", "arguments": launch } })
        )
        .unwrap();
    }
    let answers: Vec<Value> =
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let said = answers.iter().find(|a| a["id"] == 2).unwrap_or_else(|| panic!("{answers:?}"))["result"]["content"][0]["text"].clone();
    let mid =
        said.as_str().unwrap().strip_prefix("launched ").and_then(|r| r.split(':').next()).unwrap_or_else(|| panic!("{said}")).to_owned();
    assert_eq!(goal_of(&api, &mid).await.0["text"], "npm test passes and the build is green");
    drop(serve);
}
