//! The install's `Port` when none is provided as a service: wisp.runtime's tasks, through its api
//! commands (`runtime.list`, `runtime.get`, `runtime.setExt`), and the plugin's settings from the
//! kernel's settings store. A task's worktree is its `ext["wisp.worktrees"]` (v1 `session.worktree`);
//! `kept` rides in the same slot. What the cleanup remembers between sweeps (when the cleanup clock
//! started, the last record, project trust) is kept in memory: it starts again on a restart.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use wisp_kernel::{BoxFuture, Ctx};
use wisp_runtime_api::commands::Adopt;
use wisp_runtime_api::commands::{
    AdoptSession, AdoptSessionArgs, Get, IdArgs, List, ListArgs, Sessions, SessionsArgs, SetExt, SetExtArgs, Slots, SlotsArgs,
};
use wisp_runtime_api::ext::{merged, update_ext};
use wisp_runtime_api::model::{SessionState, Summary};
use wisp_runtime_api::paths::same_folder;
use wisp_runtime_api::spec::AdoptArgs;
use wisp_worktrees_api::{EXT, OfferItem, PrRef, Worktree};

use crate::ports::{Port, TaskInfo, TaskTree, WtSettings};

pub struct RuntimePort {
    ctx: Ctx,
    state: Mutex<WtSettings>,
}

impl std::fmt::Debug for RuntimePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimePort").finish_non_exhaustive()
    }
}

impl RuntimePort {
    pub fn new(ctx: Ctx, now: i64) -> Arc<Self> {
        Arc::new(Self { ctx, state: Mutex::new(WtSettings { cleanup_from: now, ..WtSettings::default() }) })
    }
}

fn text(v: Option<Value>) -> Option<String> {
    v.and_then(|v| match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// The last folder name of a path (the project a task is in).
pub(crate) fn project_of(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default().to_owned()
}

/// The task's PR as wisp.ship keeps it (`ext["wisp.ship"].pr`, a `PrSummary`: number, url, state, …), so the
/// sweep and Done see a merged or closed one. None: no PR, or one that doesn't read.
pub(crate) fn pr_of(ship: Option<&Value>) -> Option<PrRef> {
    let pr = ship?.get("pr").filter(|p| p.is_object())?;
    let r: PrRef = serde_json::from_value(pr.clone()).ok()?;
    (r.number > 0 && !r.state.is_empty()).then_some(r)
}

/// Each task's kids: the sessions whose parent is it (subagents, delegated jobs), in list order.
pub(crate) fn fill_kids(tasks: &mut [TaskInfo]) {
    let mut kids: HashMap<String, Vec<String>> = HashMap::new();
    for t in tasks.iter() {
        if let Some(p) = &t.parent_id {
            kids.entry(p.clone()).or_default().push(t.id.clone());
        }
    }
    for t in tasks.iter_mut() {
        t.kids = kids.remove(&t.id).unwrap_or_default();
    }
}

/// A list row and the task's slots (`runtime.slots`: worktrees', ship's, runtime's facts) as what the clean-up reads.
pub(crate) fn task_info(s: &Summary, ext: &BTreeMap<String, Value>, now: i64) -> TaskInfo {
    let slot = ext.get(EXT).cloned().unwrap_or(Value::Null);
    let pr = pr_of(ext.get("wisp.ship"));
    let trees = serde_json::from_value::<Worktree>(slot.clone())
        .ok()
        .filter(|w| !w.path.is_empty())
        .map(|worktree| TaskTree { worktree, pr })
        .into_iter()
        .collect();
    let runtime = ext.get("wisp.runtime");
    TaskInfo {
        id: s.id.0.clone(),
        title: s.title.clone(),
        project: project_of(&s.cwd),
        done: s.state == SessionState::Done,
        busy: s.state == SessionState::Running || s.queued > 0 || crate::archived::side_pending(runtime, now),
        kept: slot.get("kept").and_then(Value::as_bool).unwrap_or(false),
        parked: slot.get("parked").and_then(Value::as_bool).unwrap_or(false),
        updated_at: s.updated_at,
        created_at: s.created_at,
        trees,
        kids: Vec::new(),
        parent_id: s.parent_id.as_ref().map(|p| p.0.clone()),
        // the runs of one compare (runtime's slot, wisp --compare): cleaning one up offers its done siblings
        group_id: runtime.and_then(|o| o.get("group")).and_then(Value::as_str).filter(|g| !g.is_empty()).map(str::to_owned),
    }
}

impl Port for RuntimePort {
    fn tasks(&self) -> BoxFuture<'static, Vec<TaskInfo>> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let call = ctx.root_call();
            let now = ctx.clock().map_or(0, |c| c.now_ms());
            // every task, archived ones too (`Some(true)` would list only the archived ones)
            let Ok(list) = ctx.run::<List>(&call, ListArgs { archived: None, limit: None }).await else {
                return Vec::new();
            };
            // the slots it reads, every task's in one store read (perf audit 2026-11-18 #3: was `runtime.get` per task)
            let plugins = vec![EXT.to_owned(), "wisp.ship".to_owned(), "wisp.runtime".to_owned()];
            // only the tasks with a worktree slot or in a compare group (perf audit 2026-11-19 #7: all 5,000 bodies were
            // read): a task with neither has nothing to clean and joins nobody's Done but through its parent (the list);
            // its side questions only count for a task with folders
            let having = vec![EXT.to_owned(), "group".to_owned()];
            let Ok(slots) = ctx.run::<Slots>(&call, SlotsArgs { plugins, all: false, having }).await else {
                return Vec::new();
            };
            let mut ext: HashMap<String, BTreeMap<String, Value>> = slots.into_iter().map(|t| (t.id.0, t.ext)).collect();
            let mut out: Vec<TaskInfo> = list
                .into_iter()
                .map(|s| {
                    let slots = ext.remove(s.id.as_str()).unwrap_or_default();
                    task_info(&s, &slots, now)
                })
                .collect();
            fill_kids(&mut out);
            out
        })
    }

    fn set_kept(&self, id: String, kept: bool) -> BoxFuture<'static, ()> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let call = ctx.root_call();
            // a compare-and-set merge: the slot's other keys (parked, the worktree itself) written meanwhile stay
            let keep = |slot: &Value, _: &_| Ok(Some(merged(slot, &serde_json::json!({ "kept": kept }))));
            if let Err(e) = update_ext(&ctx, &call, &id, EXT, keep).await {
                tracing::warn!(error = %e.message, "worktrees: keeping a worktree");
            }
        })
    }

    fn settings(&self) -> BoxFuture<'static, WtSettings> {
        let mut s = self.state.lock().map(|s| s.clone()).unwrap_or_default();
        s.cleanup = text(self.ctx.setting("worktrees.cleanup"));
        s.max = text(self.ctx.setting("worktrees.max"));
        s.on_done = text(self.ctx.setting("worktrees.onDone"));
        s.project_ask = text(self.ctx.setting("worktrees.projectAsk"));
        Box::pin(async move { s })
    }

    fn save_settings(&self, s: WtSettings) -> BoxFuture<'static, ()> {
        if let Ok(mut cur) = self.state.lock() {
            (cur.cleanup_from, cur.last, cur.trust, cur.dismissed) = (s.cleanup_from, s.last, s.trust, s.dismissed);
        }
        Box::pin(async {})
    }

    fn projects(&self) -> BoxFuture<'static, Vec<(String, String)>> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let call = ctx.root_call();
            let Ok(list) = ctx
                .run::<wisp_resources_api::Export>(
                    &call,
                    wisp_resources_api::ExportArgs { kinds: vec!["project".into()], include: Vec::new() },
                )
                .await
            else {
                return Vec::new();
            };
            list.into_iter()
                .filter(|r| r.kind == "project")
                .filter_map(|r| {
                    r.spec
                        .get("path")
                        .and_then(Value::as_str)
                        .filter(|p| p.starts_with('/') || p.contains(":\\"))
                        .map(|p| (r.name.clone(), p.to_owned()))
                })
                .collect()
        })
    }

    fn extra_in_use(&self, path: String) -> BoxFuture<'static, bool> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let call = ctx.root_call();
            let Ok(list) = ctx.run::<List>(&call, ListArgs { archived: None, limit: None }).await else {
                // can't tell: it stays (the next change, or the sweep, looks again)
                return true;
            };
            // only the tasks working now (a handful) are read whole, for their `dirs`
            for s in list.iter().filter(|s| matches!(s.state, SessionState::Running | SessionState::Waiting) || s.queued > 0) {
                let Ok(full) = ctx.run::<Get>(&call, IdArgs { id: s.id.clone() }).await else { continue };
                // as the disk sees them: an extra folder reached through a symlink still keeps the worktree
                for d in &full.dirs {
                    if crate::fsx::within_real(d, &path).await {
                        return true;
                    }
                }
            }
            false
        })
    }

    fn chats_in(&self, paths: Vec<String>) -> BoxFuture<'static, std::collections::HashSet<String>> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let mut out = std::collections::HashSet::new();
            for h in ctx.contributions(&wisp_runtime_api::harness::sessions::SESSIONS) {
                let left: Vec<&String> = paths.iter().filter(|p| !out.contains(*p)).collect();
                if left.is_empty() {
                    break;
                }
                // one pass over its store when it can; else its list per folder still to answer
                let hits: Vec<String> = if let Some(dirs) = h.folders().await {
                    left.into_iter().filter(|p| dirs.iter().any(|d| same_folder(d, p))).cloned().collect()
                } else {
                    let mut hit = Vec::new();
                    for p in left {
                        if !h.list(p).await.is_empty() {
                            hit.push(p.clone());
                        }
                    }
                    hit
                };
                out.extend(hits);
            }
            out
        })
    }

    fn bring_in(&self, item: OfferItem, root: String) -> BoxFuture<'static, Option<String>> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let call = ctx.root_call();
            // a chat that ran there: that session, carried on in its CLI; else an empty task in the folder
            let chat = ctx.run::<Sessions>(&call, SessionsArgs { cwd: item.path.clone() }).await.ok().and_then(|v| v.into_iter().next());
            let made = match chat {
                Some(c) => ctx.run::<AdoptSession>(&call, AdoptSessionArgs { harness: c.harness, id: c.id }).await,
                None => {
                    ctx.run::<Adopt>(
                        &call,
                        AdoptArgs { title: item.branch.clone(), cwd: item.path.clone(), fresh: true, ..AdoptArgs::default() },
                    )
                    .await
                }
            };
            let id = match made {
                Ok(c) => c.id,
                Err(e) => {
                    tracing::warn!(error = %e.message, path = %item.path, "worktrees: bringing in a worktree");
                    return None;
                }
            };
            let wt = Worktree { path: item.path.clone(), cwd: item.path, branch: item.branch, root, foreign: true, ..Worktree::default() };
            let value = serde_json::to_value(&wt).unwrap_or(Value::Null);
            // set-ext: whole the task was adopted just now for this worktree: its slot is the worktree as found
            if let Err(e) = ctx.run::<SetExt>(&call, SetExtArgs { id: id.clone(), plugin: EXT.into(), value, expect: None }).await {
                tracing::warn!(error = %e.message, "worktrees: marking a brought-in worktree");
            }
            Some(id.0)
        })
    }
}

/// Perf audit 2026-11-18 #3: the pane's tasks come from one list read and one slots read, never a `runtime.get` per
/// task (5,000 store reads answered `worktrees.view` in 0.2–0.5 s).
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod reads_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;
    use wisp_kernel::{App, Declarative, Manifest, Plugin, PluginGroup, Result, Setup};
    use wisp_runtime_api::commands::{Get, List, Slots, TaskSlots};
    use wisp_runtime_api::model::{Session, SessionState, Summary};

    use crate::ports::Port;

    struct Runtime(Arc<AtomicUsize>);
    impl Plugin for Runtime {
        fn manifest(&self) -> Manifest {
            Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
        }
        fn setup(&self, s: &mut Setup) -> Result<()> {
            s.command::<List, _, _>(|_, _, _| async {
                let row = |id: &str, state, parent: Option<&str>| Summary {
                    id: id.into(),
                    title: format!("task {id}"),
                    cwd: "/code/app".into(),
                    state,
                    parent_id: parent.map(Into::into),
                    ..Summary::default()
                };
                Ok(vec![
                    row("a", SessionState::Done, None),
                    row("b", SessionState::Review, Some("a")),
                    row("c", SessionState::Review, None),
                ])
            });
            s.command::<Slots, _, _>(|_, _, a| async move {
                assert!(a.plugins.iter().any(|p| p == "wisp.worktrees"));
                assert_eq!(a.having, ["wisp.worktrees", "group"], "only the bodies with a worktree or a compare group are read");
                let tree =
                    json!({ "path": "/w/a", "cwd": "/w/a", "branch": "wisp/a", "base": "main", "baseRef": "origin/main", "root": "/code/app", "createdAt": 1, "kept": true });
                Ok(vec![
                    TaskSlots {
                        id: "a".into(),
                        ext: [
                            ("wisp.worktrees".to_owned(), tree),
                            ("wisp.ship".to_owned(), json!({ "pr": { "number": 4, "url": "u", "state": "MERGED" } })),
                        ]
                        .into(),
                    },
                    TaskSlots {
                        id: "c".into(),
                        ext: [("wisp.runtime".to_owned(), json!({ "group": "g", "btw": [{ "pending": true, "at": 1_000 }] }))].into(),
                    },
                ])
            });
            let gets = self.0.clone();
            s.command::<Get, _, _>(move |_, _, _| {
                gets.fetch_add(1, Ordering::SeqCst);
                async { Ok(Session::default()) }
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn the_pane_reads_every_task_in_two_reads_and_no_get() {
        let gets = Arc::new(AtomicUsize::new(0));
        let caller = Declarative(
            Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.worktrees", "version": "1.0.0-alpha.1", "api": "^1", "permissions": ["runtime:list", "runtime:slots", "runtime:get"] }))
                .unwrap(),
        );
        let k = App::new().plugins(PluginGroup::new().add(Runtime(gets.clone())).add(caller)).start().await.unwrap();
        let port = super::RuntimePort::new(k.ctx("wisp.worktrees").unwrap(), 2_000);
        let tasks = port.tasks().await;
        assert_eq!(gets.load(Ordering::SeqCst), 0, "no runtime.get per task");
        let a = tasks.iter().find(|t| t.id == "a").unwrap();
        assert!(a.kept && a.done && !a.busy);
        assert_eq!(a.trees[0].worktree.path, "/w/a");
        assert_eq!(a.trees[0].pr.as_ref().map(|p| p.number), Some(4));
        assert_eq!(a.kids, ["b"]);
        let c = tasks.iter().find(|t| t.id == "c").unwrap();
        assert!(c.busy, "a side question asked in the last minutes keeps it busy");
        assert_eq!(c.group_id.as_deref(), Some("g"));
        assert_eq!(tasks.iter().find(|t| t.id == "b").unwrap().trees.len(), 0);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::{TaskInfo, fill_kids, pr_of, project_of};
    use serde_json::json;

    #[test]
    fn the_project_is_the_last_folder() {
        assert_eq!(project_of("/Users/me/code/wisp/"), "wisp");
        assert_eq!(project_of("C:\\code\\app"), "app");
    }

    /// wisp.ship's slot holds a `PrSummary`: its number, url and state are the task's PR (a merged or closed one
    /// lets the sweep take the folder); no slot, a null PR or one that doesn't read is no PR.
    #[test]
    fn the_tasks_pr_is_read_from_ships_slot() {
        let ship =
            json!({ "pr": { "number": 12, "url": "https://x/pr/12", "title": "Fix", "state": "MERGED", "draft": false, "checks": {} } });
        let pr = pr_of(Some(&ship)).unwrap();
        assert_eq!((pr.number, pr.url.as_str(), pr.state.as_str()), (12, "https://x/pr/12", "MERGED"));
        assert!(pr_of(None).is_none());
        assert!(pr_of(Some(&json!({ "pr": null }))).is_none());
        assert!(pr_of(Some(&json!({ "pr": { "number": "x" } }))).is_none());
    }

    /// Done's list walks a task's kids: every session whose parent is it.
    #[test]
    fn a_tasks_kids_are_the_sessions_it_is_parent_of() {
        let t = |id: &str, parent: Option<&str>| TaskInfo { id: id.into(), parent_id: parent.map(str::to_owned), ..TaskInfo::default() };
        let mut all = vec![t("lead", None), t("sub", Some("lead")), t("job", Some("lead")), t("deep", Some("sub")), t("other", None)];
        fill_kids(&mut all);
        assert_eq!(all[0].kids, ["sub", "job"]);
        assert_eq!(all[1].kids, ["deep"]);
        assert_eq!(all[4].kids, Vec::<String>::new());
    }

    /// Done, the sweep and the pane read every task: a live one with a worktree too (the list once asked
    /// for archived ones only, so Done had no folders to ask about).
    #[tokio::test]
    async fn the_tasks_are_every_task_live_and_archived() {
        use wisp_kernel::{Manifest, Plugin, Result, Setup};
        use wisp_runtime_api::model::Summary;
        struct Rt;
        impl Plugin for Rt {
            fn manifest(&self) -> Manifest {
                Manifest::from_value(
                    json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
                )
                .unwrap()
            }
            fn setup(&self, s: &mut Setup) -> Result<()> {
                // runtime.list's own rule: `archived: Some(x)` keeps only rows whose archived is x
                s.command::<super::List, _, _>(|_c, _call, a| async move {
                    let row = |id: &str, archived: bool| Summary { id: id.into(), archived, ..serde_json::from_value(json!({ "id": id, "title": "", "agent": "claude", "model": "", "effort": "", "mode": "auto", "cwd": "/w", "state": "review", "status": "done", "unread": false, "archived": archived, "createdAt": 0, "updatedAt": 0, "turnCount": 1, "queued": 0, "preview": "", "live": "" })).unwrap() };
                    Ok([row("live", false), row("old", true)].into_iter().filter(|r| a.archived.is_none_or(|x| r.archived == x)).collect())
                });
                // every task's worktree slot in one read (`runtime.slots`), as the real one answers it
                s.command::<super::Slots, _, _>(|_c, _call, _a| async move {
                    let slot = |id: &str| wisp_runtime_api::commands::TaskSlots {
                        id: id.into(),
                        ext: [("wisp.worktrees".to_owned(), json!({ "path": format!("/w/{id}"), "cwd": "/w", "branch": "b", "base": "main", "baseRef": "origin/main", "root": "/w", "createdAt": 0 }))].into(),
                    };
                    Ok(vec![slot("live"), slot("old")])
                });
                Ok(())
            }
        }
        struct Me;
        impl Plugin for Me {
            fn manifest(&self) -> Manifest {
                Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.worktrees", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"], "permissions": ["runtime:list", "runtime:slots"] })).unwrap()
            }
            fn setup(&self, _s: &mut Setup) -> Result<()> {
                Ok(())
            }
        }
        let k = wisp_testkit::TestApp::new().with(Rt).with(Me).start().await;
        let port = super::RuntimePort::new(k.ctx("wisp.worktrees").unwrap(), 0);
        let ids: Vec<String> = crate::ports::Port::tasks(port.as_ref()).await.into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["live", "old"]);
    }

    /// Keep (a click) while the slot is written meanwhile (parked by the sweep): both land, where the plain
    /// read-then-write put the stale copy back and lost the park.
    #[tokio::test]
    async fn keeping_a_worktree_keeps_a_write_that_landed_meanwhile() {
        use wisp_runtime_api::model::Session;
        use wisp_runtime_api::testing::{SlotRuntime, caller};
        let rt = SlotRuntime::new(Session { id: "t1".into(), ..Session::default() });
        rt.set("wisp.worktrees", json!({ "path": "/w/t1" }));
        let k = wisp_testkit::TestApp::new().with(rt.clone()).with(caller("wisp.worktrees")).start().await;
        let port = super::RuntimePort::new(k.ctx("wisp.worktrees").unwrap(), 0);
        rt.race("wisp.worktrees", json!({ "path": "/w/t1", "parked": true }));
        crate::ports::Port::set_kept(port.as_ref(), "t1".into(), true).await;
        assert_eq!(rt.ext("wisp.worktrees"), json!({ "path": "/w/t1", "parked": true, "kept": true }));
    }
}
