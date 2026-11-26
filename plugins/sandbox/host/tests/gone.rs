//! ADR-0047: a deleted lead takes its delegates' sandboxes with it, except one that still holds work the lead never got
//! (a patch of it kept here, or changes there not brought back): that one stays, for the user to delete in Settings.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::{Command, Declarative, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::events::SessionDeleted;
use wisp_sandbox::SandboxPlugin;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

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
cmd!(Route, "machines.route");
cmd!(Remove, "grid-client.sandboxRemove");

/// A delegate's sandbox machine of lead `t1`, with these fields in its data.
fn machine(id: &str, sandbox: &str, data: &Value) -> Value {
    let mut v = json!({ "id": id, "peer": null, "name": "Sandbox", "transport": "grid", "address": "grid.test", "port": 443, "command": "",
        "autoConnect": true, "addedAt": 0, "kind": "grid", "target": "grid.test", "defaultCommand": "", "state": "online", "steps": [],
        "error": "", "hint": null, "stale": false, "attempt": 0, "since": null, "remote": null, "outdated": null, "log": [],
        "projects": 0, "working": 0, "sandbox": sandbox, "owner": { "lead": "t1", "member": "far" } });
    for (k, x) in data.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// wisp.machines with four delegate sandboxes: one with changes not brought back (`m1`), one whose patch is kept here
/// (`m2`), one with nothing left (`m3`), one placed before its folder was noted (`m4`). `sandbox.patch` answers there.
struct Machines;
impl Plugin for Machines {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.machines", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<List, _, _>(|_, _, _| async move {
            Ok(json!([
                machine("m1", "sb1", &json!({ "cwd": "/home/u/code/app" })),
                machine("m2", "sb2", &json!({ "cwd": "/home/u/code/app", "kept": true })),
                machine("m3", "sb3", &json!({ "cwd": "/home/u/code/app" })),
                machine("m4", "sb4", &json!({})),
            ]))
        });
        s.command::<Route, _, _>(|_, _, a: Value| async move {
            assert_eq!(a["cmd"], "sandbox.patch", "{a}");
            let patch = if a["id"] == "m1" { "diff --git a/x b/x\n" } else { "" };
            Ok(json!({ "patch": patch }))
        });
        Ok(())
    }
}

/// wisp.grid-client: the sandboxes deleted on the grid.
struct Grid(Arc<Mutex<Vec<String>>>);
impl Plugin for Grid {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.grid-client", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let gone = self.0.clone();
        s.command::<Remove, _, _>(move |_, _, a: Value| {
            gone.lock().unwrap().push(a["id"].as_str().unwrap_or_default().to_owned());
            async move { Ok(json!({ "ok": true })) }
        });
        Ok(())
    }
}

/// The lead is deleted: the sandboxes with nothing left go (`sb3`, and `sb4` as before); the one with changes there and
/// the one with a patch kept stay. (Before, every one went, and the delegate's unbrought work with it.)
#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_lead_s_delegate_sandbox_with_work_left_stays() {
    let gone = Arc::new(Mutex::new(Vec::new()));
    let runtime = Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap();
    let k = TestApp::new()
        .service(Store::open(Location::Memory, Arc::new(SystemClock)).unwrap())
        .with(SandboxPlugin { inside: Some(false), home: None })
        .with(Machines)
        .with(Grid(gone.clone()))
        .with(Declarative(runtime))
        .start()
        .await;
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &SessionDeleted { id: SessionId::from("t1") });
    for _ in 0..200 {
        if gone.lock().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut dropped = gone.lock().unwrap().clone();
    dropped.sort();
    assert_eq!(dropped, ["sb3", "sb4"]);
}
