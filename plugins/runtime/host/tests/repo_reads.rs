//! The store reads perf audit 2026-11-19 #4 and #7 narrowed: `runtime.slots` with `having` reads only the bodies that
//! name a word, and a task adopted from an outside session is found without decoding every task. Same answers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::json;
use wisp_runtime::{repo::Repo, schema};
use wisp_runtime_api::commands::SlotsArgs;
use wisp_runtime_api::model::{Session, Summary};
use wisp_store::{Location, Store, SystemClock};

fn task(id: &str, at: i64, ext: serde_json::Value) -> Session {
    let mut s =
        Session { id: id.into(), title: format!("task {id}"), cwd: "/w".into(), created_at: at, updated_at: at, ..Session::default() };
    if let serde_json::Value::Object(m) = ext {
        s.ext = m.into_iter().collect();
    }
    s
}

async fn repo(tasks: Vec<Session>) -> Repo {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    schema::install(&store).unwrap();
    store
        .write(move |w| {
            for t in &tasks {
                wisp_runtime::repo::save_session(w, t, &Summary { id: t.id.clone(), ..Summary::default() }, None)
                    .map_err(|e| wisp_store::StoreError::Internal(e.to_string()))?;
            }
            Ok(())
        })
        .await
        .unwrap();
    Repo::new(store)
}

#[tokio::test]
async fn slots_having_reads_only_the_tasks_that_name_a_word() {
    let r = repo(vec![
        task("a", 3, json!({ "wisp.worktrees": { "path": "/w/a" }, "wisp.runtime": { "spec": {} } })),
        task("b", 2, json!({ "wisp.runtime": { "spec": {} } })),
        task("c", 1, json!({ "wisp.runtime": { "group": "g" } })),
        task("d", 4, json!({ "wisp.worktrees": { "path": "/w/d" }, "wisp.runtime": { "group": "g" } })),
    ])
    .await;
    let plugins = vec!["wisp.worktrees".to_owned(), "wisp.runtime".to_owned()];
    let every = r.slots(SlotsArgs { plugins: plugins.clone(), all: false, having: Vec::new() }).await.unwrap();
    assert_eq!(every.len(), 4, "no `having`: every task with a slot asked for");
    let a = SlotsArgs { plugins, all: false, having: vec!["wisp.worktrees".into(), "group".into()] };
    let mut got: Vec<String> = r.slots(a).await.unwrap().into_iter().map(|t| t.id.0).collect();
    got.sort();
    assert_eq!(got, ["a", "c", "d"], "b names neither; d names both and comes once");
}

#[tokio::test]
async fn a_task_adopted_from_a_session_is_found_by_it() {
    let mut adopted = task("x", 1, json!({ "wisp.adopted": { "session": "conv-123" } }));
    adopted.resume_id = None;
    let mut resumed = task("y", 2, json!({}));
    resumed.resume_id = Some("conv-456".into());
    let r = repo(vec![adopted, resumed, task("z", 3, json!({ "note": "conv-1234 is another" }))]).await;
    let ids = |v: Vec<Session>| v.into_iter().map(|s| s.id.0).collect::<Vec<_>>();
    assert_eq!(ids(r.sessions_naming("conv-123").await.unwrap()).len(), 2, "x, and z (whose body only names it): the caller decides");
    assert_eq!(ids(r.sessions_naming("conv-456").await.unwrap()), ["y"]);
    let all = r.sessions().await.unwrap();
    assert_eq!(ids(all.clone()), ["z", "y", "x"], "newest first");
    assert_eq!(all[2].ext["wisp.adopted"]["session"], "conv-123");
}
