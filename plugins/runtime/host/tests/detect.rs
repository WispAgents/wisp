//! `runtime.agents` detects every harness at once: each detect spawns its CLI a few times, and one after another
//! `wisp_available`, `wisp_agents` and Settings → Agents waited for the sum of every probe, not the slowest harness.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{Fake, rig_on};
use tokio::sync::Barrier;
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::Harness;
use wisp_runtime_api::commands::AgentsArgs;
use wisp_runtime_api::harness::{HarnessFound, HarnessInfo, HarnessRun, HarnessSys, Launch, RunDriver};

/// A harness whose detect answers only once every harness's detect has started.
struct Meets {
    inner: Arc<dyn Harness>,
    all: Arc<Barrier>,
}

impl Harness for Meets {
    fn info(&self) -> &HarnessInfo {
        self.inner.info()
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async move {
            self.all.wait().await;
            HarnessFound { available: true, version: Some(format!("{} 1.0", self.inner.info().id)), ..HarnessFound::default() }
        })
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        self.inner.launch(run)
    }
    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        self.inner.driver(run)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_agents_detects_every_harness_at_once_in_registration_order() {
    let ids = ["a", "b", "c", "d"];
    let all = Arc::new(Barrier::new(ids.len()));
    let harnesses: Vec<Arc<dyn Harness>> =
        ids.iter().map(|id| Arc::new(Meets { inner: Fake::named(id, id, &[]), all: all.clone() }) as Arc<dyn Harness>).collect();
    let r = rig_on(Recorder::default(), harnesses).await;
    // one at a time, the first detect waits forever for the others
    let list = tokio::time::timeout(Duration::from_secs(5), r.rt.agents(&AgentsArgs::default()))
        .await
        .expect("runtime.agents ran the detects one after another");
    assert_eq!(list.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ids, "the list keeps the harnesses' order");
    assert!(list.iter().all(|a| a.available && a.version.as_deref() == Some(&format!("{} 1.0", a.id))), "{list:?}");
}

/// A harness whose detect waits for its gate; each detect answers `<id> <n>.0` (n: how many it ran).
struct Gated {
    inner: Arc<dyn Harness>,
    gates: Vec<tokio::sync::watch::Receiver<bool>>,
    runs: std::sync::atomic::AtomicUsize,
    available: bool,
}

impl Gated {
    fn new(id: &str, available: bool, gates: Vec<tokio::sync::watch::Receiver<bool>>) -> Arc<Self> {
        Arc::new(Self { inner: Fake::named(id, &id.to_uppercase(), &[]), gates, runs: std::sync::atomic::AtomicUsize::new(0), available })
    }
}

impl Harness for Gated {
    fn info(&self) -> &HarnessInfo {
        self.inner.info()
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async move {
            let n = self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(g) = self.gates.get(n) {
                let mut g = g.clone();
                let _ = g.wait_for(|open| *open).await;
            }
            HarnessFound {
                available: self.available,
                version: self.available.then(|| format!("{} {}.0", self.inner.info().id, n + 1)),
                ..HarnessFound::default()
            }
        })
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        self.inner.launch(run)
    }
    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        self.inner.driver(run)
    }
}

/// The `harnesses` topic's rows: (id, ord, state, version).
async fn topic(store: &wisp_store::Store) -> Vec<(String, u64, String, Option<String>)> {
    let mut rows: Vec<_> = store
        .read_async(|r| r.query("runtime.harness", &wisp_store::Query::new()))
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            let s = |k: &str| row.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_owned();
            let ord = row.get("ord").and_then(serde_json::Value::as_u64).unwrap();
            let version = row.get("agent").and_then(|a| a.get("version")).and_then(serde_json::Value::as_str).map(str::to_owned);
            (s("id"), ord, s("state"), version)
        })
        .collect();
    rows.sort_by_key(|r| r.1);
    rows
}

/// Waits (≤ 5 s) until the topic's (id, state) pairs are `want`.
async fn topic_is(store: &wisp_store::Store, want: &[(&str, &str)]) -> Vec<(String, u64, String, Option<String>)> {
    let at = std::time::Instant::now();
    loop {
        let rows = topic(store).await;
        let now: Vec<(&str, &str)> = rows.iter().map(|r| (r.0.as_str(), r.2.as_str())).collect();
        if now == want {
            return rows;
        }
        assert!(at.elapsed() < Duration::from_secs(5), "the topic never read {want:?}; it reads {now:?}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// ADR-0066: the picker and Settings → Agents render before any CLI answers. Every harness is on the topic at once as
/// `checking`, each settles the moment its own detect lands, and a hung one holds back only its own row.
#[tokio::test(flavor = "multi_thread")]
async fn the_harnesses_topic_shows_every_harness_checking_at_once_then_each_as_it_settles() {
    let (a_go, a_rx) = tokio::sync::watch::channel(false);
    let (hung_go, hung_rx) = tokio::sync::watch::channel(false);
    let (b_go, b_rx) = tokio::sync::watch::channel(false);
    let harnesses: Vec<Arc<dyn Harness>> =
        vec![Gated::new("a", true, vec![a_rx]), Gated::new("hung", true, vec![hung_rx]), Gated::new("b", false, vec![b_rx])];
    let r = rig_on(Recorder::default(), harnesses).await;
    let rt = r.rt.clone();
    let call = tokio::spawn(async move { rt.agents(&AgentsArgs::default()).await });
    let rows = topic_is(&r.store, &[("a", "checking"), ("hung", "checking"), ("b", "checking")]).await;
    assert_eq!(rows.iter().map(|x| x.1).collect::<Vec<_>>(), [0, 1, 2], "rows keep the registration order");
    assert!(rows.iter().all(|x| x.3.is_none()), "nothing is known before a detect answers: {rows:?}");
    a_go.send(true).unwrap();
    let rows = topic_is(&r.store, &[("a", "installed"), ("hung", "checking"), ("b", "checking")]).await;
    assert_eq!(rows[0].3.as_deref(), Some("a 1.0"));
    b_go.send(true).unwrap();
    topic_is(&r.store, &[("a", "installed"), ("hung", "checking"), ("b", "missing")]).await;
    assert!(!call.is_finished(), "runtime.agents answered before the hung harness did");
    hung_go.send(true).unwrap();
    let list = tokio::time::timeout(Duration::from_secs(5), call).await.unwrap().unwrap();
    assert_eq!(list.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["a", "hung", "b"], "runtime.agents still answers the whole list");
    let rows = topic_is(&r.store, &[("a", "installed"), ("hung", "installed"), ("b", "missing")]).await;
    assert_eq!(rows[1].3.as_deref(), Some("hung 1.0"));
}

/// Waits (≤ 5 s) until `h.detect()` has started at least `want` times, so a second round can't race ahead and
/// take the earlier round's gate (`gates[0]`), which would deadlock the test when it releases `old_go` first.
async fn wait_runs(h: &Gated, want: usize) {
    let at = std::time::Instant::now();
    while h.runs.load(std::sync::atomic::Ordering::SeqCst) < want {
        assert!(
            at.elapsed() < Duration::from_secs(5),
            "expected {want} detect runs to start; only {} started",
            h.runs.load(std::sync::atomic::Ordering::SeqCst)
        );
        tokio::task::yield_now().await;
    }
}

/// Settings' Re-check while the picker's ask still waits on a hung CLI: the older round's late answer never
/// overwrites the Re-check's; and a plain round over a settled harness leaves its row as it was (no `checking` flash).
#[tokio::test(flavor = "multi_thread")]
async fn an_older_rounds_late_answer_never_overwrites_a_newer_one() {
    let (old_go, old_rx) = tokio::sync::watch::channel(false);
    let h = Gated::new("h", true, vec![old_rx]);
    let r = rig_on(Recorder::default(), vec![h.clone() as Arc<dyn Harness>]).await;
    let rt = r.rt.clone();
    let old = tokio::spawn(async move { rt.agents(&AgentsArgs::default()).await });
    topic_is(&r.store, &[("h", "checking")]).await;
    wait_runs(&h, 1).await;
    let fresh = tokio::time::timeout(Duration::from_secs(5), r.rt.agents(&AgentsArgs { fresh: Some("h".into()) }))
        .await
        .expect("fresh round timed out");
    assert_eq!(fresh[0].version.as_deref(), Some("h 2.0"));
    assert_eq!(topic_is(&r.store, &[("h", "installed")]).await[0].3.as_deref(), Some("h 2.0"));
    old_go.send(true).unwrap();
    let old = tokio::time::timeout(Duration::from_secs(5), old).await.expect("old round timed out").unwrap();
    assert_eq!(old[0].version.as_deref(), Some("h 1.0"), "the older round still answers its own caller");
    assert_eq!(topic(&r.store).await[0].3.as_deref(), Some("h 2.0"), "the older round's answer overwrote the Re-check's");
    // a plain round: the row never goes back to checking, and lands in place
    let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel();
    let store = r.store.clone();
    let watch = tokio::spawn(async move {
        loop {
            let _ = seen_tx.send(topic(&store).await[0].2.clone());
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    r.rt.agents(&AgentsArgs::default()).await;
    watch.abort();
    while let Ok(state) = seen.try_recv() {
        assert_eq!(state, "installed", "a settled harness flashed `checking` on a plain round");
    }
    assert_eq!(topic(&r.store).await[0].3.as_deref(), Some("h 3.0"));
}

/// Review of ADR-0066: a Re-check's `checking` was written, then a plain round that started before it answered: that
/// older answer (from before the install) landed over the Re-check's row. A row a newer round wrote stays its.
#[tokio::test(flavor = "multi_thread")]
async fn an_older_answer_never_lands_over_a_newer_rounds_checking() {
    let (old_go, old_rx) = tokio::sync::watch::channel(false);
    let (new_go, new_rx) = tokio::sync::watch::channel(false);
    let h = Gated::new("h", true, vec![old_rx, new_rx]);
    let r = rig_on(Recorder::default(), vec![h.clone() as Arc<dyn Harness>]).await;
    let rt = r.rt.clone();
    let old = tokio::spawn(async move { rt.agents(&AgentsArgs::default()).await });
    topic_is(&r.store, &[("h", "checking")]).await;
    // wait until `old` is actually blocked on `old_rx` before spawning `recheck` (otherwise `recheck` can grab `old_rx`)
    wait_runs(&h, 1).await;
    let rt = r.rt.clone();
    let recheck = tokio::spawn(async move { rt.agents(&AgentsArgs { fresh: Some("all".into()) }).await });
    // both rounds are waiting on their detects (the Re-check's wrote its checking before its detect started)
    wait_runs(&h, 2).await;
    old_go.send(true).unwrap();
    let old_res = tokio::time::timeout(Duration::from_secs(5), old).await.expect("old round timed out").unwrap();
    assert_eq!(old_res[0].version.as_deref(), Some("h 1.0"));
    assert_eq!(topic(&r.store).await[0].2, "checking", "the older round's answer landed over the Re-check's checking");
    new_go.send(true).unwrap();
    let recheck_res = tokio::time::timeout(Duration::from_secs(5), recheck).await.expect("recheck round timed out").unwrap();
    assert_eq!(recheck_res[0].version.as_deref(), Some("h 2.0"));
    assert_eq!(topic_is(&r.store, &[("h", "installed")]).await[0].3.as_deref(), Some("h 2.0"));
}

/// Review of ADR-0066: a caller that hangs up (an MCP client gone, a page closed) mid-round left rows `checking`
/// until someone asked again. The round runs on its own task and still lands every answer.
#[tokio::test(flavor = "multi_thread")]
async fn a_caller_that_hangs_up_never_leaves_rows_checking() {
    let (go, rx) = tokio::sync::watch::channel(false);
    let r = rig_on(Recorder::default(), vec![Gated::new("h", true, vec![rx]) as Arc<dyn Harness>]).await;
    let rt = r.rt.clone();
    let asked = tokio::spawn(async move { rt.agents(&AgentsArgs::default()).await });
    topic_is(&r.store, &[("h", "checking")]).await;
    asked.abort();
    assert!(asked.await.unwrap_err().is_cancelled());
    go.send(true).unwrap();
    assert_eq!(topic_is(&r.store, &[("h", "installed")]).await[0].3.as_deref(), Some("h 1.0"));
}

/// The harness rows written after `seq`: (id, state) in order.
async fn harness_writes(store: &wisp_store::Store, seq: i64) -> Vec<(String, String)> {
    let changes = store.read_async(move |r| r.changes_since(seq, None, 1_000)).await.unwrap();
    changes
        .iter()
        .filter(|c| c.entity == "runtime.harness")
        .map(|c| {
            let state = c
                .patch
                .as_array()
                .and_then(|ops| ops.iter().find(|o| o["path"] == "/state"))
                .map_or("", |o| o["value"].as_str().unwrap_or(""));
            let state = if c.patch.get("state").is_some() { c.patch["state"].as_str().unwrap_or("") } else { state };
            (c.id.clone(), state.to_owned())
        })
        .collect()
}

/// ADR-0084: a round whose answers are what the rows already say writes nothing. A start runs several rounds at once
/// (runtime's own, and router, telemetry, accounts asking `runtime.agents`): each rewrote every row for its `at` alone,
/// 28 of a start's 42 harness writes, on the one writer thread every plugin's start waits on. A Re-check still shows
/// `checking`, then the answer.
#[tokio::test(flavor = "multi_thread")]
async fn a_round_that_finds_what_the_rows_already_say_writes_nothing_and_a_recheck_still_shows_checking() {
    let r = rig_on(Recorder::default(), vec![Fake::named("a", "A", &[]), Fake::named("b", "B", &[])]).await;
    r.rt.agents(&AgentsArgs::default()).await;
    topic_is(&r.store, &[("a", "installed"), ("b", "installed")]).await;
    let head = r.store.head_seq().unwrap();
    r.rt.agents(&AgentsArgs::default()).await;
    let plain = AgentsArgs::default();
    let (x, y, z) = tokio::join!(r.rt.agents(&plain), r.rt.agents(&plain), r.rt.agents(&plain));
    assert!([x, y, z].iter().all(|l| l.len() == 2 && l.iter().all(|a| a.available)), "every round still answers its caller");
    assert_eq!(harness_writes(&r.store, head).await, Vec::<(String, String)>::new(), "rounds that found nothing new rewrote rows");
    r.rt.agents(&AgentsArgs { fresh: Some("all".into()) }).await;
    let wrote = harness_writes(&r.store, head).await;
    let states: Vec<&str> = wrote.iter().map(|w| w.1.as_str()).collect();
    assert_eq!(states, ["checking", "checking", "installed", "installed"], "a Re-check shows checking, then the answer: {wrote:?}");
    topic_is(&r.store, &[("a", "installed"), ("b", "installed")]).await;
}
