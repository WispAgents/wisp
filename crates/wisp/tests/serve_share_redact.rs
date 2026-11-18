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
