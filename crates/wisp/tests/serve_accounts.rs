//! wisp.accounts in `wisp serve` (v1 test/accounts.test.js, harness-watch.test.js, agent-login.test.js):
//! a second Codex account signs in with the CLI's own device login and its runs get its folder; a
//! harness whose program changed holds its turn until trusted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};

async fn until(what: &str, mut f: impl AsyncFnMut() -> Option<Value>) -> Value {
    let at = Instant::now();
    loop {
        if let Some(v) = f().await {
            return v;
        }
        assert!(at.elapsed() < Duration::from_secs(30), "{what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_codex_account_signs_in_and_its_runs_use_its_folder() {
    let dir = temp("serve-accounts");
    let rec = dir.join("rec");
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let added = api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    assert_eq!(added["accounts"].as_array().unwrap().len(), 2, "{added}");
    assert_eq!(added["account"]["label"], "Work");
    // the CLI's own login: its link and code, then done (it signed in)
    let login = until("the sign-in finishes", async || {
        let v = api.cmd("accounts.login", json!({ "agent": "codex", "account": "w1" })).await;
        (v["state"] == "done").then_some(v)
    })
    .await;
    assert_eq!(login["url"], "https://auth.example.dev/codex/device", "{login}");
    assert_eq!(login["code"], "ABCD-12345");
    let st = api.cmd("accounts.login.status", json!({ "agent": "codex", "account": "w1" })).await;
    assert_eq!(st["signedIn"], true, "{st}");
    // a task on that account runs with its folder as CODEX_HOME
    let id = api.cmd("runtime.create", json!({ "prompt": "fix", "cwd": dir.join("proj"), "agent": "codex", "account": "w1" })).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    api.until(&id, "the turn ends", |t| t.len() == 1 && ended(&t[0])).await;
    let runs = common::records(&rec.join("codex.jsonl"));
    let home = runs.iter().rev().find(|r| r["argv"][0] == "exec").map(|r| r["codexHome"].clone()).unwrap();
    // the same folder, however it's spelled (Windows: `\\` and the temp folder's 8.3 `RUNNER~1` against its long name)
    let want = dir.join("home").join("accounts").join("codex").join("w1");
    let canon = |p: &std::path::Path| wisp_proc::canonical_blocking(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    assert_eq!(canon(std::path::Path::new(home.as_str().unwrap())), canon(&want), "{home}");
    // reorder and remove
    let up = api.cmd("accounts.update", json!({ "agent": "codex", "account": "w1", "move": "up" })).await;
    assert_eq!(up["accounts"][0]["id"], "w1");
    let gone = api.cmd("accounts.remove", json!({ "agent": "codex", "account": "w1" })).await;
    assert_eq!(gone["accounts"].as_array().unwrap().len(), 1, "only the CLI's own is left: {gone}");
    drop(serve);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_harness_holds_its_turn_until_trusted() {
    let dir = temp("serve-watch");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    // the first run pins the program
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    assert!(api.cmd("accounts.watch", json!({ "agent": "codex" })).await["pinnedAt"].is_number());
    // another program where codex was: other bytes
    let bin = dir.join("bin").join(format!("codex{}", std::env::consts::EXE_SUFFIX));
    let mut bytes = std::fs::read(&bin).unwrap();
    std::fs::remove_file(&bin).unwrap();
    bytes.extend_from_slice(b"\0changed");
    std::fs::write(&bin, bytes).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let id = api.cmd("runtime.create", json!({ "prompt": "again", "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let t = api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
    assert_eq!(t[0]["held"]["kind"], "harness-changed", "{:?}", t[0]);
    assert_eq!(api.cmd("accounts.watch", json!({ "agent": "codex" })).await["changed"]["kind"], "warn");
    // trusting it pins what's there now and the held turn runs
    let w = api.cmd("accounts.watch.trust", json!({ "agent": "codex" })).await;
    assert!(w["changed"].is_null(), "{w}");
    api.until(&id, "the held turn runs", |t| t.last().is_some_and(|x| x["status"] == "done")).await;
    drop(serve);
}

/// TRY-17 #8: Start now on the card of a turn the watch held (`runtime.release`, a person's yes) trusts the changed
/// program and runs the turn. Before, the release ran `before:run` again against the old pin, which held the turn
/// again: a loop with nothing on screen, and Start now did nothing.
#[tokio::test(flavor = "multi_thread")]
async fn start_now_on_a_harness_changed_hold_trusts_the_program_and_runs() {
    let dir = temp("serve-watch-start");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    let bin = dir.join("bin").join(format!("codex{}", std::env::consts::EXE_SUFFIX));
    let mut bytes = std::fs::read(&bin).unwrap();
    std::fs::remove_file(&bin).unwrap();
    bytes.extend_from_slice(b"\0changed");
    std::fs::write(&bin, bytes).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let id = api.cmd("runtime.create", json!({ "prompt": "again", "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let t = api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
    assert_eq!(t[0]["held"]["kind"], "harness-changed", "{:?}", t[0]);
    let tid = t[0]["id"].as_str().unwrap().to_owned();
    api.cmd("runtime.release", json!({ "id": id, "turnId": tid, "action": "start" })).await;
    api.until(&id, "Start now runs the held turn", |t| t.len() == 1 && t[0]["status"] == "done").await;
    let w = api.cmd("accounts.watch", json!({ "agent": "codex" })).await;
    assert!(w["changed"].is_null(), "Start now trusted the new program: {w}");
    drop(serve);
}

/// The owner's Settings → Agents: an account added whose first sign-in didn't finish (cancelled here; timed out or
/// failed the same) had no state, so its row showed no Sign in at all, only its ⋯. It's signed out now (v1's
/// `signedOut`), and its row offers Sign in.
#[tokio::test(flavor = "multi_thread")]
async fn an_added_account_whose_first_sign_in_did_not_finish_is_signed_out() {
    let dir = temp("serve-accounts-never");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let added = api.cmd("accounts.add", json!({ "agent": "claude", "id": "t1", "label": "Te1", "kind": "work" })).await;
    assert_eq!(added["account"]["label"], "Te1", "{added}");
    api.cmd("accounts.login.cancel", json!({ "agent": "claude", "account": "t1" })).await;
    let acc = until("the account reads signed out", async || {
        let v = api.cmd("accounts.list", json!({ "agent": "claude" })).await;
        v["accounts"].as_array().and_then(|l| l.iter().find(|a| a["id"] == "t1").cloned()).filter(|a| a["state"] == "signedOut")
    })
    .await;
    assert_eq!(acc["label"], "Te1", "{acc}");
    drop(serve);
}

/// Sign out on an account whose sign-in was still under way: the row read "Signed out" beside the sign-in's own
/// Cancel, and the CLI's login kept running (it could still sign the account in after the user signed it out). Sign
/// out ends the sign-in first.
#[tokio::test(flavor = "multi_thread")]
async fn sign_out_during_a_sign_in_ends_that_sign_in() {
    let dir = temp("serve-accounts-out-during");
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    let (serve, port) = start_with(&dir, &[("FAKE_LOGIN_MS", "60000")]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    until("the sign-in waits", async || {
        let v = api.cmd("accounts.login", json!({ "agent": "codex", "account": "w1" })).await;
        (v["state"] == "waiting").then_some(v)
    })
    .await;
    api.cmd("accounts.logout", json!({ "agent": "codex", "account": "w1" })).await;
    let v = api.cmd("accounts.login", json!({ "agent": "codex", "account": "w1" })).await;
    assert!(!["running", "waiting"].contains(&v["state"].as_str().unwrap_or("")), "the sign-in still runs after Sign out: {v}");
    drop(serve);
}

/// Audit D10: Grok's Sign out (its own `grok logout`, then `grok models` as the check) left the row reading Signed in
/// with nothing said: the stand-in CLI answers `models` as signed in after its logout (a real grok doesn't). A CLI that
/// still answers signed in after its logout is said, as agy's sign-out says it, and isn't marked signed out.
#[tokio::test(flavor = "multi_thread")]
async fn a_cli_still_signed_in_after_its_logout_is_said_audit_d10() {
    let dir = temp("serve-accounts-grok-out");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let before = api.cmd("accounts.login.status", json!({ "agent": "grok" })).await;
    assert_eq!(before["signedIn"], true, "the stand-in answers `grok models`: {before}");
    let out = api.try_cmd("accounts.logout", json!({ "agent": "grok" })).await;
    assert_eq!(out["ok"], false, "{out}");
    let said = out.pointer("/error/message").and_then(Value::as_str).unwrap_or_default();
    // named as everywhere else (audit E9): "Grok", the command kept as typed
    assert!(said.starts_with("Grok is still signed in after `grok logout`") && said.contains("terminal"), "{out}");
    drop(serve);
}

/// Audit C tasks #3: Trust and start on one card trusted the changed program, but the other turns it held stayed held,
/// each card still saying the harness changed (a change wisp now trusts). A yes on one card is a yes to the program:
/// every turn the watch held for it starts, as Trust does.
#[tokio::test(flavor = "multi_thread")]
async fn trust_and_start_on_one_card_starts_every_turn_the_same_change_held_audit_c_3() {
    let dir = temp("serve-watch-start-all");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    change_codex(&dir);
    let mut held = Vec::new();
    for p in ["a", "b"] {
        let id = api.cmd("runtime.create", json!({ "prompt": p, "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let t = api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
        held.push((id, t[0]["id"].as_str().unwrap().to_owned()));
    }
    let ((a, a_turn), (b, _)) = (held[0].clone(), held[1].clone());
    api.cmd("runtime.release", json!({ "id": a, "turnId": a_turn, "action": "start" })).await;
    api.until(&a, "Trust and start runs its own turn", |t| t.len() == 1 && t[0]["status"] == "done").await;
    api.until(&b, "and the other turn the same change held", |t| t.len() == 1 && t[0]["status"] == "done").await;
    drop(serve);
}

/// opencode signs in with a key pasted into wisp: it goes where opencode keeps it, and only whether one
/// is saved comes back. Codex's usage windows are read from its app-server (nothing spent). The v1
/// routes answer as the commands do.
#[tokio::test(flavor = "multi_thread")]
async fn keys_limits_and_the_v1_routes() {
    let dir = temp("serve-keys");
    let data = dir.join("xdg");
    let (serve, port) = start_with(&dir, &[("XDG_DATA_HOME", data.to_str().unwrap())]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let v = api.cmd("accounts.keys", json!({ "agent": "opencode" })).await;
    assert_eq!(v["providers"].as_array().unwrap().len(), 2, "{v}");
    assert_eq!(v["providers"][0]["set"], false);
    let set = api.cmd("accounts.keys.set", json!({ "agent": "opencode", "provider": "opencode", "key": "sk-zen-1" })).await;
    assert_eq!(set["providers"][0]["set"], true, "{set}");
    assert!(!set.to_string().contains("sk-zen-1"), "never the key itself");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(data.join("opencode/auth.json")).unwrap()).unwrap();
    assert_eq!(file["opencode"], json!({ "type": "api", "key": "sk-zen-1" }));
    let bad = api.try_cmd("accounts.keys.set", json!({ "agent": "opencode", "provider": "nope", "key": "k" })).await;
    assert_ne!(bad["ok"], true);
    let after = api.cmd("accounts.keys.remove", json!({ "agent": "opencode", "provider": "opencode" })).await;
    assert_eq!(after["providers"][0]["set"], false, "{after}");
    // Codex's windows, read without a turn
    let l = api.cmd("accounts.limits", json!({ "agent": "codex", "account": "" })).await;
    assert_eq!(l["rateLimits"]["primary"]["usedPercent"], 42, "{l}");
    // every harness detected again
    let all = api.cmd("accounts.recheck", Value::Null).await;
    assert!(all.as_array().is_some_and(|a| a.iter().any(|h| h["id"] == "opencode" && h["available"] == true)), "{all}");
    assert!(api.cmd("accounts.list", json!({ "agent": "codex" })).await["accounts"].is_array());
    drop(serve);
}

/// Re-check in Settings → Agents looks for harnesses again here and on every online computer (v1 POST /agents/recheck:
/// `machines.recheckAgents()` with this one's detect): a harness removed over there reads not found here after it.
#[tokio::test(flavor = "multi_thread")]
async fn recheck_finds_harnesses_again_on_every_online_machine() {
    let (da, db) = (temp("recheck-a"), temp("recheck-b"));
    let (sa, pa) = start(&da);
    let (sb, pb) = start(&db);
    let a = Arc::new(Api { http: reqwest::Client::new(), port: pa, n: 0.into() });
    let added = a.cmd("machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": pb, "name": "studio" })).await;
    let id = added["id"].as_str().unwrap().to_owned();
    let grok =
        |v: &Value| v["remote"]["agents"].as_array().and_then(|l| l.iter().find(|x| x["id"] == "grok")).map(|x| x["available"].clone());
    until("the peer comes online with grok", async || {
        let v = a.cmd("machines.get", json!({ "id": id })).await;
        (v["state"] == "online" && grok(&v) == Some(json!(true))).then_some(v)
    })
    .await;
    // grok goes from b; a's Re-check finds that over there, not only here
    std::fs::remove_file(db.join("bin").join(format!("grok{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let here = a.cmd("accounts.recheck", Value::Null).await;
    assert!(
        here.as_array().is_some_and(|l| l.iter().any(|h| h["id"] == "grok" && h["available"] == true)),
        "a's own grok is still there: {here}"
    );
    // the other computer answers on its own row (Re-check never waits for it)
    until("b's grok is gone after a's Re-check", async || {
        let v = a.cmd("machines.get", json!({ "id": id })).await;
        (grok(&v) == Some(json!(false))).then_some(v)
    })
    .await;
    drop((sa, sb));
}

/// A run you start yourself skips an account at its limit (here the CLI's own login) and runs on the next one; the
/// turn says which, so its end, its usage reports and Plan budget count it there, not on the CLI's own login.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_moved_to_the_second_account_records_that_account_on_its_turn() {
    let dir = temp("serve-accounts-pick");
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    until("w1 signs in", async || {
        let v = api.cmd("accounts.login.status", json!({ "agent": "codex", "account": "w1" })).await;
        (v["signedIn"] == true).then_some(v)
    })
    .await;
    let until_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() + 3_600_000;
    api.cmd(
        "accounts.cool",
        json!({ "agent": "codex", "account": "default", "until": u64::try_from(until_ms).unwrap(), "why": "5-hour limit" }),
    )
    .await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    assert_eq!(turn["account"], "w1", "the turn ran on w1 and says so: {turn}");
    drop(serve);
}

/// Round 176: with two accounts, the CLI's own login hit its limit and is cooled until its reset three hours away
/// (`accounts.cool`); then a reading shows that plan reset early (`budget.plan_free`): the account is ready at once,
/// not at the old reset. Another account's cool is left as it was.
#[tokio::test(flavor = "multi_thread")]
async fn an_early_reset_ends_the_accounts_cool_at_once() {
    let dir = temp("serve-accounts-early");
    // the CLI's own login is signed in (the fake reads auth.json), or its row says signed out and never ready
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    std::fs::write(dir.join("codex-home/auth.json"), "{}").unwrap();
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    // w1's sign-in, which `accounts.add` starts, ends before the cool: a sign-in ends a cool, and this one would land
    // after it at a random time
    until("w1 signs in", async || {
        let v = api.cmd("accounts.login", json!({ "agent": "codex", "account": "w1" })).await;
        (v["state"] == "done").then_some(v)
    })
    .await;
    for account in ["", "w1"] {
        let st = api.cmd("accounts.login.status", json!({ "agent": "codex", "account": account })).await;
        assert_eq!(st["signedIn"], true, "{account:?} signed in: {st}");
    }
    let now = i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap();
    let old = now + 3 * 3_600_000;
    for account in ["default", "w1"] {
        api.cmd("accounts.cool", json!({ "agent": "codex", "account": account, "until": old, "why": "5-hour limit" })).await;
    }
    let until_of = async |id: &str| {
        let v = api.cmd("accounts.list", json!({ "agent": "codex" })).await;
        v["accounts"].as_array().and_then(|l| l.iter().find(|a| a["id"] == id)).map(|a| a["until"].clone()).unwrap_or_default()
    };
    assert_eq!(until_of("default").await, old, "cooled until the old reset");
    // a check that says still signed in (Settings → Agents runs one after every accounts command) keeps the cool
    for account in ["", "w1"] {
        api.cmd("accounts.login.status", json!({ "agent": "codex", "account": account })).await;
    }
    assert_eq!((until_of("default").await, until_of("w1").await), (json!(old), json!(old)), "still signed in: still cooled");
    // the CLI's own login: 80% of the five-hour window, resetting in three hours; then OpenAI banks a reset
    let read = |used: f64, at: i64| json!({ "agent": "codex", "account": "", "report": { "windows": [{ "id": "five_hour", "used": used, "resetsAt": at, "ms": 18_000_000 }], "complete": true } });
    api.cmd("budget.record", read(0.8, old)).await;
    api.cmd("budget.record", read(0.02, now + 5 * 3_600_000)).await;
    until("the early reset ends the cool", async || {
        let v = api.cmd("accounts.list", json!({ "agent": "codex" })).await;
        let d = v["accounts"].as_array()?.iter().find(|a| a["id"] == "default")?.clone();
        (d["state"] == "ready" && d["until"].is_null()).then_some(d)
    })
    .await;
    assert_eq!(until_of("w1").await, old, "another account's plan didn't reset");
    drop(serve);
}

/// The fake codex with other bytes (the same other bytes each time, so a second change is the same program).
fn change_codex(dir: &std::path::Path) {
    let bin = dir.join("bin").join(format!("codex{}", std::env::consts::EXE_SUFFIX));
    let mut bytes = std::fs::read(&bin).unwrap();
    std::fs::remove_file(&bin).unwrap();
    bytes.extend_from_slice(b"\0changed");
    std::fs::write(&bin, bytes).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// TRY-17 #29: the turns the harness watch held lived only in memory. After a restart Trust (`accounts.watch.trust`)
/// started none of them, and a held card stayed held until something else released it. Both ways work after a
/// restart: Trust starts every turn it held, and Start now on a card (`runtime.release`) runs its turn.
#[tokio::test(flavor = "multi_thread")]
async fn after_a_restart_trust_starts_the_turns_the_watch_held_and_start_now_runs_one() {
    let dir = temp("serve-watch-restart");
    let (mut serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    change_codex(&dir);
    let mut held = Vec::new();
    for p in ["a", "b"] {
        let id = api.cmd("runtime.create", json!({ "prompt": p, "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let t = api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
        held.push((id, t[0]["id"].as_str().unwrap().to_owned()));
    }
    // a restart: the same home and the same (changed) codex, never installed again or deleted while the new server
    // may be running it (Windows refuses that: Access is denied)
    let changed = std::fs::read(dir.join("bin").join(format!("codex{}", std::env::consts::EXE_SUFFIX))).unwrap();
    serve.keep = true;
    drop(serve);
    let (serve, port) = restart(&dir);
    assert_eq!(
        std::fs::read(dir.join("bin").join(format!("codex{}", std::env::consts::EXE_SUFFIX))).unwrap(),
        changed,
        "the restart kept the changed program"
    );
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let ((a, a_turn), (b, _)) = (held[0].clone(), held[1].clone());
    // Start now on one card
    api.cmd("runtime.release", json!({ "id": a, "turnId": a_turn, "action": "start" })).await;
    api.until(&a, "Start now runs it after a restart", |t| t.len() == 1 && t[0]["status"] == "done").await;
    // Trust starts the other
    api.cmd("accounts.watch.trust", json!({ "agent": "codex" })).await;
    api.until(&b, "Trust starts the turn it held before the restart", |t| t.len() == 1 && t[0]["status"] == "done").await;
    drop(serve);
}

/// TRY-17 (b): a held turn skipped, archived or deleted left its entry in the watch's list (kept across restarts since
/// #29), so a later Trust started the archived one. They leave the list; Trust starts only what's still held.
#[tokio::test(flavor = "multi_thread")]
async fn trust_starts_only_turns_still_held_not_skipped_archived_or_deleted_ones() {
    let dir = temp("serve-watch-stale");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    change_codex(&dir);
    let mut held = Vec::new();
    for p in ["skip", "archive", "delete", "keep"] {
        let id = api.cmd("runtime.create", json!({ "prompt": p, "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let t = api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
        held.push((id, t[0]["id"].as_str().unwrap().to_owned()));
    }
    let [(skip, skip_turn), (archive, _), (delete, _), (keep, _)] = <[(String, String); 4]>::try_from(held).unwrap();
    api.cmd("runtime.release", json!({ "id": skip, "turnId": skip_turn, "action": "skip" })).await;
    api.cmd("runtime.archive", json!({ "id": archive, "archived": true })).await;
    api.cmd("runtime.delete", json!({ "id": delete })).await;
    api.cmd("accounts.watch.trust", json!({ "agent": "codex" })).await;
    api.until(&keep, "Trust starts the one still held", |t| t.len() == 1 && t[0]["status"] == "done").await;
    let a = api.cmd("runtime.turns", json!({ "id": archive })).await;
    assert_eq!(a[0]["status"], "held", "an archived held task stays held, never started by Trust: {a}");
    let s = api.cmd("runtime.turns", json!({ "id": skip })).await;
    assert_eq!(s[0]["status"], "stopped", "{s}");
    drop(serve);
}

/// QA r1 #10: a held task marked done (by hand, or an agent's `done`) sat under Done saying "Waiting for codex", and a
/// later Trust could start it. Marked done, Trust leaves it alone; reopened, it's held as before (Mark done's Undo
/// brings it back as it was) and Trust starts it.
#[tokio::test(flavor = "multi_thread")]
async fn trust_leaves_a_held_task_marked_done_and_starts_it_once_reopened() {
    let dir = temp("serve-watch-done");
    let (serve, port) = start(&dir);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.watch.ask", json!({ "on": true })).await;
    let (turn, _) = run_task(&api, &dir, "codex").await;
    assert_eq!(turn["status"], "done", "{turn}");
    change_codex(&dir);
    let (done, kept) = {
        let mut ids = Vec::new();
        for p in ["done", "kept"] {
            let id = api.cmd("runtime.create", json!({ "prompt": p, "cwd": dir.join("proj"), "agent": "codex" })).await["id"]
                .as_str()
                .unwrap()
                .to_owned();
            api.until(&id, "held", |t| t.len() == 1 && t[0]["status"] == "held").await;
            ids.push(id);
        }
        (ids[0].clone(), ids[1].clone())
    };
    api.cmd("runtime.setState", json!({ "id": done, "state": "done" })).await;
    api.cmd("accounts.watch.trust", json!({ "agent": "codex" })).await;
    api.until(&kept, "Trust starts the one still waiting", |t| t.len() == 1 && t[0]["status"] == "done").await;
    assert_eq!(api.cmd("runtime.turns", json!({ "id": done })).await[0]["status"], "held", "a task marked done isn't started");
    // reopened (Undo): held as before, and its card's Start now still runs it
    api.cmd("runtime.setState", json!({ "id": done, "state": "review" })).await;
    let t = api.cmd("runtime.turns", json!({ "id": done })).await;
    assert_eq!(t[0]["status"], "held", "{t}");
    let tid = t[0]["id"].as_str().unwrap().to_owned();
    api.cmd("runtime.release", json!({ "id": done, "turnId": tid, "action": "start" })).await;
    api.until(&done, "Start now runs it once reopened", |t| t.len() == 1 && t[0]["status"] == "done").await;
    drop(serve);
}

/// Subscribes to `topic` on the sync stream and collects its frames until `until` holds (20 s at most).
async fn stream(port: u16, client: &str, topic: &str, until: impl Fn(&str) -> bool) -> String {
    let http = reqwest::Client::new();
    let mut r =
        http.get(format!("http://127.0.0.1:{port}/api/events?proto_min=2&proto_max=2&client_id={client}&build=t")).send().await.unwrap();
    let (mut seen, at, mut subbed) = (String::new(), Instant::now(), false);
    while at.elapsed() < Duration::from_secs(20) {
        let Ok(Ok(Some(c))) = tokio::time::timeout(Duration::from_millis(300), r.chunk()).await else {
            if subbed && until(&seen) {
                break;
            }
            continue;
        };
        seen.push_str(&String::from_utf8_lossy(&c));
        if !subbed && seen.contains("welcome") {
            let ok = http
                .post(format!("http://127.0.0.1:{port}/api/events"))
                .header("content-type", "application/json")
                .header("x-wisp-client", client)
                .body(json!({ "t": "sub", "topic": topic }).to_string())
                .send()
                .await
                .unwrap();
            assert_eq!(ok.status(), 200);
            subbed = true;
        }
        if subbed && until(&seen) {
            break;
        }
    }
    seen
}

/// Each `accounts.login` row as a sync stream left it, by id: a snapshot or an upsert puts the whole row, a patch
/// replaces its fields (`/state`, `/code`…); with every state it went through, in order.
fn logins_in(frames: &str) -> std::collections::BTreeMap<String, (Value, Vec<String>)> {
    fn ops<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
        match v {
            Value::Object(m) if m.get("e").and_then(Value::as_str) == Some("accounts.login") => out.push(v),
            Value::Object(m) => m.values().for_each(|x| ops(x, out)),
            Value::Array(a) => a.iter().for_each(|x| ops(x, out)),
            _ => {}
        }
    }
    let mut rows = std::collections::BTreeMap::<String, (Value, Vec<String>)>::new();
    for line in frames.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim_start_matches("data:").trim()) else { continue };
        let mut seen = Vec::new();
        ops(&v, &mut seen);
        for op in seen {
            let id = op["id"].as_str().unwrap_or_default().to_owned();
            let e = rows.entry(id).or_insert_with(|| (json!({}), Vec::new()));
            if let Some(row) = op.get("v") {
                e.0 = row.clone();
            }
            for p in op["ops"].as_array().into_iter().flatten() {
                if let Some(field) = p["path"].as_str().and_then(|x| x.strip_prefix('/')) {
                    e.0[field] = p["value"].clone();
                }
            }
            if let Some(st) = e.0["state"].as_str()
                && e.1.last().map(String::as_str) != Some(st)
            {
                e.1.push(st.to_owned());
            }
        }
    }
    rows
}

/// The owner's audit (2026-11-18, Settings → Agents): a sign-in's link, code and end reached the page only with the next
/// command (the rows were written after commands alone), so an open pane showed nothing and a finished sign-in stayed
/// "waiting". Its progress reaches the topic as it happens.
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_ins_progress_and_its_end_reach_the_agents_pane_without_another_command() {
    let dir = temp("serve-accounts-live");
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    let (serve, port) = start_with(&dir, &[("FAKE_LOGIN_MS", "2500")]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let done = |f: &str| logins_in(f).get("codex@w1").is_some_and(|(row, _)| row["state"] == "done");
    let watch = tokio::spawn(stream(port, "c-live", "plugin:wisp.accounts:harnesses", done));
    tokio::time::sleep(Duration::from_millis(600)).await;
    api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    let frames = watch.await.unwrap();
    let (row, states) = logins_in(&frames).remove("codex@w1").unwrap_or_default();
    assert_eq!(row["code"], "ABCD-12345", "its code, as it was printed: {states:?}");
    assert_eq!(states.last().map(String::as_str), Some("done"), "its end, with no command after it: {states:?}");
    // and signed in: the row says Signed in, not just that a sign-in ended (v1 agent-login: done → signedIn)
    assert_eq!(row["signedIn"], true, "{row}");
    drop(serve);
}

/// A sign-in under way when wisp stopped has no CLI behind it after a restart: it isn't left "waiting" (a link that
/// leads nowhere, a Cancel that cancels nothing); the account can sign in again.
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_cut_by_a_restart_is_not_left_waiting() {
    let dir = temp("serve-accounts-restart-login");
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    let (mut serve, port) = start_with(&dir, &[("FAKE_LOGIN_MS", "60000")]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    api.cmd("accounts.add", json!({ "agent": "codex", "id": "w1", "label": "Work", "kind": "work" })).await;
    until("the sign-in waits", async || {
        let v = api.cmd("accounts.login", json!({ "agent": "codex", "account": "w1" })).await;
        (v["state"] == "waiting").then_some(v)
    })
    .await;
    serve.keep = true;
    drop(serve);
    let (serve, port) = restart(&dir);
    // what an open Settings → Agents reads: the rows on the topic, once the plugin is up
    let frames = stream(port, "c-restart", "plugin:wisp.accounts:harnesses", |f| f.contains("accounts.harness")).await;
    let w1 = logins_in(&frames).remove("codex@w1");
    assert!(w1.as_ref().is_none_or(|(row, _)| !["running", "waiting"].contains(&row["state"].as_str().unwrap_or(""))), "{w1:?}");
    // its first sign-in never finished: the account reads signed out, so its row offers Sign in (the owner's "Te1")
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let list = api.cmd("accounts.list", json!({ "agent": "codex" })).await;
    let acc = list["accounts"].as_array().and_then(|l| l.iter().find(|a| a["id"] == "w1").cloned());
    assert_eq!(acc.as_ref().map(|a| a["state"].clone()), Some(json!("signedOut")), "{list}");
    drop(serve);
}

/// `PostHog`, alpha.15 (27 Nov): three extra Claude accounts were added and removed at once, and the removes reported
/// 2, 3 and 1 accounts left, not 3, 2, 1. Each remove read the list, waited on its sign-in's cancel and the CLI's
/// logout (seconds on a real CLI), then saved the list it read before: a remove that finished later put back the
/// accounts the others had just removed. Removes at once leave the list as if they ran one after another.
#[tokio::test(flavor = "multi_thread")]
async fn accounts_removed_at_once_all_go() {
    let dir = temp("serve-accounts-remove-at-once");
    let (serve, port) = start_with(&dir, &[("FAKE_LOGIN_MS", "60000")]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    for id in ["b1", "b2", "b3"] {
        api.cmd("accounts.add", json!({ "agent": "claude", "id": id, "label": id, "kind": "personal" })).await;
    }
    let added = api.cmd("accounts.list", json!({ "agent": "claude" })).await;
    assert_eq!(added["accounts"].as_array().unwrap().len(), 4, "{added}");
    let gone = ["b1", "b2", "b3"].map(|id| {
        let api = api.clone();
        tokio::spawn(async move { api.cmd("accounts.remove", json!({ "agent": "claude", "account": id })).await })
    });
    for g in gone {
        g.await.unwrap();
    }
    let left = api.cmd("accounts.list", json!({ "agent": "claude" })).await;
    let ids: Vec<&str> = left["accounts"].as_array().unwrap().iter().filter_map(|a| a["id"].as_str()).collect();
    assert_eq!(ids, ["default"], "a remove put back an account another had removed: {left}");
    drop(serve);
}
