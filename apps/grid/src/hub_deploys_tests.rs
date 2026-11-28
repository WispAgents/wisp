//! One deploy at a time, and the history (ADR-0148 §5).
#![allow(clippy::unwrap_used)]
use std::sync::Arc;

use super::*;
use crate::accounts::Accounts;
use crate::clock::ManualClock;
use crate::db::Db;

fn applied(ops: &[&str]) -> Value {
    json!({ "ok": true, "result": { "plan": { "ops": ops.iter().map(|o| json!({ "op": o })).collect::<Vec<_>>() }, "applied": ops.len() } })
}

#[test]
fn one_deploy_at_a_time_and_a_stale_one_gives_the_hub_back() {
    let db = Arc::new(Db::memory().unwrap());
    let clock = ManualClock::new(1_000);
    let a = Accounts { db: db.clone(), clock: clock.clone(), telemetry: None };
    let t = Teams { db, clock: clock.clone() };
    let ada = a.dev_user("ada").unwrap();
    t.create(&ada, "acme", "").unwrap();
    let tid = t.hub("acme").unwrap().unwrap().team_id;
    let ci = By { kind: "key", name: "ci-main".into() };
    let n = t.deploy_begin(&tid, &ci, "acme/agents@a1b2c3\n").unwrap();
    assert_eq!(n, 1);
    let e = t.deploy_begin(&tid, &By { kind: "person", name: "ada".into() }, "").unwrap_err();
    assert_eq!(e.status(), 409);
    assert!(e.to_string().contains("#1 by ci-main"), "{e}");
    t.deploy_end(&tid, n, Ok(counts(&applied(&["create", "create", "update"])))).unwrap();
    // nothing changed: not kept, and the hub is free
    let n2 = t.deploy_begin(&tid, &ci, "").unwrap();
    t.deploy_end(&tid, n2, Ok(counts(&applied(&[])))).unwrap();
    // a failure is kept with why
    let n3 = t.deploy_begin(&tid, &ci, "").unwrap();
    t.deploy_end(&tid, n3, Err("resources.apply refused".into())).unwrap();
    // one that never closes gives the hub back after STALE_MS
    let n4 = t.deploy_begin(&tid, &ci, "").unwrap();
    assert_eq!(t.deploy_begin(&tid, &ci, "").unwrap_err().status(), 409);
    clock.advance(STALE_MS + 1);
    let n5 = t.deploy_begin(&tid, &ci, "").unwrap();
    assert!(n5 > n4);
    let list = t.deploys(&tid, 10).unwrap();
    let states: Vec<&str> = list.iter().map(|d| d.state.as_str()).collect();
    assert_eq!(states, ["running", "abandoned", "failed", "done"]);
    let first = list.last().unwrap();
    assert_eq!(first.source, "acme/agents@a1b2c3", "one line, trimmed");
    assert_eq!(first.summary, json!({ "created": 2, "updated": 1, "deleted": 0 }));
}
