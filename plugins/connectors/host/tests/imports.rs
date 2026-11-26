//! The harnesses' own servers in Settings → Connectors, end to end in one process: "Not now" outlives a restart (it
//! lived in memory, so every restart offered the same servers again) and its servers can still be added; a connector a
//! harness already has by that name is left out of that harness's runs. The harnesses' configs are a test folder
//! (`ConfigHome`), never yours.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_connectors::ConfigHome;
use wisp_kernel::{BoxFuture, Caller, Kernel, Result};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{McpQuery, RUN};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
impl wisp_kernel::services::Secrets for Secrets {
    fn get(&self, key: &str) -> BoxFuture<'static, Result<Option<String>>> {
        let v = self.0.lock().unwrap().get(key).cloned();
        Box::pin(async move { Ok(v) })
    }
    fn set(&self, key: &str, value: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Box::pin(async { Ok(()) })
    }
    fn delete(&self, key: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().remove(key);
        Box::pin(async { Ok(()) })
    }
    fn keys(&self, prefix: &str) -> BoxFuture<'static, Result<Vec<String>>> {
        let ks = self.0.lock().unwrap().keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        Box::pin(async move { Ok(ks) })
    }
}

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A folder whose Claude Code config has `linear` (remote) and Codex's has `docs` (local).
fn home(name: &str) -> Dir {
    let dir = std::env::temp_dir().join(format!("wisp-connectors-imports-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("home/.codex")).unwrap();
    std::fs::write(
        dir.join("home/.claude.json"),
        json!({ "mcpServers": { "linear": { "url": "https://mcp.linear.app/mcp" } } }).to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("home/.codex/config.toml"), "[mcp_servers.docs]\ncommand = \"docs-mcp\"\n").unwrap();
    Dir(dir)
}

async fn kernel(dir: &Path) -> Kernel {
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let secrets: wisp_kernel::services::SecretsService = Arc::new(Secrets::default());
    TestApp::new()
        .service(store)
        .service(secrets)
        .service(ConfigHome(dir.join("home")))
        .with(wisp_connectors::ConnectorsPlugin)
        .start()
        .await
}

async fn list(k: &Kernel) -> Value {
    k.call(Caller::user(), "connectors.list", json!({})).await.unwrap()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_owned()).collect()
}

/// "Not now" on `linear`, then a restart: the new pane reads what it hid back, so the card offers only `docs`; the
/// review's Show hidden still lists it, and adding it from there unhides it.
#[tokio::test(flavor = "multi_thread")]
async fn not_now_outlives_a_restart_and_a_hidden_server_can_still_be_added() {
    let d = home("not-now");
    let k = kernel(&d.0).await;
    assert_eq!(names(&list(&k).await["found"]), ["linear", "docs"]);
    let v = k.call(Caller::user(), "connectors.import", json!({ "dismiss": ["linear"] })).await.unwrap();
    assert_eq!(names(&v["found"]), ["docs"]);
    drop(k);

    let k = kernel(&d.0).await;
    let v = list(&k).await;
    assert_eq!(names(&v["found"]), ["docs"], "what Not now hid stays hidden: {v}");
    assert_eq!(names(&v["hidden"]), ["linear"]);
    assert_eq!(v["hidden"][0]["hidden"], true);

    let v = k.call(Caller::user(), "connectors.import", json!({ "names": ["linear"] })).await.unwrap();
    assert_eq!(names(&v["connectors"]), ["linear"]);
    assert!(names(&v["hidden"]).is_empty(), "{v}");
    drop(k);
    let k = kernel(&d.0).await;
    let v = list(&k).await;
    assert!(names(&v["hidden"]).is_empty() && names(&v["found"]) == ["docs"], "added, it's not hidden any more: {v}");
}

/// A connector named as a harness's own server: that harness keeps its own (the row says which), the others get it.
#[tokio::test(flavor = "multi_thread")]
async fn a_harness_keeps_its_own_server_by_a_connectors_name() {
    let d = home("own");
    let k = kernel(&d.0).await;
    k.call(Caller::user(), "connectors.add", json!({ "name": "docs", "transport": "remote", "url": "https://docs.example/mcp" }))
        .await
        .unwrap();
    let v = list(&k).await;
    let row = v["connectors"].as_array().unwrap().iter().find(|c| c["id"] == "docs").unwrap();
    assert_eq!(row["own"], json!(["codex"]), "{row}");
    let servers = |harness: &str| {
        let q = McpQuery {
            mode: wisp_runtime_api::model::Mode::Edits,
            harness: harness.into(),
            team: Vec::new(),
            turn_id: String::new(),
            sub: false,
        };
        k.contributions(&RUN).iter().flat_map(|r| r.mcp_servers(&Session::default(), &q).into_keys()).collect::<Vec<_>>()
    };
    assert!(!servers("codex").contains(&"docs".to_owned()), "Codex keeps its own docs");
    assert!(servers("claude").contains(&"docs".to_owned()));
}
