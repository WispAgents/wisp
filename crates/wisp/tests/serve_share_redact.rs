//! QA round 12 #3 end to end: with a task titled "login" on this wisp, sharing another task whose prompt is "fix the
//! login bug" shares it as written. Another task is taken out only where the text refers to it (its mention chip, its
//! id). The real `wisp serve` with the fake harness (nothing paid runs).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Api, ended, start, temp};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn a_share_keeps_words_that_are_another_tasks_title_and_takes_out_a_mention_of_it() {
    let dir = temp("serve-share-redact");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = |prompt: &str| {
        let a = json!({ "prompt": prompt, "cwd": dir.join("proj"), "agent": "claude" });
        let api = &api;
        async move { api.cmd("runtime.create", a).await["id"].as_str().unwrap().to_owned() }
    };
    let login = made("login").await;
    api.until(&login, "its turn ends", |t| t.last().is_some_and(ended)).await;
    api.cmd("runtime.rename", json!({ "id": login, "title": "login" })).await;
    let fix = made(&format!("fix the login bug, as in @[login](wisp:{login}) ({login})")).await;
    api.until(&fix, "its turn ends", |t| t.last().is_some_and(ended)).await;

    let p = api.cmd("share.preview", json!({ "id": fix, "level": "report" })).await;
    let text = p["bundle"].to_string();
    assert!(text.contains("fix the login bug, as in <task> (<task>)"), "the words stay, the mention and the id go: {text}");
    assert!(!text.contains(&login), "the other task's id never leaves: {text}");
    let names: Vec<(String, String)> = p["redactions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "name")
        .map(|r| (r["was"].as_str().unwrap().to_owned(), r["now"].as_str().unwrap().to_owned()))
        .collect();
    assert!(!names.is_empty() && names.iter().all(|(_, now)| now == "<task>"), "{names:?}");
    assert!(!names.iter().any(|(was, _)| was == "login" && text.contains("fix the <task> bug")), "{names:?}");
    drop(serve);
}

/// QA round 13 #1: the Share dialog read `targets` (and `shared`, `work`) that `share.preview` never answered, and
/// crashed on open. QA round 14 #5: it draws what goes and what is hidden from this same answer. The answer the
/// dialog gets (`share.preview { id, level: "report" }`, as plugins/stuff/ui/src/share/commands.ts sends it: what
/// "Its work" sends) is kept as a golden (plugins/stuff/ui/test/goldens/share-preview.json); the dialog's readers are tested
/// on this same file
/// (`plugins/stuff/ui/test/share-preview.test.ts`), so a field one side drops fails on both.
#[tokio::test(flavor = "multi_thread")]
async fn the_share_dialogs_preview_is_the_golden_its_readers_are_tested_on() {
    let dir = temp("serve-share-preview");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = api
        .cmd(
            "runtime.create",
            json!({ "prompt": "fix the add bug, token=abcd1234efgh TOPIC[Fix add]", "cwd": dir.join("proj"), "agent": "claude" }),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "its turn ends", |t| t.last().is_some_and(ended)).await;
    let mut p = api.cmd("share.preview", json!({ "id": id, "level": "report" })).await;
    // the dialog's fields: no link yet, the files it changed, no grid signed in here (so nowhere to go)
    assert_eq!(p["shared"], serde_json::Value::Null, "{p}");
    assert_eq!(p["work"], json!({ "kind": "files" }), "{p}");
    assert_eq!(p["targets"], json!([]), "{p}");
    // what changes run to run
    let b = &mut p["bundle"];
    b["id"] = json!("share-…");
    b["createdAt"] = json!(0);
    b["version"] = json!("…");
    b["spec"]["formula"] = json!("…");
    for r in p["redactions"].as_array_mut().unwrap() {
        if r["kind"] == "path" {
            r["was"] = json!("/tmp/…/proj");
        }
    }
    // in the UI package that reads it (a package imports nothing from outside itself)
    let goldens = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/stuff/ui/test/goldens");
    wisp_testkit::assert_golden_at(&goldens, "share-preview", &p);
    drop(serve);
}
