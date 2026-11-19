//! Audit E7: Check in now on a paused member queued a check-in without a word, and a second click said "Ada is already
//! checking in." (a WARN in the log each tick). Now Check in now says the member is paused and queues nothing (v1:
//! "Paused members skip check-ins"); resumed, it checks in.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Api, start_with, temp};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_member_says_so_on_check_in_and_queues_nothing_audit_e7() {
    let dir = temp("crew-paused");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let (serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = api.cmd("crew.create", json!({ "name": "Ada", "agent": "claude", "cwd": dir.join("proj"), "paused": true })).await;
    let id = made["member"]["id"].as_str().or_else(|| made["id"].as_str()).unwrap_or_else(|| panic!("{made}")).to_owned();

    // Check in now: refused with why, twice the same, nothing queued
    for _ in 0..2 {
        let r = api.try_cmd("crew.checkin", json!({ "id": id })).await;
        assert_eq!(r["ok"], false, "{r}");
        let said = r.to_string();
        assert!(said.contains("Ada is paused") && !said.contains("already checking in"), "{r}");
    }
    // resumed: Check in now starts one
    api.cmd("crew.update", json!({ "id": id, "paused": false })).await;
    let r = api.cmd("crew.checkin", json!({ "id": id })).await;
    assert!(r["session"].is_object(), "{r}");
    drop(serve);
}
