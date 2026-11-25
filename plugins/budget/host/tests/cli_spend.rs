//! Harnesses with no quota windows (ADR-0124): `budget.view` reads `accounts.limits` for Cursor, opencode and Grok in
//! the background, and the next view carries what their CLI said: Cursor's tier (`plan`), and what opencode and Grok
//! total as spent (`spent`). The bodies below are what each harness host's `live_limits` returns (its own tests parse
//! them from captured CLI output).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_accounts_api::commands as acc;
use wisp_budget::BudgetPlugin;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const NOW: i64 = 1_800_000_000_000;

/// wisp.accounts as far as budget calls it: `accounts.limits` answers what each harness's `live_limits` gives.
struct Accounts(Arc<Mutex<Vec<String>>>);
impl Plugin for Accounts {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.accounts", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let asked = self.0.clone();
        s.command::<acc::Limits, _, _>(move |_, _, a: acc::AccountArgs| {
            asked.lock().unwrap().push(a.agent.clone());
            let v = match a.agent.as_str() {
                "cursor" => json!({ "windows": [], "plan": "Pro" }),
                "opencode" => json!({ "windows": [], "cost": 1.25, "costDays": 7,
                    "tokens": { "input": 30_300_000.0, "output": 1_700_000.0, "cacheRead": 253_500_000.0, "cacheWrite": 1_800_000.0 } }),
                "grok" => json!({ "windows": [], "cost": 0.118_592, "costDays": 7, "tokens": 136_102 }),
                _ => Value::Null,
            };
            async move { Ok(v) }
        });
        Ok(())
    }
}

async fn kernel(asked: &Arc<Mutex<Vec<String>>>) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new().clock(&FakeClock::at(NOW)).service(store).with(BudgetPlugin).with(Accounts(asked.clone())).start().await
}

/// `budget.view` until its first account has `key` (the read it starts runs in the background).
async fn view_with(k: &Kernel, agent: &str, key: &str) -> Value {
    for _ in 0..300 {
        let v = k.call(Caller::user(), "budget.view", json!({ "agent": agent })).await.unwrap();
        if v.get(0).and_then(|a| a.get(key)).is_some() {
            return v;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("{agent}: budget.view never carried {key}");
}

#[tokio::test(flavor = "multi_thread")]
async fn cursor_tier_reaches_budget_view() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&asked).await;
    let v = view_with(&k, "cursor", "plan").await;
    assert_eq!(v[0]["plan"], "Pro");
    assert_eq!(v[0]["windows"], json!([]), "a tier, no windows");
    assert!(v[0].get("spent").is_none(), "Cursor says no spend");
    assert!(asked.lock().unwrap().iter().all(|a| a == "cursor"));
}

#[tokio::test(flavor = "multi_thread")]
async fn opencode_stats_cost_and_tokens_reach_budget_view() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&asked).await;
    let v = view_with(&k, "opencode", "spent").await;
    let s = &v[0]["spent"];
    assert_eq!((s["cost"].as_f64(), s["days"].as_u64()), (Some(1.25), Some(7)));
    assert_eq!(s["tokens"].as_u64(), Some(287_300_000), "input + output + cache read + cache write");
    assert_eq!(s["at"].as_i64(), Some(NOW), "when it was read");
    assert!(v[0].get("plan").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn grok_usage_cost_reaches_budget_view() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&asked).await;
    let v = view_with(&k, "grok", "spent").await;
    let s = &v[0]["spent"];
    assert_eq!((s["cost"].as_f64(), s["days"].as_u64(), s["tokens"].as_u64()), (Some(0.118_592), Some(7), Some(136_102)));
}

/// Claude and Codex report their windows on every run: a view doesn't read them in the background.
#[tokio::test(flavor = "multi_thread")]
async fn a_view_of_claude_or_codex_reads_nothing() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&asked).await;
    for agent in ["claude", "codex", "copilot"] {
        let v = k.call(Caller::user(), "budget.view", json!({ "agent": agent })).await.unwrap();
        assert_eq!(v, json!([]));
    }
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    assert!(asked.lock().unwrap().is_empty(), "{:?}", asked.lock().unwrap());
}
