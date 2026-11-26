//! SBX-4 (ADR-0047): a sandbox the grid made for an owner but that never came up, or whose owner couldn't be written,
//! is dropped by the make that failed: left behind it held one of the user's few sandboxes (5 at most) for good.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{Caller, Command, Manifest, Plugin, Result, Setup, WireError};
use wisp_sandbox::SandboxPlugin;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

macro_rules! cmd {
    ($name:ident, $id:literal) => {
        struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            type Args = Value;
            type Out = Value;
        }
    };
}
cmd!(List, "machines.list");
cmd!(Get, "machines.get");
cmd!(Update, "machines.update");
cmd!(New, "grid-client.sandbox");
cmd!(Remove, "grid-client.sandboxRemove");

fn view() -> Value {
    // never up: a box the grid made that doesn't answer
    json!({ "id": "m1", "peer": null, "name": "Sandbox", "transport": "grid", "address": "grid.test", "port": 443, "command": "",
        "autoConnect": true, "addedAt": 0, "kind": "grid", "target": "grid.test", "defaultCommand": "", "state": "connecting", "steps": [],
        "error": "", "hint": null, "stale": false, "attempt": 0, "since": null, "remote": null, "outdated": null, "log": [],
        "projects": 0, "working": 0, "sandbox": "sb1" })
}

/// wisp.machines with the one machine the grid made (`m1`); `refuses`: its owner can't be written.
struct Machines {
    refuses: bool,
}
impl Plugin for Machines {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.machines", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<List, _, _>(|_, _, _| async move { Ok(json!([])) });
        s.command::<Get, _, _>(|_, _, _| async move { Ok(view()) });
        let refuses = self.refuses;
        s.command::<Update, _, _>(move |_, _, _| async move {
            if refuses { Err(WireError::unavailable("the store is busy")) } else { Ok(view()) }
        });
        Ok(())
    }
}

/// wisp.grid-client: makes `sb1` (kept as machine `m1`) and says which sandboxes it was asked to remove.
struct Grid(Arc<Mutex<Vec<String>>>);
impl Plugin for Grid {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.grid-client", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<New, _, _>(|_, _, _| async move {
            Ok(json!({ "id": "sb1", "name": "app", "url": "https://sb1.grid.test", "machine": "m1" }))
        });
        let removed = self.0.clone();
        s.command::<Remove, _, _>(move |_, _, a: Value| {
            removed.lock().unwrap().push(a["id"].as_str().unwrap_or_default().to_owned());
            async move { Ok(json!({ "ok": true })) }
        });
        Ok(())
    }
}

/// `sandbox.place` for a crew job, a new sandbox; the wait's polls tick by on a fake clock. The sandboxes removed.
async fn place(refuses: bool) -> (std::result::Result<Value, WireError>, Vec<String>) {
    let clock = FakeClock::at(1_000);
    let removed = Arc::new(Mutex::new(Vec::new()));
    let k = TestApp::new()
        .clock(&clock)
        .service(Store::open(Location::Memory, Arc::new(SystemClock)).unwrap())
        .with(SandboxPlugin { inside: Some(false), home: None })
        .with(Machines { refuses })
        .with(Grid(removed.clone()))
        .start()
        .await;
    let tick = clock.clone();
    let ticker = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            tick.advance(5_000);
        }
    });
    let out = k.call(Caller::user(), "sandbox.place", json!({ "owner": { "crew": "ada", "job": "" } })).await;
    ticker.abort();
    let removed = removed.lock().unwrap().clone();
    (out, removed)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sandbox_that_never_comes_up_is_dropped_by_its_make() {
    let (out, removed) = place(false).await;
    let err = out.unwrap_err();
    assert!(err.message.contains("didn't come up"), "{err:?}");
    assert_eq!(removed, ["sb1"], "the grid's box goes with the make that failed");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sandbox_whose_owner_cant_be_written_is_dropped_by_its_make() {
    let (out, removed) = place(true).await;
    assert_eq!(out.unwrap_err().message, "the store is busy", "the owner write's own error");
    assert_eq!(removed, ["sb1"]);
}
