//! ADR-0074: what wisp.machines says about a write goes out with that write's commit. The store's commit listener is
//! the reader (the rows are visible, the write hasn't returned); it asks a subscriber that answers after its handlers
//! took what came before (ADR-0073), and the event is in the answer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{Caller, Command, Manifest, Plugin, Result, Setup};
use wisp_machines::MachinesPlugin;
use wisp_machines_api::events::Removed;
use wisp_store::{ChangeOp, Location, Store, SystemClock};
use wisp_testkit::TestApp;

struct Heard;
impl Command for Heard {
    const ID: &'static str = "ear.heard";
    type Args = ();
    type Out = Vec<String>;
}

/// Hears `machines.removed`; `ear.heard` answers once its handlers took what came before.
struct Ear(Arc<Mutex<Vec<String>>>);
impl Plugin for Ear {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.ear", "version": "2.0.0", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let got = self.0.clone();
        s.on::<Removed, _, _>(move |_, e| {
            let got = got.clone();
            async move { got.lock().unwrap().push(e.id) }
        });
        let got = self.0.clone();
        s.command::<Heard, _, _>(move |ctx, _, ()| {
            let got = got.clone();
            async move {
                ctx.events_delivered(std::time::Duration::from_secs(10)).await;
                Ok(got.lock().unwrap().clone())
            }
        });
        Ok(())
    }
}

/// `machines.remove`: whoever sees the peer's row gone finds `machines.removed` already queued (written, then
/// emitted, the answer there was empty).
#[tokio::test(flavor = "multi_thread")]
async fn a_reader_who_sees_a_peer_gone_finds_machines_removed_queued() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().service(store.clone()).with(MachinesPlugin).with(Ear(Arc::default())).start().await;
    let m = k
        .call(Caller::user(), "machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": 9, "connect": false }))
        .await
        .unwrap();
    let id = m["id"].as_str().unwrap().to_owned();
    let answers = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (kk, a2, handle) = (Arc::new(Mutex::new(Some(k.clone()))), answers.clone(), tokio::runtime::Handle::current());
    let _sub = store.on_commit(move |rows| {
        if !rows.iter().any(|r| r.entity == "machines_peer" && r.op == ChangeOp::Del) {
            return;
        }
        let Some(k) = kk.lock().unwrap().clone() else { return };
        a2.lock().unwrap().push(handle.block_on(k.call(Caller::user(), "ear.heard", json!(null))).unwrap());
    });
    k.call(Caller::user(), "machines.remove", json!({ "id": id })).await.unwrap();
    assert_eq!(*answers.lock().unwrap(), vec![json!([id])], "the reader of the deleted row heard it was removed");
}
