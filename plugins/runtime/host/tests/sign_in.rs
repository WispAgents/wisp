//! A run that failed signed out (v1 sign-in-card.js, F-comp-42): Sign in on the turn arms it, and when that harness's
//! login lands (`runtime.signInDone`, from wisp.accounts) the request is sent again, once, in the same task. Another
//! harness's login, or a turn that didn't fail signed out, sends nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::atomic::Ordering;

use common::{SIGNED_IN, rig, until};
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::model::TurnStatus;

#[tokio::test(flavor = "multi_thread")]
async fn a_signed_out_run_is_sent_again_once_its_login_lands_v1_sign_in_rerun() {
    let r = rig().await;
    let id =
        r.rt.create(StartArgs { prompt: "signedout fix the login".into(), cwd: Some(common::tmp()), ..StartArgs::default() })
            .await
            .unwrap()
            .id
            .0;
    until("the turn to fail", || async {
        r.rt.turns(&id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Error)
    })
    .await;
    let failed = r.rt.turns(&id, None, Some(1)).await.unwrap().pop().unwrap();
    assert_eq!(failed.ext["wisp.runtime"]["authFailed"], true, "the turn shows the sign-in card");
    assert!(r.rt.sign_in(&id, &failed.id).await.unwrap(), "Sign in arms it");
    // another harness's login: nothing goes
    r.rt.sign_in_done("codex", "").await.unwrap();
    assert_eq!(r.sim.spawned().len(), 1);
    SIGNED_IN.store(true, Ordering::SeqCst);
    r.rt.sign_in_done("fake", "").await.unwrap();
    until("the request to go again", || async { r.sim.spawned().len() == 2 }).await;
    until("the rerun to finish", || async {
        r.rt.turns(&id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Done)
    })
    .await;
    // once: a second login doesn't send it a third time
    r.rt.sign_in_done("fake", "").await.unwrap();
    assert_eq!(r.sim.spawned().len(), 2);
    assert_eq!(r.sim.spawned()[1].args[0], "signedout fix the login", "the same request");
}

/// QA desktop r2 #6: a run whose sign-in had expired died with "unexpected status 401 Unauthorized: Missing bearer…" on
/// stderr and no result line, and the list showed that raw line as the task's subtitle: only a result line could say
/// signed out. The harness reads its own words on any failed end: the turn shows the sign-in card (Sign in), and its
/// answer says it plainly, not the raw API error.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_that_dies_with_its_sign_in_expired_says_signed_out_not_the_raw_error() {
    let r = rig().await;
    let id =
        r.rt.create(StartArgs { prompt: "expired hello".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id.0;
    until("the turn to fail", || async {
        r.rt.turns(&id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Error)
    })
    .await;
    let t = r.rt.turns(&id, None, Some(1)).await.unwrap().pop().unwrap();
    assert_eq!(t.ext["wisp.runtime"]["authFailed"], true, "the sign-in card: {:?}", t.ext);
    let hero = t.presentation.as_ref().and_then(|p| p.blocks.iter().find(|b| b["type"] == "hero")).cloned().unwrap();
    let summary = hero["summary"].as_str().unwrap_or_default();
    assert!(!summary.contains("401") && !summary.contains("bearer"), "plain words, not the API error: {summary}");
    assert!(summary.to_lowercase().contains("sign"), "{summary}");
}

/// QA desktop r3 (r2 #6 still open): a task saved before signed-out ends were read on a failed end kept the raw
/// "unexpected status 401 Unauthorized…" as its line in the list. On start, the last turn of a failed task is read again
/// with its harness's own sign-in check: it says signed out in plain words, and its row's pill is Signed out.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_saved_with_a_raw_401_reads_signed_out_after_a_start() {
    let r = rig().await;
    let id =
        r.rt.create(StartArgs { prompt: "expired legacy hello".into(), cwd: Some(common::tmp()), ..StartArgs::default() })
            .await
            .unwrap()
            .id
            .0;
    until("the turn to fail", || async {
        r.rt.turns(&id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Error)
    })
    .await;
    let before = r.rt.turns(&id, None, Some(1)).await.unwrap().pop().unwrap();
    assert!(before.ext.get("wisp.runtime").and_then(|x| x.get("authFailed")).is_none_or(|v| v != true), "saved as an older wisp did");
    common::LEGACY_KNOWN.store(true, Ordering::SeqCst);
    r.rt.start().await.unwrap();
    let t = r.rt.turns(&id, None, Some(1)).await.unwrap().pop().unwrap();
    assert_eq!(t.ext["wisp.runtime"]["authFailed"], true, "{:?}", t.ext);
    let hero = t.presentation.as_ref().and_then(|p| p.blocks.iter().find(|b| b["type"] == "hero")).cloned().unwrap();
    assert!(!hero["summary"].as_str().unwrap_or_default().contains("401"), "{hero}");
    let row = r.rt.list(&wisp_runtime_api::commands::ListArgs::default()).into_iter().find(|x| x.id.0 == id).unwrap();
    let row = serde_json::to_value(&*row).unwrap();
    assert!(!row.to_string().contains("401 Unauthorized: Missing"), "no raw 401 on the row: {row}");
    assert_eq!(row["halt"], "signedOut", "{row}");
}
