//! Audit G (2026-11-20), the CLI's words left open, end to end on a real `wisp serve` with the fake agent:
//!   - G12: `wisp crew pause <member>` and `wisp crew resume <member>`, by name or id (only `--paused false` resumed one).
//!   - G16: `wisp goal set <task> nevr` (a typo of the declared `never`) became the goal "nevr", where `--goal nevr` is
//!     refused: the word is a declared goal's name, refused when none has it; `--text "…"` sets words.
//!   - G17: `wisp fresh <id> -d` printed the usage line without naming `-d`.
//!   - E11: no command's `--help` (or tool description) carries a note written for wisp's developers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, run_task, start, start_with, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, cwd: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env_remove("WISP_TASK")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

async fn goal(api: &Api, id: &str) -> Value {
    api.cmd("runtime.get", json!({ "id": id })).await["ext"]["wisp.goal"].clone()
}

async fn paused(api: &Api, id: &str) -> bool {
    let m = api.cmd("crew.get", json!({ "id": id })).await;
    m["member"]["paused"].as_bool().or_else(|| m["paused"].as_bool()).unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread")]
async fn crew_pause_and_resume_take_a_member_by_name_or_id_audit_g12() {
    let dir = temp("serve-auditG-crew");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = api.cmd("crew.create", json!({ "name": "Ada", "agent": "claude", "cwd": dir.join("proj") })).await;
    let id = made["member"]["id"].as_str().or_else(|| made["id"].as_str()).unwrap_or_else(|| panic!("{made}")).to_owned();
    let (code, out, err) = wisp(&dir, &dir, port, &["crew", "pause", "ada"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Ada is paused"), "{out}{err}");
    assert!(paused(&api, &id).await, "paused by name");
    let (code, out, err) = wisp(&dir, &dir, port, &["crew", "resume", &id]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Ada is back"), "{out}{err}");
    assert!(!paused(&api, &id).await, "resumed by id");
    let (code, out, err) = wisp(&dir, &dir, port, &["crew", "resume"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("wisp crew resume <member>"), "{out}{err}");
    let (code, out, err) = wisp(&dir, &dir, port, &["crew", "pause", "nobody"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(!out.contains("launched"), "never a task: {out}{err}");
    assert!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().is_empty(), "no task was started");
}

#[tokio::test(flavor = "multi_thread")]
async fn goal_set_takes_a_declared_name_and_text_takes_words_audit_g16() {
    let dir = temp("serve-auditG-goal");
    let (_serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::write(
        dir.join("goals.yml"),
        "wisp: 1\ngoals:\n  never: { text: the moon is cheese }\n  green-build: { text: npm test passes }\n",
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, &dir, port, &["apply", "-f", "goals.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    let (_, task) = run_task(&api, &dir, "claude").await;
    let id = task["id"].as_str().unwrap().to_owned();
    // a typo of a declared name: refused with the names and the way to words; nothing set
    let (code, out, err) = wisp(&dir, &dir, port, &["goal", "set", &id, "nevr"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("no goal named nevr") && err.contains("green-build, never"), "{out}{err}");
    assert!(err.contains("--text"), "it says how to set words: {err}");
    assert!(goal(&api, &id).await.get("text").is_none(), "nothing set: {}", goal(&api, &id).await);
    // the name: that goal
    let (code, out, err) = wisp(&dir, &dir, port, &["goal", "set", &id, "never"]);
    assert_eq!(code, 0, "{out}{err}");
    let g: Value = goal(&api, &id).await;
    assert_eq!((g["name"].as_str(), g["text"].as_str()), (Some("never"), Some("the moon is cheese")), "{g}");
    // --text: words, even one that is a name
    let (code, out, err) = wisp(&dir, &dir, port, &["goal", "set", &id, "--text", "nevr"]);
    assert_eq!(code, 0, "{out}{err}");
    let g: Value = goal(&api, &id).await;
    assert_eq!((g["text"].as_str(), g.get("name").and_then(Value::as_str).unwrap_or_default()), (Some("nevr"), ""), "{g}");
}

#[tokio::test(flavor = "multi_thread")]
async fn fresh_takes_detach_as_launches_do_audit_g17() {
    let dir = temp("serve-auditG-fresh");
    let (_serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (_, task) = run_task(&api, &dir, "claude").await;
    let id = task["id"].as_str().unwrap().to_owned();
    for flag in ["-d", "--detach"] {
        let (code, out, err) = wisp(&dir, &dir, port, &["fresh", &id, flag]);
        assert_eq!(code, 0, "{flag}: {out}{err}");
        assert!(out.contains("Fresh thread") && !out.contains("launched"), "{flag}: {out}{err}");
    }
}

/// The notes a doc must not carry in `--help` or a tool's description (audit E11, G17): written for wisp's developers.
/// Audit H (E11 left over): ADR numbers anywhere, v1's files and routes (`v1 crew.js`, `v1 /app-state`, `PATCH
/// /sessions/:id`, `POST /connectors`), a wire shape for words (`{kind: task, body}`), and "unknown fields are ignored"
/// (they are refused).
const DEV_NOTES: [&str; 20] = [
    "for a frame",
    "Additive (",
    "(v1 ",
    "v1's",
    "ADR-",
    "(audit ",
    "(review ",
    "(owner-list",
    "(B2",
    "(B4",
    "QA round",
    "round 2",
    "v1 ",
    "POST /",
    "PATCH /",
    "GET /",
    "/app-state",
    "unknown fields are ignored",
    "`{kind:",
    "{kind: ",
];

#[tokio::test(flavor = "multi_thread")]
async fn no_commands_help_carries_a_developers_note_audit_e11() {
    let dir = temp("serve-auditG-docs");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let list: Vec<wisp_kernel::CommandInfo> = serde_json::from_value(api.cmd("cli.catalogue", json!({})).await).unwrap();
    let mut found = Vec::new();
    for c in &list {
        let spec = wisp_kernel::decl::parse(c.args.as_deref());
        for (what, doc) in std::iter::once(("", spec.doc.as_str())).chain(spec.fields.iter().map(|f| (f.name.as_str(), f.doc.as_str()))) {
            if let Some(n) = DEV_NOTES.iter().find(|n| doc.contains(*n)) {
                found.push(format!("{} {what}: {n:?} in {doc:?}", c.id));
            }
        }
    }
    assert!(found.is_empty(), "{} notes left:\n{}", found.len(), found.join("\n"));
}
