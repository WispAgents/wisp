//! Setup audit B (docs/qa/audit-B-setup.md), end to end: the real binary against `wisp serve`, a scratch home, the fake
//! harnesses (nothing paid ever runs, no registry is reached).
//! - B2: a project at a folder that isn't there gets in through neither `wisp import` nor `wisp validate`/`apply`.
//! - B3: an imported v1 task in a project's folder is that project's.
//! - B11: v1's default harness is New task's pick (which the CLI runs on too), its subagent model Usage's setting.
//! - B6: with Auto off, `wisp export` and Setup carry no `router:`, and `wisp help` and completion don't offer it.
//! - B10: completion offers every harness and an enum setting's values; an unknown `--agent` is refused, no task made.
//! - B5: the harness version check asks npm on demand (Re-check) through `WISP_NPM`, and only while it's on.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::{Duration, Instant};

use common::{Api, start, temp};
#[cfg(unix)]
use common::{start_with, yaml_str};
use serde_json::{Value, json};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The projects here, by name, with their paths.
async fn projects(api: &Api) -> Vec<(String, String)> {
    let all = api.cmd("resources.export", json!({ "kinds": ["project"] })).await;
    all.as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap().to_owned(), p.pointer("/spec/path").and_then(Value::as_str).unwrap_or("").to_owned()))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_import_takes_only_folders_that_are_there_files_its_tasks_and_sets_one_default_harness() {
    let dir = temp("serve-setup-b-import");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let v1 = dir.join("v1home");
    std::fs::create_dir_all(v1.join("sessions")).unwrap();
    let cfg = json!({
        "projects": [{ "name": "Old App", "path": repo }, { "name": "gone", "path": dir.join("v1-gone-folder") }],
        "defaults": { "agent": "codex", "subagentModel": "haiku", "effort": "high" }
    });
    std::fs::write(v1.join("config.json"), cfg.to_string()).unwrap();
    let task = json!({
        "id": "v1sess0001", "title": "A v1 task", "titleAuto": false, "agent": "claude", "model": "", "effort": "", "mode": "edits",
        "cwd": repo, "state": "done", "unread": false, "createdAt": 1, "updatedAt": 2, "queue": [],
        "turns": [{ "id": "v1sess0001-t1", "prompt": "fix", "agent": "claude", "model": "", "effort": "", "mode": "edits", "attachments": [],
            "startedAt": 1, "endedAt": 2, "status": "done", "raw": "done", "activity": [], "changes": [] }]
    });
    std::fs::write(v1.join("sessions/v1sess0001.json"), task.to_string()).unwrap();

    // B2: the plan says the missing folder's project is left out, and the import goes on without it
    let (code, out, err) = wisp(&dir, port, &["import", v1.to_str().unwrap(), "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("project gone not imported: No folder at") && !out.contains("project/gone"), "{out}");
    let (code, out, err) = wisp(&dir, port, &["import", v1.to_str().unwrap(), "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    let ps = projects(&api).await;
    assert!(!ps.iter().any(|(n, _)| n == "gone"), "{ps:?}");
    // by its last component: a backslash before it on Windows
    let (name, _) = ps.iter().find(|(_, p)| Path::new(p).ends_with("repo")).cloned().unwrap_or_else(|| panic!("{ps:?}"));

    // B3: the imported task in that folder is that project's
    let list = api.cmd("runtime.list", json!({})).await;
    let row = list.as_array().unwrap().iter().find(|r| r["id"] == "v1sess0001").cloned().unwrap_or_else(|| panic!("{list}"));
    assert_eq!(row["project"], json!(name), "{row}");

    // B11: one default harness (New task's pick, which a task naming none runs on) and Usage's subagent model
    assert_eq!(api.cmd("settings.get", json!({ "key": "runtime.launch" })).await["agent"], "codex");
    assert_eq!(api.cmd("settings.get", json!({ "key": "runtime.subagentModel" })).await, json!("haiku"));
    let profiles = api.cmd("runtime.profiles", json!({})).await;
    let def = profiles.as_array().unwrap().iter().find(|p| p["name"] == "default").cloned().unwrap_or(Value::Null);
    assert!(def["harness"].as_str().is_none_or(str::is_empty), "the profile names no harness of its own: {def}");

    // B2: a formula's project at a missing folder is refused by validate and apply, as Projects' Add refuses it
    let ghost = dir.join("ghost-folder");
    std::fs::write(
        dir.join("proj/ghost.wisp.yml"),
        format!("wisp: 1\nprojects:\n  ghost: {{ path: {} }}\n", yaml_str(ghost.display().to_string())),
    )
    .unwrap();
    for args in [
        &["validate", "-f", "ghost.wisp.yml"][..],
        &["apply", "-f", "ghost.wisp.yml", "--dry-run"],
        &["apply", "-f", "ghost.wisp.yml", "--yes"],
    ] {
        let (code, out, err) = wisp(&dir, port, args);
        assert!(code != 0 && format!("{out}{err}").contains(&format!("No folder at {}", ghost.display())), "{args:?} → {code}: {out}{err}");
    }
    assert!(!projects(&api).await.iter().any(|(n, _)| n == "ghost"));
    // a file isn't a folder either; a folder that's there is a project
    std::fs::write(
        dir.join("proj/file.wisp.yml"),
        format!("wisp: 1\nprojects:\n  afile: {{ path: {} }}\n", yaml_str(dir.join("proj/math.js").display().to_string())),
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "file.wisp.yml"]);
    assert!(code != 0 && format!("{out}{err}").contains("is a file, not a folder"), "{code}: {out}{err}");
    std::fs::create_dir_all(&ghost).unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "ghost.wisp.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(projects(&api).await.iter().any(|(n, _)| n == "ghost"));
    drop(serve);
}

#[tokio::test(flavor = "multi_thread")]
async fn with_auto_off_no_formula_setup_help_or_completion_offers_it_and_unknown_harnesses_are_refused() {
    let dir = temp("serve-setup-b-cli");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // B6: Auto off (its default): no router in the formula, help or completion
    let kinds = |v: &Value| v.as_array().unwrap().iter().map(|r| r["kind"].as_str().unwrap_or("").to_owned()).collect::<Vec<_>>();
    assert!(!kinds(&api.cmd("resources.export", json!({})).await).contains(&"router".to_owned()));
    let (code, out, err) = wisp(&dir, port, &["export"]);
    assert!(code == 0 && !out.contains("router:"), "{out}{err}");
    let (_, help, _) = wisp(&dir, port, &["help"]);
    let (_, bash, _) = wisp(&dir, port, &["completions", "bash"]);
    assert!(!help.lines().any(|l| l.trim_start().starts_with("wisp router ")), "{help}");
    assert!(!bash.contains("    router) "), "completion offers no router");
    // on, all of it is back
    api.cmd("settings.set", json!({ "key": "runtime.auto", "value": true })).await;
    assert!(kinds(&api.cmd("resources.export", json!({})).await).contains(&"router".to_owned()));
    let (_, help, _) = wisp(&dir, port, &["help"]);
    assert!(help.lines().any(|l| l.trim_start().starts_with("wisp router ")), "{help}");
    api.cmd("settings.set", json!({ "key": "runtime.auto", "value": false })).await;

    // B10: every harness after --agent, and an enum setting's values after its key
    let (code, bash, err) = wisp(&dir, port, &["completions", "bash"]);
    assert_eq!(code, 0, "{err}");
    let agents = bash.split("--agent|-a) COMPREPLY=($(compgen -W '").nth(1).unwrap_or("").split('\'').next().unwrap_or("").to_owned();
    for h in ["claude", "codex", "opencode", "cursor", "grok"] {
        assert!(agents.split(' ').any(|a| a.starts_with(h)), "{h} after --agent: {agents}");
    }
    assert!(bash.contains("worktrees.max) COMPREPLY=($(compgen -W 'off 20 40 80'"), "worktrees.max's values");
    // an unknown harness: refused before anything starts, the harnesses named
    let (code, out, err) = wisp(&dir, port, &["--no-open", "fix the add bug", "--agent", "frob"]);
    assert!(code == 2 && err.contains("no harness frob") && err.contains("claude") && !out.contains("launched"), "{code}: {out}{err}");
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    // a harness that's here still launches
    let (code, out, err) = wisp(&dir, port, &["--no-open", "fix the add bug", "--agent", "codex"]);
    assert!(code == 0 && out.contains("launched"), "{code}: {out}{err}");
    drop(serve);
}

/// B5: every `wisp serve` asked the npm registry for Claude Code's, Codex's and opencode's latest versions. The check
/// runs on demand (Re-check) and daily from a minute after start, through `WISP_NPM` here, and only while it's on
/// (`WISP_UPDATE_CHECK=0`, which every other test server has, `CI`, or Settings: `upgrade::checks_on`'s unit test).
/// The wait here is well inside that first minute, so only Re-check can have asked.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn the_harness_version_check_asks_npm_on_demand_while_it_is_on() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp("serve-setup-b-npm");
    let log = dir.join("npm.log");
    let npm = dir.join("fake-npm");
    std::fs::write(&npm, format!("#!/bin/sh\necho \"$@\" >> '{}'\necho 9.9.10\n", log.display())).unwrap();
    std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (serve, port) = start_with(&dir, &[("WISP_UPDATE_CHECK", "1"), ("CI", ""), ("WISP_NPM", npm.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    api.cmd("accounts.recheck", Value::Null).await;
    let at = Instant::now();
    while !std::fs::read_to_string(&log).unwrap_or_default().contains("view @anthropic-ai/claude-code version") {
        assert!(at.elapsed() < Duration::from_secs(30), "Re-check asked npm: {}", std::fs::read_to_string(&log).unwrap_or_default());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(serve);
}
