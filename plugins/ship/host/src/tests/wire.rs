//! Ship it wired to runtime's sessions: which tasks it follows, the slot it keeps on them, and the row's
//! fields after a recheck (F-ship-2: nothing to ship, no Ship it; F-ship-7: more changes after a merge
//! bring it back). The UI half of both rows is `plugins/ship/ui/test/pr.test.ts`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_runtime_api::model::{Session, SessionState};
use wisp_ship_api::{PrSummary, Worktree};
use wisp_ship_core::slot;

use super::{H, h};
use crate::wire::{Known, follow_session};

fn session(id: &str, wt: Option<&Worktree>, state: SessionState) -> Session {
    let mut s = Session { id: id.into(), cwd: "/code/app".into(), agent: "claude".into(), state, ..Session::default() };
    if let Some(w) = wt {
        // the shape wisp.worktrees writes (its own Worktree, more fields than ship reads)
        s.ext.insert(
            "wisp.worktrees".into(),
            json!({ "path": w.path, "cwd": w.path, "branch": w.branch, "base": "main", "baseRef": "origin/main", "remote": "origin", "root": w.root, "createdAt": 1 }),
        );
    }
    s
}

fn row(h: &H, id: &str) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    slot::row_fields(&slot::ext_of(&h.task(id)), &mut out);
    out
}

#[tokio::test]
async fn a_worktree_task_is_followed_a_stale_running_read_never_marks_it_running_and_a_task_in_place_is_not() {
    let h = h();
    let known = Known::default();
    let w = h.wt("app");
    follow_session(&h.ship, &known, &session("t1", Some(&w), SessionState::Running));
    assert_eq!(h.task("t1").repos[0].worktree.path, w.path);
    assert!(h.ship.tasks.is_live("t1"), "first seen running");
    h.ship.tasks.set_live("t1", false); // turn.ended
    // a session.changed whose read was taken mid-turn lands after the end: the task stays idle
    follow_session(&h.ship, &known, &session("t1", Some(&w), SessionState::Running));
    assert!(!h.ship.tasks.is_live("t1"));
    follow_session(&h.ship, &known, &session("t2", None, SessionState::Review));
    assert!(h.ship.tasks.get("t2").is_none());
}

#[tokio::test]
async fn after_a_restart_the_task_comes_back_from_its_slot_with_its_pr() {
    let h = h();
    let w = h.wt("app");
    follow_session(&h.ship, &Known::default(), &session("t1", Some(&w), SessionState::Review));
    let mut t = h.task("t1");
    t.repos[0].pr = Some(PrSummary { number: 7, state: "OPEN".into(), head: "abc".into(), ..PrSummary::default() });
    let mut s = session("t1", Some(&w), SessionState::Review);
    s.ext.insert("wisp.ship".into(), slot::ext_of(&t));
    let fresh = super::h();
    follow_session(&fresh.ship, &Known::default(), &s);
    assert_eq!(fresh.task("t1").repos[0].pr.as_ref().map(|p| p.number), Some(7));
}

#[tokio::test]
async fn each_change_to_a_task_is_told_and_a_write_that_changes_nothing_is_not() {
    let h = h();
    let told: Arc<Mutex<Vec<String>>> = Arc::default();
    let t = told.clone();
    h.ship.tasks.on_change(Arc::new(move |id| t.lock().unwrap().push(id.to_owned())));
    let (known, w) = (Known::default(), h.wt("app"));
    let s = session("t1", Some(&w), SessionState::Review);
    follow_session(&h.ship, &known, &s);
    follow_session(&h.ship, &known, &s); // its own slot write comes back as session.changed: no loop
    assert_eq!(told.lock().unwrap().len(), 1);
    h.ship.tasks.update("t1", |t| t.pushed_at = 9);
    h.ship.tasks.update("t1", |t| t.pushed_at = 9);
    assert_eq!(told.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn nothing_to_ship_when_the_task_finishes_puts_zeros_on_the_row() {
    let h = h();
    let w = h.wt("app");
    follow_session(&h.ship, &Known::default(), &session("t1", Some(&w), SessionState::Review));
    h.ship.recheck(&h.call, "t1", false).await.unwrap();
    assert_eq!(row(&h, "t1")["toShip"], json!({ "dirty": 0, "ahead": 0, "unpushed": 0 }));
}

#[tokio::test]
async fn more_changes_after_a_merge_bring_ship_it_back_on_the_row() {
    let h = h();
    let w = h.wt("app");
    follow_session(&h.ship, &Known::default(), &session("t1", Some(&w), SessionState::Review));
    h.ship.tasks.update("t1", |t| t.repos[0].pr = Some(PrSummary { number: 4, state: "MERGED".into(), ..PrSummary::default() }));
    h.ship.recheck(&h.call, "t1", false).await.unwrap();
    assert_eq!(row(&h, "t1")["toShip"]["dirty"], 0);
    h.dirty(&w, 2); // the agent changed more after the merge
    h.ship.recheck(&h.call, "t1", false).await.unwrap();
    let r = row(&h, "t1");
    assert_eq!(r["toShip"]["dirty"], 2);
    assert_eq!(r["pr"]["state"], "MERGED");
}

// B56: a task followed only for its attached PR, the PR detached: the next session events keep it with none. Dropped,
// the one after restored it from the slot, which still held the PR (written before the detach), so it came back.
#[tokio::test]
async fn a_detached_pr_does_not_come_back_from_the_slot() {
    let h = h();
    let known = Known::default();
    let attached = wisp_ship_api::model::AttachedPr {
        repo: "acme/repo".into(),
        pr: PrSummary { number: 12, state: "OPEN".into(), url: "https://github.com/acme/repo/pull/12".into(), ..PrSummary::default() },
    };
    let with_pr = wisp_ship_api::model::ShipTask { session: "t1".into(), prs: vec![attached], ..wisp_ship_api::model::ShipTask::default() };
    // the slot as it was written while the PR was attached
    let mut s = session("t1", None, SessionState::Review);
    s.ext.insert("wisp.ship".into(), slot::ext_of(&with_pr));
    follow_session(&h.ship, &known, &s);
    assert_eq!(h.task("t1").prs.len(), 1, "followed for its PR");
    // detached (ship saves the task with no PR); the slot isn't rewritten yet
    h.ship.tasks.update("t1", |t| t.prs.clear());
    follow_session(&h.ship, &known, &s);
    follow_session(&h.ship, &known, &s);
    assert!(h.ship.tasks.get("t1").is_some_and(|t| t.prs.is_empty()), "the PR stays detached");
}

/// Perf audit 2026-11-18 #5: the boot follow reads every task in one list read and one slots read; it read
/// `runtime.get` for each of 5,000 tasks after every start. The tasks with a worktree or a ship slot are followed
/// as before; one with neither (or only a crew slot) is not.
#[tokio::test]
async fn boot_follows_from_one_list_and_one_slots_read_never_a_get_per_task() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wisp_kernel::{App, Declarative, Manifest, Plugin, PluginGroup, Result, Setup};
    use wisp_runtime_api::commands::{Get, List, Slots, TaskSlots};
    use wisp_runtime_api::model::Summary;

    struct Runtime(Arc<AtomicUsize>, serde_json::Value);
    impl Plugin for Runtime {
        fn manifest(&self) -> Manifest {
            Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "2.0.0", "api": "^1" })).unwrap_or_default()
        }
        fn setup(&self, s: &mut Setup) -> Result<()> {
            s.command::<List, _, _>(|_, _, _| async {
                let row = |id: &str| Summary { id: id.into(), cwd: "/code/app".into(), state: SessionState::Review, ..Summary::default() };
                Ok(vec![row("wt"), row("plain"), row("crew")])
            });
            let tree = self.1.clone();
            s.command::<Slots, _, _>(move |_, _, _| {
                let tree = tree.clone();
                async move {
                    Ok(vec![
                        TaskSlots { id: "wt".into(), ext: [("wisp.worktrees".to_owned(), tree)].into() },
                        TaskSlots { id: "crew".into(), ext: [("wisp.crew".to_owned(), json!({ "member": "ops" }))].into() },
                    ])
                }
            });
            let gets = self.0.clone();
            s.command::<Get, _, _>(move |_, _, _| {
                gets.fetch_add(1, Ordering::SeqCst);
                async { Ok(Session::default()) }
            });
            Ok(())
        }
    }
    let h = h();
    let w = h.wt("app");
    let tree = json!({ "path": w.path, "cwd": w.path, "branch": w.branch, "base": "main", "baseRef": "origin/main", "remote": "origin", "root": w.root, "createdAt": 1 });
    let gets = Arc::new(AtomicUsize::new(0));
    let me = Declarative(
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.ship", "version": "2.0.0", "api": "^1", "permissions": ["runtime:list", "runtime:slots", "runtime:get"] }))
            .unwrap_or_default(),
    );
    let k = App::new().plugins(PluginGroup::new().add(Runtime(gets.clone(), tree)).add(me)).start().await.unwrap_or_else(|e| panic!("{e}"));
    let ctx = k.ctx("wisp.ship").unwrap_or_else(|| panic!("ship's context"));
    crate::wire::boot(&ctx, &h.ship, &Known::default()).await;
    assert_eq!(gets.load(Ordering::SeqCst), 0, "no runtime.get per task");
    assert_eq!(h.task("wt").repos[0].worktree.path, w.path);
    assert!(h.ship.tasks.get("plain").is_none() && h.ship.tasks.get("crew").is_none());
}
