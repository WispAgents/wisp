//! The setup audit's CLI findings (/tmp/qa-keep/audit-A-setup.md), end to end: the real binary against `wisp serve`,
//! a scratch home, the fake harness (nothing paid ever runs).
//! - F22: an enum setting with number-like values (`worktrees.max` off|20|40|80) is set from the CLI and a formula.
//! - F23: a formula naming a folder that's already a project, under another name, is refused, not a second project.
//! - F13: "Automations stop at" set in Usage is in `wisp export budget`.
//! - F18: `wisp plugin new` is listed at once (off).
//! - F20: a removed plugin isn't in `wisp export`.
//! - F29: `wisp skill install --agent all` says one thing per agent, and no error.
//! - F27: bash completion offers each word once, the built-in verbs, the harnesses and the setting keys.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp, yaml_str};
use serde_json::json;

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

#[tokio::test(flavor = "multi_thread")]
async fn the_setup_audit_s_cli_findings_hold() {
    let dir = temp("serve-setup-audit");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // F22: the CLI's 80 and a formula's 20 are the enum's "80" and "20"
    let (code, out, err) = wisp(&dir, port, &["settings", "set", "worktrees.max", "80"]);
    assert_eq!(code, 0, "{out}{err}");
    let got = api.cmd("settings.get", json!({ "key": "worktrees.max" })).await;
    assert!(got.to_string().contains("\"80\""), "{got}");
    std::fs::write(dir.join("proj/max.wisp.yml"), "wisp: 1\nsettings:\n  worktrees.max: 20\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "max.wisp.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(api.cmd("settings.get", json!({ "key": "worktrees.max" })).await.to_string().contains("\"20\""));

    // F23: a second name for a folder that's a project: refused, and no second project
    let repo = dir.join("repo2");
    std::fs::create_dir_all(&repo).unwrap();
    let (code, out, err) = wisp(&dir, port, &["add", repo.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    std::fs::write(
        dir.join("proj/dup.wisp.yml"),
        format!("wisp: 1\nprojects:\n  repo2-formula: {{ path: {} }}\n", yaml_str(repo.display().to_string())),
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "dup.wisp.yml", "--yes"]);
    assert!(code != 0 && format!("{out}{err}").contains("same path"), "{code}: {out}{err}");
    let projects = api.cmd("resources.export", json!({ "kinds": ["project"] })).await;
    assert_eq!(projects.as_array().unwrap().len(), 1, "{projects}");

    // F13: the line set in Usage is exported
    api.cmd("budget.setStop", json!({ "key": "autoStop", "value": "75" })).await;
    let (code, out, err) = wisp(&dir, port, &["export", "budget"]);
    assert!(code == 0 && out.contains("stopAt") && out.contains("75"), "{code}: {out}{err}");

    // F18: a plugin you're writing is listed at once, off
    let (code, out, err) = wisp(&dir, port, &["plugin", "new", "qa-theme"]);
    assert_eq!(code, 0, "{out}{err}");
    let list = api.cmd("plugins.list", json!({})).await.to_string();
    assert!(list.contains("user.qa-theme"), "{list}");

    // F20: switched on, then taken out: not in `wisp export`
    api.cmd("plugins.setEnabled", json!({ "id": "user.qa-theme", "enabled": true })).await;
    let (_, out, _) = wisp(&dir, port, &["export", "plugin"]);
    assert!(out.contains("user.qa-theme"), "kept while it's there: {out}");
    api.cmd("plugins.uninstall", json!({ "id": "user.qa-theme" })).await;
    let (code, out, err) = wisp(&dir, port, &["export", "plugin"]);
    assert!(code == 0 && !out.contains("user.qa-theme"), "{code}: {out}{err}");

    // F29: one outcome per agent, and no error
    let (code, out, err) = wisp(&dir, port, &["skill", "install", "--agent", "all"]);
    assert!(code == 0 && !err.contains('✗') && out.contains("opencode"), "{code}: {out}{err}");

    // F27: bash
    let (code, out, err) = wisp(&dir, port, &["completions", "bash"]);
    assert_eq!(code, 0, "{err}");
    let firsts: Vec<&str> = out.split("compgen -W '").nth(2).unwrap().split('\'').next().unwrap().split(' ').collect();
    let mut once = firsts.clone();
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), firsts.len(), "each first word once: {firsts:?}");
    assert!(out.contains("    plugin) ") && out.contains("    cloud) ") && out.contains("signin"), "{out}");
    assert!(out.contains("--agent|-a) COMPREPLY=($(compgen -W '") && out.contains("claude"), "{out}");
    assert!(out.contains("worktrees.max"), "the setting keys: {out}");
    drop(serve);
}
