//! Setup audit D (docs/qa/audit-D-setup.md), end to end: the real binary against `wisp serve`, a scratch home, the fake
//! harnesses (nothing paid ever runs, no registry is reached).
//! - D12: a formula naming a project's folder by another spelling (a link to it, macOS's `/private/tmp`) is refused.
//! - D3: a hand-written `worktrees.max: off` (YAML 1.1 reads it as false) applies as the enum's "off".
//! - D2: with Auto off, `wisp export router`, validate and apply don't take the router either.
//! - D5: `wisp settings get` of a key no plugin declares says so.
//! - D9: a formula file or a v1 home that isn't there is said in words, never "os error 2".
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

async fn project_names(api: &Api) -> Vec<String> {
    let all = api.cmd("resources.export", json!({ "kinds": ["project"] })).await;
    all.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_formula_is_read_as_the_ui_reads_it_audit_d() {
    let dir = temp("serve-setup-d");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // D12: a project, then a formula naming its folder through a link (and on macOS as /private/…): one project
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let (code, out, err) = wisp(&dir, port, &["add", repo.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let mut others: Vec<String> = Vec::new();
    #[cfg(unix)]
    {
        let link = dir.join("repo-link");
        std::os::unix::fs::symlink(&repo, &link).unwrap();
        others.push(link.display().to_string());
    }
    let shown = repo.display().to_string();
    if cfg!(target_os = "macos") && (shown.starts_with("/var/") || shown.starts_with("/tmp/")) {
        others.push(format!("/private{shown}"));
    }
    for other in &others {
        std::fs::write(dir.join("proj/again.wisp.yml"), format!("wisp: 1\nprojects:\n  repo-again: {{ path: {} }}\n", yaml_str(other)))
            .unwrap();
        let (code, out, err) = wisp(&dir, port, &["apply", "-f", "again.wisp.yml", "--yes"]);
        assert!(code != 0 && format!("{out}{err}").contains("same path"), "{other}: {code}: {out}{err}");
        assert_eq!(project_names(&api).await, ["repo"], "{other}: no second project");
    }

    // D3: the word the error listed, written bare (YAML 1.1 reads it as false), is the enum's "off"
    std::fs::write(dir.join("proj/off.wisp.yml"), "wisp: 1\nsettings:\n  worktrees.max: off\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "off.wisp.yml", "--dry-run"]);
    assert!(code == 0 && !format!("{out}{err}").contains("false"), "the plan shows \"off\": {code}: {out}{err}");
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "off.wisp.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(api.cmd("settings.get", json!({ "key": "worktrees.max" })).await, json!("off"));
    let (_, out, err) = wisp(&dir, port, &["apply", "-f", "off.wisp.yml", "--dry-run"]);
    assert!(!format!("{out}{err}").contains("worktrees.max"), "applied, it plans as unchanged: {out}{err}");

    // D2: Auto off (its default): no kind, export, validate or apply takes the router
    let kinds = api.cmd("resources.kinds", json!({})).await;
    assert!(!kinds.as_array().unwrap().iter().any(|k| k["kind"] == "router"), "{kinds}");
    let (code, out, err) = wisp(&dir, port, &["export", "router"]);
    assert!(code != 0 && !out.contains("best2"), "{code}: {out}{err}");
    std::fs::write(dir.join("proj/router.wisp.yml"), "wisp: 1\nrouter:\n  best2: true\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "router.wisp.yml"]);
    assert!(code != 0, "{out}{err}");
    // audit E6: it says why (Auto is off), never that no plugin owns `router:`; Setup → Import's read says the same
    for said in [format!("{out}{err}"), wisp(&dir, port, &["apply", "-f", "router.wisp.yml", "--yes"]).2] {
        assert!(said.contains("`router:`: Auto is off (runtime.auto)") && !said.contains("no plugin"), "{said}");
    }
    let read = api.try_cmd("resources.read", json!({ "text": "wisp: 1\nrouter:\n  best2: true\n", "format": "yaml" })).await;
    assert!(read.to_string().contains("Auto is off") && !read.to_string().contains("no plugin"), "{read}");
    let plan =
        api.try_cmd("resources.plan", json!({ "resources": [{ "kind": "router", "name": "default", "spec": { "best2": true } }] })).await;
    assert!(plan.to_string().contains("Auto is off"), "a plan naming it says why: {plan}");
    // on, it's a kind as before
    api.cmd("settings.set", json!({ "key": "runtime.auto", "value": true })).await;
    let (code, out, err) = wisp(&dir, port, &["export", "router"]);
    assert!(code == 0 && out.contains("router"), "{code}: {out}{err}");
    api.cmd("settings.set", json!({ "key": "runtime.auto", "value": false })).await;

    // audit E2: a setting not set yet plans from its default: `crew.tint: false` says `true → false`, its default again
    // changes nothing, and `wisp diff` says the value
    let plan = api
        .cmd(
            "resources.plan",
            json!({ "resources": [
            { "kind": "setting", "name": "crew.tint", "spec": false },
            { "kind": "setting", "name": "budget.drainWatch", "spec": false },
        ] }),
        )
        .await;
    let ops = plan["ops"].as_array().unwrap();
    assert_eq!(ops.len(), 1, "drainWatch: false is its default: {plan}");
    assert_eq!(
        (ops[0]["name"].clone(), ops[0]["changes"][0]["from"].clone(), ops[0]["changes"][0]["to"].clone()),
        (json!("crew.tint"), json!(true), json!(false)),
        "{plan}"
    );
    std::fs::write(dir.join("proj/tint.wisp.yml"), "wisp: 1\nsettings:\n  crew.tint: false\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["diff", "-f", "tint.wisp.yml"]);
    assert!(code == 0 && out.contains("setting/crew.tint") && out.contains("true → false"), "{code}: {out}{err}");

    // D5: a key no plugin declares
    let (code, out, err) = wisp(&dir, port, &["settings", "get", "nope.key"]);
    assert!(code != 0 && format!("{out}{err}").contains("no plugin declares the setting nope.key"), "{code}: {out}{err}");

    // D9: what isn't there, in words
    for args in
        [&["validate", "-f", "nope.yml"][..], &["apply", "-f", "nope.yml", "--yes"], &["import", "/wisp-no-such-v1-home", "--dry-run"]]
    {
        let (code, out, err) = wisp(&dir, port, args);
        let said = format!("{out}{err}");
        assert!(
            code != 0 && !said.contains("os error") && (said.contains("No file at") || said.contains("No folder at")),
            "{args:?}: {said}"
        );
    }
    drop(serve);
}
