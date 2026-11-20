//! `runtime.projectAdd` (ADR-0076, ADR-0077): a folder becomes a project under a name the server picks, inside the
//! store's transaction, so every client gets one rule and two adds at once never take one name.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::{Caller, Code, Kernel};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

/// wisp.runtime over a store of its own (kept, to read the rows back).
struct K {
    k: Kernel,
    store: Store,
}

async fn kernel() -> K {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new()
        .clock(&wisp_testkit::FakeClock::at(1_700_000_000_000))
        .service(store.clone())
        .service(wisp_proc::local())
        .with(wisp_runtime::RuntimePlugin)
        .start()
        .await;
    K { k, store }
}

/// `path` made on disk first: a project is a folder that's there (audit A F1)
fn made(path: &str) -> &str {
    std::fs::create_dir_all(path).unwrap();
    path
}

async fn add(k: &K, args: Value) -> Value {
    if let Some(p) = args["path"].as_str().filter(|p| p.starts_with('/')) {
        made(p.trim_end_matches("/."));
    }
    k.k.call(Caller::user(), "runtime.projectAdd", args).await.unwrap_or_else(|e| panic!("runtime.projectAdd: {}", e.message))
}

/// The stored projects, `(name, path)`, by name.
async fn projects(k: &K) -> Vec<(String, String)> {
    let rows = k.store.read_async(|r| r.query("project", &wisp_store::Query::new())).await.unwrap();
    let mut out: Vec<(String, String)> =
        rows.iter().map(|r| (r["id"].as_str().unwrap().to_owned(), r["body"]["path"].as_str().unwrap_or_default().to_owned())).collect();
    out.sort();
    out
}

/// Two clients adding `/a/wisp` and `/b/wisp` at the same moment each chose `wisp` (ADR-0076's open edge) and the
/// later apply replaced the first: the server picks inside one transaction, so they are `wisp` and `wisp-2`.
#[tokio::test(flavor = "multi_thread")]
async fn two_concurrent_adds_of_same_named_folders_give_wisp_and_wisp_2() {
    let k = kernel().await;
    let (x, y) = tokio::join!(add(&k, json!({ "path": "/tmp/a/wisp" })), add(&k, json!({ "path": "/tmp/b/wisp" })));
    let mut names = vec![x["name"].as_str().unwrap().to_owned(), y["name"].as_str().unwrap().to_owned()];
    names.sort();
    assert_eq!(names, ["wisp", "wisp-2"]);
    assert_eq!((x["created"].clone(), y["created"].clone()), (json!(true), json!(true)));
    let got = projects(&k).await;
    assert_eq!(got.len(), 2, "{got:?}");
    let paths: Vec<&str> = got.iter().map(|(_, p)| p.as_str()).collect();
    assert!(paths.contains(&"/tmp/a/wisp") && paths.contains(&"/tmp/b/wisp"), "{got:?}");
}

/// The same folder twice, in any spelling (`/.`, a trailing `/`), is one project: the second add writes nothing
/// and answers the name it has, `created: false`.
#[tokio::test(flavor = "multi_thread")]
async fn the_same_folder_twice_gives_one_project() {
    let k = kernel().await;
    let first = add(&k, json!({ "path": "/tmp/same/web" })).await;
    assert_eq!(first, json!({ "name": "web", "path": "/tmp/same/web", "created": true }));
    for again in ["/tmp/same/web", "/tmp/same/web/.", "/tmp/same/web/"] {
        let a = add(&k, json!({ "path": again, "name": "other" })).await;
        assert_eq!((a["name"].as_str(), a["created"].as_bool()), (Some("web"), Some(false)), "{again}");
    }
    assert_eq!(projects(&k).await, vec![("web".to_owned(), "/tmp/same/web".to_owned())]);
}

/// A caller's name (Projects' Add with a name, an agent) that another folder has is `-2`, never an overwrite.
#[tokio::test(flavor = "multi_thread")]
async fn a_caller_given_name_thats_taken_gets_2() {
    let k = kernel().await;
    add(&k, json!({ "path": "/tmp/n/api" })).await;
    let a = add(&k, json!({ "path": "/tmp/n/server", "name": "api" })).await;
    assert_eq!((a["name"].as_str(), a["created"].as_bool()), (Some("api-2"), Some(true)));
    let b = add(&k, json!({ "path": "/tmp/n/free", "name": " docs " })).await;
    assert_eq!(b["name"], "docs", "trimmed");
    assert_eq!(projects(&k).await.len(), 3);
    assert_eq!(projects(&k).await[0], ("api".to_owned(), "/tmp/n/api".to_owned()), "the first one kept its folder");
}

/// A relative path or a URL isn't a folder to add (a relative one would be the server's own folder); a caller
/// whose set doesn't name `runtime:projectAdd` is refused (default deny).
#[tokio::test(flavor = "multi_thread")]
async fn a_relative_path_is_refused_and_so_is_a_caller_without_the_permission() {
    let k = kernel().await;
    for bad in ["wisp", "", "https://github.com/x/y"] {
        let e = k.k.call(Caller::user(), "runtime.projectAdd", json!({ "path": bad })).await.unwrap_err();
        assert_eq!(e.code, Code::Invalid, "{bad}: {}", e.message);
    }
    let e = k.k.call(Caller::set("pane:nobody"), "runtime.projectAdd", json!({ "path": "/tmp/x" })).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden, "{}", e.message);
    assert_eq!(projects(&k).await, Vec::<(String, String)>::new());
}

/// A task a channel started below Full auto (`agent:capped`) doesn't change the user's projects, and is told so in
/// the capped wording; no task's agent does (ADR-0112: the user's setup is the user's), only the user.
#[tokio::test(flavor = "multi_thread")]
async fn no_tasks_agent_adds_a_project_and_a_capped_one_hears_the_capped_words() {
    let k = kernel().await;
    let e = k.k.call(Caller::agent("t1", true), "runtime.projectAdd", json!({ "path": made("/tmp/cap/web") })).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden, "{}", e.message);
    assert!(e.message.contains("add a folder to the user’s projects"), "{}", e.message);
    assert_eq!(projects(&k).await, Vec::<(String, String)>::new());
    let e = k.k.call(Caller::agent("t1", false), "runtime.projectAdd", json!({ "path": "/tmp/cap/web" })).await.unwrap_err();
    assert!(
        e.code == Code::Forbidden
            && e.message.contains(
                "it can't add a project itself: ask with runtime_projectAdd or runtime.setup.ask, and the user allows it on a card"
            ),
        "{}",
        e.message
    );
    let ok = k.k.call(Caller::user(), "runtime.projectAdd", json!({ "path": "/tmp/cap/web" })).await.unwrap();
    assert_eq!(ok["name"], "web");
}

/// A name that can't be a resource name (a path's separators, a newline) is refused, never stored as an id.
#[tokio::test(flavor = "multi_thread")]
async fn a_name_with_separators_or_control_characters_is_refused() {
    let k = kernel().await;
    for bad in ["a/b", "a\\b", "x:y", "two\nlines"] {
        let e = k.k.call(Caller::user(), "runtime.projectAdd", json!({ "path": made("/tmp/nm/web"), "name": bad })).await.unwrap_err();
        assert_eq!(e.code, Code::Invalid, "{bad:?}: {}", e.message);
    }
    assert_eq!(add(&k, json!({ "path": "/tmp/nm/web", "name": "my.app" })).await["name"], "my.app");
}

/// Audit A F1: a folder that isn't there, or a file, was added as a project from the UI ("nope-not-here", "math.js");
/// the CLI refused both. The server refuses them for every client, in plain words. F3: no command ids in what it says
/// (it read "(clone a repo URL first: git.clone)").
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_folder_or_a_file_is_refused_in_plain_words() {
    let k = kernel().await;
    let missing = "/tmp/wisp-audit-f1/nope-not-here";
    let _ = std::fs::remove_dir_all("/tmp/wisp-audit-f1");
    made("/tmp/wisp-audit-f1");
    let file = "/tmp/wisp-audit-f1/math.js";
    std::fs::write(file, "1").unwrap();
    for (bad, says) in
        [(missing, "No folder at /tmp/wisp-audit-f1/nope-not-here"), (file, "/tmp/wisp-audit-f1/math.js is a file, not a folder")]
    {
        let e = k.k.call(Caller::user(), "runtime.projectAdd", json!({ "path": bad })).await.unwrap_err();
        assert_eq!(e.code, Code::Invalid, "{bad}: {}", e.message);
        assert!(e.message.contains(says), "{bad}: {}", e.message);
    }
    for bad in ["repo2", "https://github.com/x/y"] {
        let e = k.k.call(Caller::user(), "runtime.projectAdd", json!({ "path": bad })).await.unwrap_err();
        assert!(!e.message.contains("git.clone") && !e.message.starts_with("path:"), "{bad}: {}", e.message);
        assert!(e.message.contains("full path"), "{bad}: {}", e.message);
    }
    assert_eq!(projects(&k).await, Vec::<(String, String)>::new());
    // a file:// URL of a folder is that folder
    made("/tmp/wisp-audit-f1/repo2");
    assert_eq!(add(&k, json!({ "path": "file:///tmp/wisp-audit-f1/repo2" })).await["path"], "/tmp/wisp-audit-f1/repo2");
    // spaces as a file URL writes them, and localhost as its host
    made("/tmp/wisp-audit-f1/My App");
    assert_eq!(add(&k, json!({ "path": "file://localhost/tmp/wisp-audit-f1/My%20App" })).await["path"], "/tmp/wisp-audit-f1/My App");
}
