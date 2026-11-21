//! Audit H (orchestration, 2026-11-20), end to end on a real `wisp serve` with the fake agent:
//!   - H2: a misspelled launch flag (`wisp "…" --mdoe plan`) launched in Full auto with the flag in its prompt.
//!   - H4, H13: the app's trigger switch wrote `project: proj` back as its folder; the CLI couldn't switch one off.
//!   - H6: `--goal nevr` listed all 32 declared goals; H7: `wisp later cancel <nothing>` said "ok true".
//!   - H8, H12: verbs printed field dumps; `crew ls` didn't mark a paused member. H9: help with no words.
//!   - H10: one rerun printed the several-runs hint; H11: `wisp run nosuch` named only recipes.
//!   - D6: `factory_set`, `resources_plan` and `resources_read` took only `{args, peer}` over MCP.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, ended, start, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env_remove("WISP_TASK")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

async fn tasks(api: &Api) -> usize {
    api.cmd("runtime.list", json!({})).await.as_array().map_or(0, Vec::len)
}

async fn mcp(http: &reqwest::Client, port: u16, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let r = http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    let text = r.text().await.unwrap();
    let json = text.lines().find_map(|l| l.strip_prefix("data:")).map_or(text.as_str(), str::trim).to_owned();
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_misspelled_launch_flag_launches_nothing_audit_h2() {
    let dir = temp("serve-auditH-flag");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let before = tasks(&api).await;
    for (args, close) in [
        (&["H typo2", "--mdoe", "plan"][..], Some("did you mean --mode?")),
        (&["H typo flag", "--agnet", "codex"], Some("did you mean --agent?")),
        (&["H flagtest", "--bogus", "1"], None),
        (&["H later", "--at", "23:55"], None),
    ] {
        let (code, out, err) = wisp(&dir, port, args);
        assert_eq!(code, 1, "{args:?}: {out}{err}");
        assert!(err.contains(&format!("wisp takes no {}", args[1])) && err.contains("Nothing was launched"), "{args:?}: {err}");
        if let Some(c) = close {
            assert!(err.contains(c), "{args:?}: {err}");
        }
    }
    assert_eq!(tasks(&api).await, before, "nothing launched");
    // after `--` it's the prompt's: it launches, the flag in its words
    let (code, out, err) = wisp(&dir, port, &["H typo2", "-d", "--", "--mdoe", "plan"]);
    assert_eq!(code, 0, "{out}{err}");
    let list = api.cmd("runtime.list", json!({})).await;
    assert!(list.to_string().contains("--mdoe"), "{list}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_switch_changes_enabled_alone_and_the_cli_switches_and_removes_audit_h4_h13() {
    let dir = temp("serve-auditH-trig");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let r = api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [
                { "kind": "project", "name": "proj", "spec": { "path": proj } },
                { "kind": "trigger", "name": "nightly", "spec": { "on": "schedule", "cron": "0 3 * * *", "prompt": "Tidy H4", "project": "proj" } }
            ] }),
        )
        .await;
    assert_eq!(r["ok"], true, "{r}");
    let stored = || async {
        let all = api.cmd("resources.export", json!({ "kinds": ["trigger"] })).await;
        all.as_array().unwrap().iter().find(|t| t["name"] == "nightly").map_or(Value::Null, |t| t["spec"].clone())
    };
    let written = stored().await;
    // what the app reads (trigger.ls) says the project as written, so its Edit and switch send it back so
    let ls = api.cmd("automations.trigger.ls", json!({})).await;
    let row = ls.as_array().unwrap().iter().find(|t| t["name"] == "nightly").cloned().unwrap();
    assert_eq!(row["spec"]["project"], "proj", "{row}");
    // the switch (the app's, and `wisp trigger off`): enabled alone changes
    api.cmd("automations.trigger.switch", json!({ "name": "nightly", "on": false })).await;
    let off = stored().await;
    assert_eq!(off["enabled"], false, "{off}");
    let mut want = written.clone();
    want["enabled"] = json!(false);
    assert_eq!(off, want, "only enabled changed");
    let (code, out, err) = wisp(&dir, port, &["trigger", "on", "nightly"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("trigger nightly is on"), "{out}");
    assert_eq!(stored().await["enabled"], true);
    assert_eq!(stored().await["project"], "proj");
    let (code, out, err) = wisp(&dir, port, &["trigger", "off", "nightly"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("trigger nightly is off"), "{out}");
    let state = api.cmd("automations.trigger.ls", json!({})).await;
    assert!(state.to_string().contains("\"state\":\"off\""), "{state}");
    let (code, _, err) = wisp(&dir, port, &["trigger", "off", "nosuch"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no trigger named nosuch"), "{err}");
    let (code, out, err) = wisp(&dir, port, &["trigger", "rm", "nightly"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(stored().await, Value::Null, "removed");
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_words_say_what_happened_audit_h6_h7_h8_h9_h10_h11_h12() {
    let dir = temp("serve-auditH-words");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // H6: a refusal names 8 goals, how many more, and the close one
    let mut goals = String::from("wisp: 1\ngoals:\n  never:\n    text: never met\n");
    for i in 1..=12 {
        std::fmt::Write::write_fmt(&mut goals, format_args!("  goal-{i:02}:\n    text: goal {i}\n")).unwrap();
    }
    std::fs::write(dir.join("goals.yml"), goals).unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", dir.join("goals.yml").to_str().unwrap(), "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["H goal", "--goal", "nevr", "-d"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("did you mean never?") && err.contains("and 5 more"), "{err}");

    // H7: nothing scheduled with that id says so
    let (code, out, err) = wisp(&dir, port, &["later", "cancel", "nosuchid123"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("Nothing is scheduled on nosuchid123"), "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["later", "cancel-plan", "nosuchid123"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("No plan has the id nosuchid123"), "{out}{err}");

    // H11: a run that names nothing names each kind, and what lists it
    let (code, _, err) = wisp(&dir, port, &["run", "nosuch"]);
    assert_ne!(code, 0);
    assert!(err.contains("no declared task, recipe or workflow named nosuch") && err.contains("wisp workflows list"), "{err}");

    // H8, H12: a crew member's job and check-in are sentences; a paused one is marked in the list
    let made = api.cmd("crew.create", json!({ "name": "Zed", "agent": "claude", "cwd": dir.join("proj") })).await;
    assert!(made.to_string().contains("Zed"), "{made}");
    let (code, out, err) = wisp(&dir, port, &["crew", "checkin", "Zed"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("✓ Zed · Check-in") && out.contains("wisp attach "), "{out}");
    let (code, out, err) = wisp(&dir, port, &["crew", "update", "Zed", "--paused"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Zed: paused") && !out.contains("checkins"), "a sentence, not every field: {out}");
    let (_, out, _) = wisp(&dir, port, &["crew", "ls"]);
    assert!(out.lines().any(|l| l.starts_with("Zed") && l.contains("paused")), "{out}");

    // H9: help with words, never the wire shape
    let (_, out, _) = wisp(&dir, port, &["later", "plan", "--help"]);
    assert!(!out.contains("{kind:") && out.contains("--draft") && out.contains("Drafts"), "{out}");
    for flag in ["--kind", "--at", "--after", "--body", "--text", "--from"] {
        let line = out.lines().find(|l| l.trim_start().starts_with(flag)).unwrap_or_else(|| panic!("{flag}: {out}"));
        assert!(line.split_whitespace().count() > 2, "{flag} has words: {line}");
    }
    for (verb, flag) in [(&["review", "maybe", "--help"][..], "--id"), (&["later", "cancel-plan", "--help"], "--id")] {
        let (_, out, _) = wisp(&dir, port, verb);
        let line = out.lines().find(|l| l.trim_start().starts_with(flag)).unwrap_or_else(|| panic!("{verb:?}: {out}"));
        assert!(!line.trim_end().ends_with("required.") || line.split_whitespace().count() > 3, "{verb:?}: {line}");
    }
    let (_, out, _) = wisp(&dir, port, &["crew", "assign", "--help"]);
    assert!(out.contains("Give a crew member a job") && !out.contains("`kit: true`"), "{out}");
    let (_, out, _) = wisp(&dir, port, &["factory", "stop", "--help"]);
    assert!(out.contains("Stop a factory run") && !out.contains("`factory.stop`"), "{out}");

    // H10: one rerun names its id, not the several-runs hint
    let first =
        api.cmd("runtime.create", json!({ "prompt": "H rerun me", "cwd": dir.join("proj"), "agent": "claude", "worktree": false })).await;
    let id = first["id"].as_str().unwrap().to_owned();
    api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    let (code, out, err) = wisp(&dir, port, &["rerun", &id, "-d"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("follows it") && !out.contains("--wait waits for all"), "{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_three_tools_without_a_schema_have_one_audit_d6() {
    let dir = temp("serve-auditH-d6");
    let (_serve, port) = start(&dir);
    let http = reqwest::Client::new();
    let list = mcp(&http, port, "tools/list", json!({})).await;
    let tools = list["result"]["tools"].as_array().cloned().unwrap_or_default();
    for (name, fields) in
        [("factory_set", &["name", "stations"][..]), ("resources_plan", &["resources", "prune"]), ("resources_read", &["text", "format"])]
    {
        let t = tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("no {name}"));
        let props = t["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
        for f in fields {
            assert!(props.contains_key(*f), "{name} takes {f}: {t}");
        }
        assert!(!props.contains_key("args"), "{name}: its fields, not {{args}}: {t}");
    }
    // a field it doesn't know is refused, as every other tool's
    let r = mcp(&http, port, "tools/call", json!({ "name": "resources_read", "arguments": { "txt": "wisp: 1" } })).await;
    assert_eq!(r["result"]["isError"], true, "{r}");
    let r = mcp(&http, port, "tools/call", json!({ "name": "resources_read", "arguments": { "text": "wisp: 1\n" } })).await;
    assert_ne!(r["result"]["isError"], true, "{r}");
}
