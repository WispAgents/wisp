//! ADR-0128: a run's browser follows `runtime.browser` (and a harness's own `runtime.browsers`): a harness that takes
//! MCP gets the pinned Playwright server as `browser` for headless or Chrome, none when it's off or unset, and a
//! connector the user named `browser` is never replaced. A harness without per-run MCP never gets one.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{Fake, Recording, rig_on, until};
use serde_json::{Value, json};
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::harness::HarnessRun;
use wisp_runtime_api::harness::browser::{Browser, PLAYWRIGHT_MCP};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{McpQuery, RunPart};

/// A connector the user named `browser` (their own MCP server).
struct Mine;

impl RunPart for Mine {
    fn id(&self) -> &'static str {
        "test.mine"
    }
    fn mcp_servers(&self, _s: &Session, _q: &McpQuery) -> BTreeMap<String, Value> {
        BTreeMap::from([("browser".to_owned(), json!({ "command": "my-browser-mcp" }))])
    }
}

async fn first_run(settings: &[(&str, Value)], parts: Vec<Arc<dyn RunPart>>, mcp: bool) -> HarnessRun {
    let h = if mcp { Fake::leads("fake", "Fake CLI") } else { Fake::harness() };
    let (h, runs) = Recording::of(h);
    let host = Recorder { parts, settings: settings.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect(), ..Recorder::default() };
    let r = rig_on(host, vec![h]).await;
    r.rt.create(StartArgs { prompt: "look at the page".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap();
    until("its run", || async { !runs.lock().unwrap().is_empty() }).await;
    runs.lock().unwrap()[0].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_browser_part_is_there_for_headless_and_chrome_and_absent_when_off_or_unset() {
    // the sandbox's env is the process's: these runs are outside one
    if std::env::var("WISP_SANDBOX").as_deref() == Ok("1") {
        return;
    }
    for (value, want) in [("headless", Browser::Headless), ("chrome", Browser::Chrome)] {
        let run = first_run(&[("runtime.browser", json!(value))], Vec::new(), true).await;
        assert_eq!(run.browser, want);
        let b = &run.mcp_servers["browser"];
        assert_eq!(b["command"], "npx", "{value}");
        assert_eq!(b["args"][1], PLAYWRIGHT_MCP, "pinned: {b}");
    }
    for settings in [vec![("runtime.browser", json!("off"))], vec![("runtime.browser", json!("default"))], vec![]] {
        let run = first_run(&settings, Vec::new(), true).await;
        assert!(!run.mcp_servers.contains_key("browser"), "{settings:?}");
        assert_eq!(run.browser, Browser::Off);
    }
    // one harness's own wins over every harness's
    let run = first_run(&[("runtime.browser", json!("off")), ("runtime.browsers", json!({ "fake": "headless" }))], Vec::new(), true).await;
    assert!(run.mcp_servers.contains_key("browser"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connector_named_browser_stays_and_a_harness_without_mcp_gets_none() {
    let run = first_run(&[("runtime.browser", json!("headless"))], vec![Arc::new(Mine)], true).await;
    assert_eq!(run.mcp_servers["browser"]["command"], "my-browser-mcp", "the user's own server wins");
    let run = first_run(&[("runtime.browser", json!("headless"))], Vec::new(), false).await;
    assert!(!run.mcp_servers.contains_key("browser"));
    assert_eq!(run.browser, Browser::Off);
}
