//! Audit D10: the CLI didn't show a task's goal (only `runtime.get`'s `ext.wisp.goal` had it), and `goal.set` on an idle
//! task without `kick` answered `status working`, though nothing judges it until its next turn. `wisp show` says the
//! goal and where it stands; `goal.set` says when it's checked.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::process::{Command, Stdio};

use common::{Api, run_task, start_with, temp};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn show_says_the_goal_and_setting_one_on_an_idle_task_says_when_its_checked() {
    let dir = temp("serve-goal-words");
    let (serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (_, task) = run_task(&api, &dir, "claude").await;
    let id = task["id"].as_str().unwrap().to_owned();
    let set = api.cmd("goal.set", json!({ "id": id, "text": "the tests pass" })).await;
    assert!(set["next"].as_str().is_some_and(|n| n.contains("next turn")), "{set}");
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(["show", &id])
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let shown = String::from_utf8_lossy(&o.stdout);
    assert!(shown.contains("◎ Goal: the tests pass") && shown.contains("checked when a turn ends"), "{shown}");
    drop(serve);
}
