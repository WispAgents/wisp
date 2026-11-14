//! A task the client shows at once is made under the client's id (the owner, 2026-11-14: New task → Send opened
//! `/task/local:<client id>` while the server made the task under its own id, so the page stayed blank forever).
//! `runtime.create` takes `id`, `runtime.fork` `newId` and `runtime.trial` `ids`: wisp's own shape (32 lowercase hex),
//! never a task's that exists, never a peer-prefixed key or a dashed UUID. The real binary, the fake `claude`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use serde_json::json;

const A: &str = "019a5b3c4d5e7f008000aaaaaaaaaaaa";
const B: &str = "019a5b3c4d5e7f008000bbbbbbbbbbbb";

#[tokio::test(flavor = "multi_thread")]
async fn a_task_the_client_shows_at_once_is_made_under_the_clients_id_blank_page_after_send() {
    let dir = temp("client-ids");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");

    // New task: the page the client opened is the task's
    let made = api.cmd("runtime.create", json!({ "id": A, "prompt": "fix add TOPIC[Fix add]", "cwd": proj, "agent": "claude" })).await;
    assert_eq!(made["id"], A, "{made}");
    let got = api.cmd("runtime.get", json!({ "id": A })).await;
    assert_eq!(got["id"], A, "{got}");
    api.until(A, "the first turn ends", |t| t.last().is_some_and(ended)).await;

    // the same create sent again (a pending create resent after a restart, a new mid): the task it made, not a second
    let tasks = |v: &serde_json::Value| v.as_array().map_or(0, Vec::len);
    let before = tasks(&api.cmd("runtime.list", json!({})).await);
    let again = api.cmd("runtime.create", json!({ "id": A, "prompt": "fix add TOPIC[Fix add]", "cwd": proj, "agent": "claude" })).await;
    assert_eq!(again["id"], A, "{again}");
    assert_eq!(tasks(&api.cmd("runtime.list", json!({})).await), before, "nothing made twice");

    // never a key or a dashed UUID (what the client used to send), a path or upper case: refused, nothing made
    for (bad, what) in [
        (format!("local:{B}"), "a peer-prefixed key"),
        ("019a5b3c-4d5e-7f00-8000-bbbbbbbbbbbb".to_owned(), "a dashed UUID"),
        ("../etc".to_owned(), "a path"),
        (B.to_uppercase(), "upper case"),
    ] {
        let r = api.try_cmd("runtime.create", json!({ "id": bad, "prompt": "again", "cwd": proj, "agent": "claude" })).await;
        assert_eq!(r["ok"], false, "{what}: {r}");
    }
    assert_eq!(tasks(&api.cmd("runtime.list", json!({})).await), before, "a refused id makes nothing");

    // Fork: the copy is the row the client shows
    let fork = api.cmd("runtime.fork", json!({ "id": A, "newId": B })).await;
    assert_eq!(fork["id"], B, "{fork}");
    let r = api.try_cmd("runtime.fork", json!({ "id": A, "newId": A })).await;
    assert_eq!(r["ok"], false, "a fork can't take its source's id: {r}");
    drop(serve);
}
