//! End-to-end verification of crew turn-end wiring (v1 `#turnDone` / `#ended` / `#sendFollowUps`):
//! - `{"type":"crew"}` blocks are stripped from `turn.presentation.blocks` before persisting.
//! - `topic` in a `crew` block renames the job on turn 1.
//! - `forLater: true` persists `ext["wisp.runtime"]["forLater"]` with `by: "crew"`.
//! - `remember`, `notebook`, and `wakeIn` update the member's notebook and `wakeAt`.
//! - Every finished job records an entry in `member.log`.
//! - `followUp` sends a message to a linked task, joins the member to that task's delegates, and records `sent`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Api, start_with, temp};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn crew_block_is_stripped_and_wires_memory_for_later_topic_log_and_follow_up() {
    let dir = temp("crew-turn-done");
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let block_file = dir.join("crew-block.json");

    // 1. Check-in emits a crew block with forLater, topic, remember, and wakeIn.
    std::fs::write(
        &block_file,
        serde_json::to_string(&json!({
            "type": "crew",
            "forLater": true,
            "topic": "Daily coding-agent pain check",
            "remember": ["Check-in 1: all quiet", "Release 1.0 seen"],
            "wakeIn": 45
        }))
        .unwrap(),
    )
    .unwrap();

    let (serve, port) = start_with(&dir, &[("FAKE_CREW_BLOCK_FILE", block_file.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    let made = api.cmd("crew.create", json!({ "name": "Ada", "role": "watcher", "agent": "claude", "cwd": proj })).await;
    let member_id = made["member"]["id"].as_str().or_else(|| made["id"].as_str()).unwrap().to_owned();

    let started = api.cmd("crew.checkin", json!({ "id": member_id })).await;
    let checkin_id = started["session"]["id"].as_str().unwrap().to_owned();
    api.until(&checkin_id, "check-in turn done", |t| t.len() == 1 && t[0]["status"] == "done").await;

    // Wait until the durable TaskFinished handler has updated the member's notebook and log.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let member = loop {
        let g = api.cmd("crew.get", json!({ "id": member_id })).await;
        if g["log"].as_array().is_some_and(|l| !l.is_empty()) && !g["notebook"].as_str().unwrap_or_default().is_empty() {
            break g;
        }
        assert!(std::time::Instant::now() < deadline, "timed out waiting for crew report: {g}");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    // crew block stripped from stored presentation; topic renamed the task; forLater persisted with by: "crew".
    let turns = api.cmd("runtime.turns", json!({ "id": checkin_id })).await;
    let blocks = turns[0]["presentation"]["blocks"].as_array().unwrap();
    assert!(blocks.iter().all(|b| b["type"] != "crew"), "crew block must be stripped: {blocks:?}");
    let session = api.cmd("runtime.get", json!({ "id": checkin_id })).await;
    assert_eq!(session["title"], "Daily coding-agent pain check", "crew.topic names turn 1: {session}");
    assert_eq!(session["ext"]["wisp.runtime"]["forLater"]["by"], "crew", "forLater persisted: {session}");
    assert!(session["ext"]["wisp.runtime"]["forLater"]["at"].is_i64(), "{session}");

    // Member memory (remember + wakeIn) and log updated.
    let nb = member["notebook"].as_str().unwrap();
    assert!(nb.contains("- Check-in 1: all quiet") && nb.contains("- Release 1.0 seen"), "notebook={nb:?}");
    assert!(member["wakeAt"].as_i64().is_some(), "wakeIn sets wakeAt: {member}");
    let log = member["log"].as_array().unwrap();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0]["sessionId"], checkin_id);
    assert_eq!(log[0]["forLater"], true);

    // 2. Ask Ada in a task (creates a link in m.links), and have her rewrite the notebook and send a followUp to that task.
    let _ = std::fs::remove_file(&block_file);
    let task = api.cmd("runtime.create", json!({ "prompt": "Initial work", "cwd": proj, "agent": "claude" })).await;
    let task_id = task["id"].as_str().unwrap().to_owned();
    api.until(&task_id, "initial task turn done", |t| t.len() == 1 && t[0]["status"] == "done").await;

    std::fs::write(
        &block_file,
        serde_json::to_string(&json!({
            "type": "crew",
            "notebook": "- Tidied notebook: main checked",
            "followUp": { "task": task_id, "text": "Please rerun the tests after my note." }
        }))
        .unwrap(),
    )
    .unwrap();

    let asked = api.cmd("crew.ask", json!({ "id": member_id, "sessionId": task_id, "text": "Check this task" })).await;
    let ask_job = asked["job"].as_str().unwrap().to_owned();
    api.until(&ask_job, "ask job done", |t| t.len() == 1 && t[0]["status"] == "done").await;

    // Wait for followUp turn on the linked task to land.
    let _ = std::fs::remove_file(&block_file);
    api.until(&task_id, "follow-up from crew member", |t| t.len() == 2 && t[1]["status"] == "done").await;

    let task_turns = api.cmd("runtime.turns", json!({ "id": task_id })).await;
    assert_eq!(task_turns[1]["by"], "crew", "{task_turns}");
    assert!(task_turns[1]["prompt"].as_str().unwrap_or_default().contains("Please rerun the tests after my note."), "{task_turns}");
    let task_after = api.cmd("runtime.get", json!({ "id": task_id })).await;
    let subs = task_after["ext"]["wisp.runtime"]["subagents"].as_array().unwrap();
    assert!(subs.iter().any(|s| s["crew"] == member_id), "member joins task delegates: {subs:?}");

    // Notebook rewritten, log has 2 entries, and ask_job recorded the follow-up in ext.wisp.crew.sent.
    let g2 = api.cmd("crew.get", json!({ "id": member_id })).await;
    assert_eq!(g2["notebook"], "- Tidied notebook: main checked");
    assert_eq!(g2["log"].as_array().unwrap().len(), 2);
    let ask_session = api.cmd("runtime.get", json!({ "id": ask_job })).await;
    let sent = ask_session["ext"]["wisp.crew"]["sent"].as_array().unwrap();
    assert!(sent.iter().any(|x| x["id"] == task_id), "sent recorded: {sent:?}");

    drop(serve);
}
