//! Audit A F14: Pause automations took effect, but This computer's page kept "Running as usual" (and after Resume,
//! "paused") until a reload: the snapshot row the page reads was written only at the next sample. A pause or a
//! resume writes it at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wisp_kernel::Caller;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

async fn guard(store: &Store) -> Option<Value> {
    let rows = store.read_async(|r| r.query("machine.snapshot", &wisp_store::Query::new())).await.ok()?;
    rows.first().map(|r| r["guard"].clone())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pause_and_a_resume_show_on_the_snapshot_at_once() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let clock = wisp_testkit::FakeClock::at(1_700_000_000_000);
    let k = TestApp::new().clock(&clock).service(store.clone()).service(wisp_proc::local()).with(wisp_machine::MachinePlugin).start().await;
    // the first sample (its readings take a moment): its row is the page's
    let deadline = Instant::now() + Duration::from_secs(30);
    while guard(&store).await.is_none() {
        assert!(
            Instant::now() < deadline,
            "no first snapshot: {:?}",
            store.read_async(|r| r.query("machine.load", &wisp_store::Query::new())).await.map(|v| v.len())
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(guard(&store).await.unwrap()["pausedUntil"], json!(0));
    let until = 1_700_000_000_000_i64 + 3_600_000;
    k.call(Caller::user(), "machine.automations.pause", json!({ "until": until })).await.unwrap();
    // no wait: the command wrote it
    let g = guard(&store).await.unwrap();
    assert_eq!(g["pausedUntil"], json!(until), "{g}");
    assert_eq!(g["held"], json!("paused"), "{g}");
    k.call(Caller::user(), "machine.automations.pause", json!({ "until": 0 })).await.unwrap();
    assert_eq!(guard(&store).await.unwrap()["pausedUntil"], json!(0));
}
