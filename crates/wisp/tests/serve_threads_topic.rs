//! A task's long-thread rows on its topic (ADR-0079, ADR-0096) on a real `wisp serve` with the fake agent: its first
//! look (`session:<id>`) carries its Notes, their versions, the Notes' rev and its health, so the Thread tab has them
//! on open; and a later rewrite (`wisp notes --set`, `threads.notesUpdate`) reaches a client that has the task open.
//! Before, the first look had none, the rewrite's patch of `threads_meta` found no row on the client, and the topic's
//! resync came back without them again: the Thread tab stayed empty while the CLI wrote (found on the dev server,
//! 2026-11-18).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{Duration, Instant};

use common::{Api, run_task, start_with, temp};
use serde_json::json;

/// Subscribes to `topic` and collects the stream until `until` holds (15 s at most); `then` runs once subscribed.
async fn stream(port: u16, client: &str, topic: &str, then: Option<&Api>, until: impl Fn(&str) -> bool) -> String {
    let http = reqwest::Client::new();
    let mut r =
        http.get(format!("http://127.0.0.1:{port}/api/events?proto_min=2&proto_max=2&client_id={client}&build=t")).send().await.unwrap();
    let (mut seen, at, mut subbed) = (String::new(), Instant::now(), false);
    while at.elapsed() < Duration::from_secs(15) {
        let Ok(Ok(Some(c))) = tokio::time::timeout(Duration::from_millis(300), r.chunk()).await else {
            if subbed && until(&seen) {
                break;
            }
            continue;
        };
        seen.push_str(&String::from_utf8_lossy(&c));
        if !subbed && seen.contains("welcome") {
            let ok = http
                .post(format!("http://127.0.0.1:{port}/api/events"))
                .header("content-type", "application/json")
                .header("x-wisp-client", client)
                .body(json!({ "t": "sub", "topic": topic }).to_string())
                .send()
                .await
                .unwrap();
            assert_eq!(ok.status(), 200);
            subbed = true;
            if let Some(api) = then {
                // the snapshot first, then the change (a client with the task open)
                tokio::time::sleep(Duration::from_millis(500)).await;
                let id = topic.trim_start_matches("session:");
                api.cmd("threads.notesUpdate", json!({ "id": id, "section": "state", "text": "Parser done; tests next." })).await;
            }
        }
        if subbed && until(&seen) {
            break;
        }
    }
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tasks_first_look_carries_its_notes_and_a_later_rewrite_reaches_it() {
    let dir = temp("threads-topic");
    let (_serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (_turn, task) = run_task(&api, &dir, "claude").await;
    let id = task["id"].as_str().unwrap().to_owned();
    api.cmd("threads.notesUpdate", json!({ "id": id, "section": "goal", "text": "Import CSV files into the ledger" })).await;
    api.cmd("threads.notesUpdate", json!({ "id": id, "section": "goal", "text": "Import CSV files, with headers" })).await;
    let topic = format!("session:{id}");

    // the first look: the section, both its versions and the Notes' rev, beside the task's turns
    let first = stream(port, "c-first", &topic, None, |s| s.contains("threads_meta") && s.contains("threads_revisions")).await;
    for kind in ["\"threads_notes\"", "\"threads_revisions\"", "\"threads_meta\""] {
        assert!(first.contains(kind), "{kind} on the first look: {first}");
    }
    assert!(first.contains("Import CSV files, with headers") && first.contains("Import CSV files into the ledger"), "{first}");

    // a client with the task open hears a rewrite: the section's upsert and the Notes' rev, on the task's topic
    let live =
        stream(port, "c-live", &topic, Some(&api), |s| s.contains("Parser done; tests next.") && s.matches("threads_meta").count() >= 2)
            .await;
    assert!(live.contains("Parser done; tests next."), "the rewrite reached the open task: {live}");
}
