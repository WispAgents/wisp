//! `runtime.agents` (the runner picker, Settings → Agents, `wisp_available`) is quick when CLIs hang (ADR-0011,
//! round 212). Each harness's detect runs its CLI a few times (`--version`, `--help`, `models`, `auth list`), each
//! killed after 15 s: one after another, opencode's three hung probes took 45 s. Now a detect's probes run at once,
//! the same probe asked twice runs once, and an answer stands while the CLI is the same file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{Api, start_with, temp};
use serde_json::{Value, json};

/// The probes a detect runs past `--version` (which the fakes always answer), by `<cli> <argv>` (the fake codex
/// answers `debug models` before it records; opencode's plain `models` is ended once `--verbose` lists models, so it
/// may or may not have recorded).
const DETECT: &[&str] = &[
    "agy --help",
    "agy models",
    "copilot help config",
    "copilot --help",
    "cursor-agent --list-models",
    "grok models",
    "opencode models --verbose",
    "opencode auth list",
];

/// How many times each detect probe ran, from `FAKE_RECORD`.
fn ran(rec: &Path) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(rec).into_iter().flatten().flatten() {
        let name = e.path().file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
        for r in common::records(&e.path()) {
            let probe = format!("{name} {}", common::argv(&r).join(" "));
            if r["stdin"].as_str().is_some_and(|s| s.trim().is_empty()) && DETECT.contains(&probe.as_str()) {
                *out.entry(probe).or_default() += 1;
            }
        }
    }
    out
}

/// `ran` once it held still for `quiet`.
async fn settled(rec: &Path, quiet: Duration) -> BTreeMap<String, usize> {
    let (mut last, mut since) = (ran(rec), Instant::now());
    while since.elapsed() < quiet {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = ran(rec);
        if now != last {
            (last, since) = (now, Instant::now());
        }
    }
    last
}

async fn timed(api: &Api, args: Value) -> (Value, Duration) {
    let at = Instant::now();
    let v = api.cmd("runtime.agents", args).await;
    (v, at.elapsed())
}

/// Round 212: every CLI answers `--version` but hangs on everything else. Before, the answer waited for each
/// harness's probes one after another (opencode: `models --verbose`, `models`, `auth list`, 15 s each); now one time
/// limit (15 s), however many probes hang.
#[tokio::test(flavor = "multi_thread")]
async fn with_every_cli_hung_runtime_agents_waits_one_time_limit() {
    let dir = temp("serve-detect-hung");
    let (serve, port) = start_with(&dir, &[("FAKE_HANG", "1")]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (list, took) = timed(&api, json!({})).await;
    eprintln!("runtime.agents, every probe hung: {took:?}");
    assert!(took < Duration::from_secs(20), "it waited {took:?}: several time limits, one after another");
    let up: Vec<&str> = list.as_array().unwrap().iter().filter(|a| a["available"] == true).filter_map(|a| a["id"].as_str()).collect();
    for id in ["claude", "codex", "opencode", "cursor", "copilot", "grok", "antigravity"] {
        assert!(up.contains(&id), "{id} answered --version and is available: {list}");
    }
    // the probes timed out: asked again, the hangs stand (for a while), not another 15 s
    let (_, again) = timed(&api, json!({})).await;
    eprintln!("runtime.agents again, every probe hung: {again:?}");
    assert!(again < Duration::from_secs(2), "the second ask waited {again:?}");
    drop(serve);
}

/// Healthy CLIs: each probe runs once however many ask (wisp.accounts at start, this call); a second ask answers at
/// once from what was kept, without a probe; a CLI that changed (an update) is probed again, the others aren't;
/// Re-check (`fresh`) probes that harness again.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_ask_is_answered_from_the_cache_and_a_changed_cli_is_probed_again() {
    let dir = temp("serve-detect-cache");
    let rec = dir.join("rec");
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (first, took) = timed(&api, json!({})).await;
    let once = settled(&rec, Duration::from_secs(1)).await;
    eprintln!("runtime.agents, healthy, first: {took:?}, probes {once:?}");
    let (second, again) = timed(&api, json!({})).await;
    eprintln!("runtime.agents, healthy, second: {again:?}");
    // `agy models` is also agy's sign-in status (wisp.accounts reads it apart from detect)
    assert!(once.iter().all(|(p, &n)| n == 1 || p == "agy models"), "a probe ran more than once: {once:?}");
    assert_eq!(once.len(), DETECT.len(), "{once:?}");
    assert_eq!(first, second, "the kept answer is the same answer");
    assert!(again < Duration::from_millis(500), "the second ask took {again:?}");
    assert_eq!(settled(&rec, Duration::from_millis(500)).await, once, "the second ask ran a probe");

    // agy updated: another file at the same path
    let agy = dir.join("bin").join(format!("agy{}", std::env::consts::EXE_SUFFIX));
    let real = wisp_proc::canonical_blocking(&agy).unwrap();
    std::fs::remove_file(&agy).unwrap();
    std::fs::copy(&real, &agy).unwrap();
    api.cmd("runtime.agents", json!({})).await;
    let after = settled(&rec, Duration::from_millis(500)).await;
    assert_eq!(after["agy --help"], 2, "the updated agy wasn't probed again: {after:?}");
    assert!(after["agy models"] > once["agy models"], "the updated agy's models weren't read again: {after:?}");
    assert_eq!(after["cursor-agent --list-models"], 1, "an unchanged CLI was probed again: {after:?}");

    // Re-check on one harness
    api.cmd("runtime.agents", json!({ "fresh": "cursor" })).await;
    let after = settled(&rec, Duration::from_millis(500)).await;
    assert_eq!(after["cursor-agent --list-models"], 2, "Re-check didn't probe cursor again: {after:?}");
    assert_eq!(after["agy --help"], 2, "Re-check on cursor probed agy: {after:?}");
    drop(serve);
}
