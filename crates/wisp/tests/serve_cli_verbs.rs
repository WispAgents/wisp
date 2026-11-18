//! QA round 9 #1: `wisp computer on --grid wisp.localhost:4797`, what the Grid's Add a computer prints, launched an agent
//! task titled "computer on", twice. Every verb path the CLI knows, built in (`words::VERB_PATHS`) or a plugin's verb
//! from the live catalogue, with a flag it doesn't take: never a task, and a usage error (exit non-zero) for every
//! built-in path and every typed catalogue verb (an untyped one takes each flag as a field, `--args JSON`). The
//! real binary against `wisp serve` with the fake harness (nothing paid ever runs).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::json;

fn wisp(dir: &Path, port: u16, args: &[String]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `words::VERB_PATHS`, read from its source (one `vp("cmd", &[verbs…], …)` per line), so this test follows the table.
fn built_in_paths() -> Vec<Vec<String>> {
    let src = include_str!("../src/words.rs");
    let mut out = Vec::new();
    for line in src.lines().map(str::trim).filter(|l| l.starts_with("vp(\"")) {
        let cmd = line.trim_start_matches("vp(\"").split('"').next().unwrap().to_owned();
        let verbs = line.split("&[").nth(1).unwrap().split(']').next().unwrap();
        for v in verbs.split(',').map(|v| v.trim().trim_matches('"')).filter(|v| !v.is_empty()) {
            out.push(vec![cmd.clone(), v.to_owned()]);
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn no_verb_path_with_a_bad_flag_ever_becomes_a_task() {
    let dir = temp("serve-cli-verbs");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let built_in = built_in_paths();
    assert!(built_in.len() > 40, "{} built-in verb paths", built_in.len());
    let catalogue = api.cmd("cli.catalogue", json!({})).await;
    // (path, typed): a typed verb's flags come from its argument type, so a flag it doesn't have is an error; an
    // untyped one (no declared type: `--args JSON`, each flag a field) can't tell, but is never a task either
    let plugin: Vec<(Vec<String>, bool)> = catalogue
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let (ns, verbs) = wisp_cli_core::verbs::path_of(c["id"].as_str().unwrap());
            (std::iter::once(ns).chain(verbs).collect(), c.get("args").is_some_and(serde_json::Value::is_string))
        })
        .collect();
    let typed = plugin.iter().filter(|(_, t)| *t).count();
    assert!(typed > 50, "{typed} typed catalogue verbs");
    let mut wrong = Vec::new();
    // every verb refuses a flag it doesn't take (exit non-zero): a typed one by its argument type, an untyped one by
    // what its command needs (`--args JSON`); an untyped verb that needs nothing took the flag as a field and exited 0,
    // so those declare their (empty or optional) argument types
    for (path, strict) in built_in.iter().map(|p| (p, true)).chain(plugin.iter().map(|(p, _)| (p, true))) {
        let mut args = path.clone();
        args.push("--definitely-not-a-flag".into());
        let (code, out, err) = wisp(&dir, port, &args);
        if (strict && code == 0) || out.contains("launched") || err.contains("launched") {
            wrong.push(format!("wisp {} → {code}: {}{}", args.join(" "), out.trim(), err.trim()));
        }
    }
    // --grid goes with `computer on` only
    for verb in ["off", "status"] {
        let args = ["computer", verb, "--grid", "wisp.localhost:4797"].map(str::to_owned);
        let (code, _, err) = wisp(&dir, port, &args);
        if code == 0 {
            wrong.push(format!("wisp computer {verb} --grid … took it: {err}"));
        }
    }
    // past every arm (more words than the path takes): still that command's usage, not a task
    for extra in [&["computer", "on", "now", "please"][..], &["cloud", "status", "of", "my", "box"], &["hook", "install", "it"]] {
        let (code, out, err) = wisp(&dir, port, &extra.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>());
        if code != 2 || out.contains("launched") {
            wrong.push(format!("wisp {} → {code}: {out}{err}", extra.join(" ")));
        }
    }
    // `--help` on a verb path is its usage, never the command (it used to turn the computer on)
    let (code, out, err) = wisp(&dir, port, &["computer", "on", "--help"].map(str::to_owned));
    if code != 0 || !out.contains("wisp computer on|off|status") {
        wrong.push(format!("wisp computer on --help → {code}: {out}{err}"));
    }
    // a path missing what it needs says so
    let (code, _, err) = wisp(&dir, port, &["task", "history"].map(str::to_owned));
    if code != 2 || !err.contains("wisp task history needs more") {
        wrong.push(format!("wisp task history → {code}: {err}"));
    }
    // the flags these verbs do take still work, with no flag and with theirs (the 34 that took any flag: typed now)
    let proj = dir.join("proj").display().to_string();
    for args in [
        &["settings", "list"][..],
        &["channels", "status"],
        &["stuff", "list"],
        &["stuff", "find", "--query", "logo", "--limit", "3"],
        &["stuff", "uploads"],
        &["resources", "export", "--kinds", "settings"],
        &["resources", "write", "--kinds", "settings", "--format", "json"],
        &["resources", "kinds"],
        &["workflows", "list"],
        &["automations", "tasks"],
        &["worktrees", "view"],
        &["learn", "view"],
        &["plugin", "ls"],
        &["plugins", "list", "--json"],
        &["mcp", "--repo", proj.as_str()],
    ] {
        let (code, out, err) = wisp(&dir, port, &args.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>());
        if code != 0 {
            wrong.push(format!("wisp {} → {code}: {out}{err}", args.join(" ")));
        }
    }
    // a verb whose plugin waits for its first call (issues, onCommand) knows its flags too: `--project` is one
    let (code, out, err) = wisp(&dir, port, &["issues", "list", "--help"].map(str::to_owned));
    if code != 0 || !out.contains("--project") {
        wrong.push(format!("wisp issues list --help → {code}: {out}{err}"));
    }
    assert!(wrong.is_empty(), "{} took a flag they don't: \n{}", wrong.len(), wrong.join("\n"));
    let tasks = api.cmd("runtime.list", json!({})).await;
    assert_eq!(tasks.as_array().unwrap().len(), 0, "no task was made: {tasks}");
    // the usage is that command's
    let (code, _, err) = wisp(&dir, port, &["computer", "on", "--definitely-not-a-flag"].map(str::to_owned));
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--definitely-not-a-flag") && err.contains("wisp computer on|off|status"), "{err}");
    drop(serve);
}
