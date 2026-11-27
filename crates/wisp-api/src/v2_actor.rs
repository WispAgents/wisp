//! The v2 stream's actor: the one task that owns wisp-sync's machine. Every input reaches it as a
//! `Msg`; it does what the machine returns (frames to each stream's queue, reads to `Snapshots`,
//! commands to `Backend::call` once per mid) and sleeps until the machine's next wake.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use serde_json::Value;
use tokio::sync::mpsc;
use wisp_kernel::{Caller, Result, WireError};
use wisp_sync::{CommandOutcome, ConnId, Input, Output, SnapshotAnswer, SyncServer};

use super::{Msg, Read, Snapshots, now_ms};
use crate::backend::Backend;
use crate::sync::{Idempotency, command_answer};

struct Stream {
    pub(super) tx: mpsc::Sender<Bytes>,
    caller: Caller,
    /// bytes queued and not yet written to the socket: the machine's backpressure signal
    queued: Arc<AtomicUsize>,
    pub(super) active: bool,
}

pub(super) struct Actor {
    server: SyncServer<Caller>,
    conns: HashMap<ConnId, Stream>,
    tx: mpsc::UnboundedSender<Msg>,
    snaps: Arc<dyn Snapshots>,
    backend: Arc<dyn Backend>,
    idem: Arc<Idempotency>,
    active: Arc<AtomicUsize>,
    order: Order,
}

impl Actor {
    pub(super) fn new(
        server: SyncServer<Caller>,
        tx: mpsc::UnboundedSender<Msg>,
        snaps: Arc<dyn Snapshots>,
        backend: Arc<dyn Backend>,
        active: Arc<AtomicUsize>,
    ) -> Self {
        Self { server, conns: HashMap::new(), tx, snaps, backend, idem: Arc::new(Idempotency::default()), active, order: Order::default() }
    }

    pub(super) async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        loop {
            let wake = self.server.next_wake_at().map(|at| at.saturating_sub(now_ms()));
            let msg = tokio::select! {
                m = rx.recv() => match m { Some(m) => Some(m), None => return },
                () = sleep(wake) => None,
            };
            match msg {
                Some(Msg::Open { conn, caller, query, tx, queued }) => {
                    self.conns.insert(conn, Stream { tx, caller: caller.clone(), queued, active: false });
                    self.feed(Input::Connect { conn, caller, query });
                }
                Some(Msg::Post { caller, client, body, reply }) => {
                    let _ = reply.send(self.post(&caller, &client, body));
                }
                Some(Msg::In(input)) => {
                    // Refresh socket backlog before handling snapshot pages, commits, or command results so stale
                    // `buffered` counts never hold back snapshots or trigger spurious high-water resyncs.
                    match &input {
                        Input::SnapshotRows(a) => self.refresh_one(a.conn),
                        Input::CommandResult { conn, .. } => self.refresh_one(*conn),
                        Input::Committed { .. } => self.refresh_all(),
                        _ => {}
                    }
                    self.feed(input);
                }
                Some(Msg::Gone(conn)) => {
                    if self.conns.contains_key(&conn) {
                        self.close(conn);
                        self.feed(Input::Close { conn });
                    }
                }
                None => {
                    // the machine ticks with each socket's backlog as it is now: pages held past the high-water mark
                    // go once it drained, though nothing was written since the last report (ADR-0081)
                    self.refresh_all();
                    self.feed(Input::Tick);
                }
            }
            self.reap();
        }
    }

    fn post(&mut self, caller: &Caller, client: &str, message: Value) -> Result<()> {
        let conn = self
            .server
            .conn_of(client)
            .ok_or_else(|| WireError::not_found("no open stream for this client: open GET /api/events first"))?;
        // a stream's messages come from whoever opened it, never another caller
        let owner = self.conns.get(&conn).map(|s| &s.caller);
        if owner.is_none_or(|c| c.set != caller.set || c.session != caller.session) {
            return Err(WireError::forbidden("that stream belongs to another caller"));
        }
        self.refresh_one(conn);
        self.feed(Input::Message { conn, message });
        Ok(())
    }

    fn refresh_one(&mut self, conn: ConnId) {
        if let Some(s) = self.conns.get(&conn) {
            self.feed(Input::Drained { conn, buffered_bytes: s.queued.load(Ordering::Relaxed) });
        }
    }

    fn refresh_all(&mut self) {
        let now: Vec<(ConnId, usize)> = self.conns.iter().map(|(c, s)| (*c, s.queued.load(Ordering::Relaxed))).collect();
        for (conn, buffered_bytes) in now {
            self.feed(Input::Drained { conn, buffered_bytes });
        }
    }

    /// Streams whose socket went away: closed in the machine too.
    fn reap(&mut self) {
        let gone: Vec<ConnId> = self.conns.iter().filter(|(_, s)| s.tx.is_closed()).map(|(c, _)| *c).collect();
        for c in gone {
            self.close(c);
            self.feed(Input::Close { conn: c });
        }
    }

    fn close(&mut self, conn: ConnId) {
        if let Some(s) = self.conns.remove(&conn)
            && s.active
        {
            self.active.fetch_sub(1, Ordering::Relaxed);
        }
    }

    fn feed(&mut self, first: Input<Caller>) {
        let mut inputs = vec![first];
        while let Some(input) = inputs.pop() {
            let mut sent: Vec<ConnId> = Vec::new();
            let mut dead: Vec<ConnId> = Vec::new();
            for out in self.server.handle(input, now_ms()) {
                self.carry(out, &mut sent, &mut dead);
            }
            dead.sort_unstable();
            dead.dedup();
            for conn in dead {
                inputs.push(Input::Close { conn });
            }
            sent.sort_unstable();
            sent.dedup();
            for conn in sent {
                if let Some(s) = self.conns.get(&conn) {
                    inputs.push(Input::Drained { conn, buffered_bytes: s.queued.load(Ordering::Relaxed) });
                }
            }
        }
    }

    fn carry(&mut self, out: Output<Caller>, sent: &mut Vec<ConnId>, dead: &mut Vec<ConnId>) {
        match out {
            Output::Send { conn, frame, .. } => {
                let ok = self.conns.get(&conn).is_some_and(|s| {
                    let len = frame.len();
                    s.queued.fetch_add(len, Ordering::Relaxed);
                    if s.tx.try_send(Bytes::from(frame)).is_ok() {
                        true
                    } else {
                        s.queued.fetch_sub(len, Ordering::Relaxed);
                        false
                    }
                });
                if ok {
                    sent.push(conn);
                } else {
                    self.close(conn);
                    dead.push(conn);
                }
            }
            Output::Activity { conn, active } => {
                if let Some(s) = self.conns.get_mut(&conn).filter(|s| s.active != active) {
                    s.active = active;
                    if active {
                        self.active.fetch_add(1, Ordering::Relaxed)
                    } else {
                        self.active.fetch_sub(1, Ordering::Relaxed)
                    };
                }
            }
            Output::Close { conn, .. } => self.close(conn),
            Output::SnapshotRequest { conn, topic, since, req } => {
                let (snaps, tx) = (self.snaps.clone(), self.tx.clone());
                // each page reaches the machine as it is read (ADR-0081), one answer per page
                let at = topic.clone();
                let page = Box::new(move |r: Read, more: bool| {
                    let full = r.full || since.is_none();
                    // a full page's rows are encoded here, where the page was read, never in this actor (perf audit
                    // 2026-11-19 #1: ten fresh clients' lists encoded in it held every other client's patches)
                    let text = (full && since.is_none()).then(|| serde_json::to_string(&r.rows).ok()).flatten();
                    let (rows, rows_json) = match text {
                        Some(t) => (Vec::new(), Some(std::sync::Arc::<str>::from(t))),
                        None => (r.rows, None),
                    };
                    let a = SnapshotAnswer { conn, topic: at.clone(), req, seq: r.seq, rows, full, deleted: r.deleted, more, rows_json };
                    let _ = tx.send(Msg::In(Input::SnapshotRows(a)));
                });
                tokio::spawn(async move { snaps.snapshot_pages(&topic, since, page).await });
            }
            Output::WindowRequest { conn, req, topic, before_seq, limit } => {
                let (snaps, tx) = (self.snaps.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let (rows, done) = snaps.window(&topic, before_seq, limit).await;
                    let _ = tx.send(Msg::In(Input::WindowRows { conn, req, rows, done }));
                });
            }
            Output::Command { conn, caller, mid, cmd, args, .. } => self.command(conn, caller, mid, cmd, args),
        }
    }

    /// Runs once per (caller, mid): a mutate retried over a flaky tunnel gets the first answer.
    fn command(&mut self, conn: ConnId, caller: Caller, mid: String, cmd: String, args: Value) {
        let (backend, idem, tx, snaps) = (self.backend.clone(), self.idem.clone(), self.tx.clone(), self.snaps.clone());
        let who = format!("{}|{}", caller.set, caller.session.as_deref().unwrap_or(""));
        // a read runs again on a retry; only a write's answer is kept (perf audit #1)
        let read = backend.read_only(&cmd);
        // a write waits for the one this client sent before it to the same plugin: taken here, in arrival order
        let turn = (!read).then(|| self.order.next(format!("{who}|{}", plugin_of(&cmd))));
        tokio::spawn(async move {
            let key = format!("{who}|{mid}");
            let _done = match turn {
                Some((before, done)) => {
                    before.wait().await;
                    Some(done)
                }
                None => None,
            };
            let run = async move { command_answer(backend.call(caller, &cmd, args).await, i64::try_from(snaps.head_seq()).unwrap_or(0)) };
            let (status, v) = if read { run.await } else { idem.once(key, run).await };
            let outcome = if status == 200 {
                CommandOutcome::Ok { seq: v["seq"].as_u64().unwrap_or(0), result: v.get("result").cloned().filter(|r| !r.is_null()) }
            } else {
                CommandOutcome::Err {
                    code: v["error"]["code"].as_str().unwrap_or("internal").to_owned(),
                    message: v["error"]["message"].as_str().unwrap_or("").to_owned(),
                    reason: v["error"]["reason"].as_str().map(str::to_owned),
                }
            };
            let _ = tx.send(Msg::In(Input::CommandResult { conn, mid, outcome }));
        });
    }
}

/// The plugin a command belongs to: `accounts` for `accounts.login.start`.
fn plugin_of(cmd: &str) -> &str {
    cmd.split_once('.').map_or(cmd, |(p, _)| p)
}

/// How long a write waits for the one before it: long enough for a burst (a reconnect sends every write it held
/// at once; in telemetry from alpha.15, 27 Nov: three account adds, cancels and removes in 56ms ran in any order), short
/// enough that a write that waits on the user or a run (`runtime.stop` winds the run down first) never holds the
/// next one for long, nor for ever if it waits on the next one.
const WAIT_BEFORE_MS: u64 = 5_000;

/// Writes from one client to one plugin, in the order they reached the server (the writes a page held while away are
/// user's actions in order: the page showed each at once). Writes to other plugins, and reads, don't wait.
#[derive(Default)]
struct Order {
    /// per client and plugin, the last write started: dropped (its sender) when it has answered
    last: HashMap<String, tokio::sync::oneshot::Receiver<()>>,
}

/// What a write waits on: the write before it (none: the first).
struct Before(Option<tokio::sync::oneshot::Receiver<()>>);

impl Before {
    async fn wait(self) {
        if let Some(rx) = self.0 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(WAIT_BEFORE_MS), rx).await;
        }
    }
}

impl Order {
    /// The write's place: what it waits on, and what it drops when it has answered.
    fn next(&mut self, key: String) -> (Before, tokio::sync::oneshot::Sender<()>) {
        if self.last.len() > 256 {
            // the ones that answered already: nothing waits on them
            self.last.retain(|_, rx| matches!(rx.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Empty)));
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        (Before(self.last.insert(key, rx)), tx)
    }
}

async fn sleep(ms: Option<u64>) {
    match ms {
        Some(ms) => tokio::time::sleep(std::time::Duration::from_millis(ms)).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod order_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::Mutex;

    /// A slow write then a fast one to the same plugin: the fast one starts after the slow one answered. Another
    /// plugin's write doesn't wait.
    #[tokio::test]
    async fn writes_to_one_plugin_run_in_order_others_dont_wait() {
        let mut o = Order::default();
        let log = Arc::new(Mutex::new(Vec::new()));
        let run = |(before, done): (Before, tokio::sync::oneshot::Sender<()>),
                   name: &'static str,
                   ms: u64,
                   log: Arc<Mutex<Vec<&'static str>>>| {
            tokio::spawn(async move {
                before.wait().await;
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                log.lock().unwrap().push(name);
                drop(done);
            })
        };
        let a = run(o.next("me|accounts".into()), "add", 80, log.clone());
        let b = run(o.next("me|accounts".into()), "remove", 0, log.clone());
        let c = run(o.next("me|runtime".into()), "other", 0, log.clone());
        for t in [a, b, c] {
            t.await.unwrap();
        }
        assert_eq!(*log.lock().unwrap(), ["other", "add", "remove"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_write_waits_on_the_one_before_it_five_seconds_at_most() {
        let mut o = Order::default();
        let (_b1, held) = o.next("me|runtime".into());
        let (b2, _d2) = o.next("me|runtime".into());
        let at = tokio::time::Instant::now();
        b2.wait().await;
        assert_eq!(at.elapsed().as_millis(), u128::from(WAIT_BEFORE_MS));
        drop(held);
    }

    #[test]
    fn a_command_belongs_to_the_plugin_before_its_first_dot() {
        assert_eq!(plugin_of("accounts.login.start"), "accounts");
        assert_eq!(plugin_of("ping"), "ping");
    }
}
