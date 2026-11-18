//! Long threads end to end (ADR-0079, ADR-0096, ADR-0097) on a real `wisp serve` with the fake agent: a formula's
//! task declares its Notes (`wisp validate` refuses an unknown section), its run starts with them; `wisp notes` shows
//! and rewrites them; `wisp fresh --print` shows the brief; `wisp fresh` starts a new conversation from it (the CLI
//! is started without `--resume`, on the brief, with nothing of the old turns), and the next reply resumes it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, argv, records, start_with, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env_remove("WISP_TASK")
        .env("NO_COLOR", "1")
        .env("COLUMNS", "100")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn formula(proj: &Path, notes: &str) -> String {
    format!(
        "wisp: 1\ntasks:\n  - name: import-csv\n    prompt: build the CSV importer\n    project: {}\n    agent: claude\n    notes:\n{notes}",
        proj.display()
    )
}

/// The fake claude's runs that had a prompt (its detect probes have none), in order.
fn runs(rec: &Path) -> Vec<Value> {
    records(&rec.join("claude.jsonl")).into_iter().filter(|r| r["stdin"].as_str().is_some_and(|s| !s.is_empty())).collect()
}

fn notes_of(n: &Value, id: &str) -> String {
    n["sections"].as_array().unwrap().iter().find(|s| s["id"] == id).and_then(|s| s["text"].as_str()).unwrap_or("").to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_formulas_notes_then_wisp_notes_and_wisp_fresh() {
    let dir = temp("serve-threads");
    let proj = dir.join("proj");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let rec_s = rec.display().to_string();
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec_s.as_str())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // an unknown section is refused before anything is applied
    std::fs::write(dir.join("bad.yml"), formula(&proj, "      mood: fine\n")).unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "bad.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("mood"), "{out}{err}");
    let good = formula(
        &proj,
        "      goal: Import CSV files into the ledger\n      constraints:\n        - No new dependencies\n        - Keep the public API\n",
    );
    std::fs::write(dir.join("wisp.yml"), good).unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "wisp.yml"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["apply", "-f", "wisp.yml", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");

    // the declared task's run starts with its Notes
    let (code, out, err) = wisp(&dir, port, &["run", "import-csv"]);
    assert_eq!(code, 0, "{out}{err}");
    let id = out.split("wisp attach ").nth(1).unwrap().trim().to_owned();
    api.until(&id, "the first turn ends", |t| t.last().is_some_and(common::ended)).await;
    let n = api.cmd("threads.notes", json!({ "id": id })).await;
    assert_eq!(notes_of(&n, "goal"), "Import CSV files into the ledger", "{n}");
    assert_eq!(notes_of(&n, "constraints"), "No new dependencies\nKeep the public API");
    assert_eq!(api.cmd("runtime.spec", json!({ "id": id })).await["notes"]["goal"], "Import CSV files into the ledger", "a rerun has them");

    // wisp notes shows them and rewrites a section
    let (code, out, err) = wisp(&dir, port, &["notes", &id, "--set", "decisions=Parse with the csv crate"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("## Goal\nImport CSV files into the ledger") && out.contains("## Decisions\n- Parse with the csv crate"), "{out}");
    let (code, out, _) = wisp(&dir, port, &["notes", &id, "--set", "mood=fine"]);
    assert_ne!(code, 0, "{out}");

    // the brief, then a fresh thread from it
    let (code, brief, err) = wisp(&dir, port, &["fresh", &id, "--print"]);
    assert_eq!(code, 0, "{err}");
    assert!(brief.starts_with("# Fresh thread") && brief.contains("### Decisions\n- Parse with the csv crate"), "{brief}");
    assert!(brief.contains("**Asked:** build the CSV importer") && !brief.contains("```wisp"), "{brief}");
    let before = runs(&rec).len();
    let (code, out, err) = wisp(&dir, port, &["fresh", &id]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Fresh thread started"), "{out}");
    let turns = api.until(&id, "the fresh turn ends", |t| t.len() == 2 && t.last().is_some_and(common::ended)).await;
    assert_eq!(turns[1]["ext"]["wisp.runtime"]["fresh"], true, "{:?}", turns[1]["ext"]);
    let all = runs(&rec);
    assert_eq!(all.len(), before + 1);
    let fresh = &all[before];
    assert!(!argv(fresh).iter().any(|a| a == "--resume"), "a new conversation: {:?}", argv(fresh));
    let said = fresh["stdin"].as_str().unwrap();
    assert!(said.contains("# Fresh thread") && !said.contains("<task-notes"), "the brief has the Notes: {said}");

    // the next reply resumes the new conversation, with the note as usual
    let (code, out, err) = wisp(&dir, port, &["reply", &id, "and the tests"]);
    assert_eq!(code, 0, "{out}{err}");
    api.until(&id, "the reply ends", |t| t.len() == 3 && t.last().is_some_and(common::ended)).await;
    let next = runs(&rec).pop().unwrap();
    assert!(argv(&next).iter().any(|a| a == "--resume"), "{:?}", argv(&next));
    assert!(next["stdin"].as_str().unwrap().contains("<task-notes"));
    drop(serve);
}
