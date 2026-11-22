//! Driving a run (v1 runner.js `#startTurn`'s `onEvent`, `release`, `LINGER_MS`): output lines into the
//! harness's driver, its events into the turn's head and log. Activity is appended through the store's
//! Coalescer, one shared by every running turn (ADR-0083): all of them commit together, `ACTIVITY_WINDOW_MS` after
//! the first line buffered, or at once on a tool's result, a wait and the turn's end (plan §4.1). It is
//! published as one `turn.activity` per output chunk, never per token. A CLI may outlive its result: it gets `LINGER_MS`, then it is let go.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use wisp_proc::{ChildHandle, Output, Stream};
use wisp_runtime_api::RunDriver;
use wisp_runtime_api::events::{TurnActivity, TurnLimits};
use wisp_runtime_api::harness::{Exit, ParserInput, ParserOutput, StopHow};
use wisp_runtime_api::model::{Activity, Step};
use wisp_runtime_core::activity::{self, Effect, Head};
use wisp_store::{Coalescer, CoalescerOptions};

use crate::host::Emitted;
use crate::rows;
use crate::run::Ctl;
use crate::runtime::Runtime;
use crate::sys::ProcSys;

/// How long a CLI may keep running after it reported its result (v1 `LINGER_MS`).
pub const LINGER: Duration = Duration::from_secs(3);
/// stderr kept for an exit without a result.
const STDERR_TAIL: usize = 8000;
/// `turn.activity` goes out at most this often (and at the turn's end): never once per line or token.
const PUBLISH_EVERY: Duration = Duration::from_millis(100);
/// The shared coalescer's window (ADR-0083): a line waits at most this long for its commit. With every running
/// turn in one commit, 10 commits a second keep the bytes written under the 15× budget at any number of tasks.
pub const ACTIVITY_WINDOW_MS: u64 = 100;
/// The list row's live line ("Reading src/a.ts") is rewritten at most this often while a turn runs: its first step at
/// once, then the latest one a second (each is a write of the task's row, so never once per step).
const ROW_EVERY: Duration = Duration::from_secs(1);

pub(crate) struct Drive {
    pub sid: String,
    pub tid: String,
    pub head: Head,
    pub seq: u64,
    pub coal: Coalescer,
    pub finished: bool,
    /// steps appended but not yet published, and when the last batch went
    pending: Vec<Step>,
    published: Instant,
    out: Vec<u8>,
    err: Vec<u8>,
    stderr: String,
    /// the live line the task's row was last written with, and when: a newer step rewrites it (`ROW_EVERY`)
    row_label: Option<String>,
    row_at: Instant,
}

fn take_lines(buf: &mut Vec<u8>, data: &[u8], last: bool) -> Vec<String> {
    buf.extend_from_slice(data);
    let mut lines = Vec::new();
    let mut start = 0;
    while let Some(i) = buf[start..].iter().position(|&b| b == b'\n') {
        let end = start + i;
        let line = buf[start..end].strip_suffix(b"\r").unwrap_or(&buf[start..end]);
        lines.push(String::from_utf8_lossy(line).into_owned());
        start = end + 1;
    }
    buf.drain(..start);
    if last && !buf.is_empty() {
        lines.push(String::from_utf8_lossy(buf).into_owned());
        buf.clear();
    }
    lines
}

impl Runtime {
    pub(crate) async fn drive(
        self: Arc<Self>,
        sid: String,
        tid: String,
        mut driver: Box<dyn RunDriver>,
        handle: ChildHandle,
        mut rx: mpsc::Receiver<Output>,
        mut ctl: mpsc::UnboundedReceiver<Ctl>,
    ) {
        let sys = ProcSys::new(self.proc.clone());
        let seq = self.repo.next_seq(&sid).await.unwrap_or(1);
        // the .env values the task's earlier turns read (ADR-0050); a store that can't say is read again next turn
        let known = self.env_known(&sid).await.unwrap_or_else(|e| {
            tracing::warn!(task = %sid, error = %e, "runtime: the task's earlier .env reads are unknown to this turn");
            wisp_runtime_core::activity::EnvValues::default()
        });
        let mut d = Drive {
            sid,
            tid,
            head: Head::seeded(known),
            seq,
            coal: self.coalescer(),
            finished: false,
            pending: Vec::new(),
            published: Instant::now(),
            out: Vec::new(),
            err: Vec::new(),
            stderr: String::new(),
            row_label: None,
            row_at: Instant::now(),
        };
        let mut linger: Option<Instant> = None;
        loop {
            tokio::select! {
                o = rx.recv() => match o {
                    Some(o) => {
                        let (buf, stderr) = if o.stream == Stream::Stderr { (&mut d.err, true) } else { (&mut d.out, false) };
                        for line in take_lines(buf, &o.data, false) {
                            let input = if stderr { ParserInput::Stderr(&line) } else { ParserInput::Stdout(&line) };
                            if stderr {
                                d.stderr.push_str(&line);
                                d.stderr.push('\n');
                            }
                            let out = self.feed(&mut *driver, input, &sys).await;
                            if self.apply(&mut d, out, &handle).await && linger.is_none() {
                                linger = Some(Instant::now() + LINGER);
                            }
                        }
                    }
                    None => break,
                },
                c = ctl.recv() => match c {
                    Some(Ctl::Stop | Ctl::Interrupt) => {
                        let out = self.feed(&mut *driver, ParserInput::Stop, &sys).await;
                        let asked = out.stop.is_some();
                        self.apply(&mut d, out, &handle).await;
                        if !asked {
                            let _ = handle.kill().await;
                        }
                    }
                    Some(Ctl::Release) if d.finished => break,
                    Some(Ctl::Release) | None => {}
                },
                () = tokio::time::sleep_until(linger.unwrap_or_else(Instant::now)), if linger.is_some() => break,
                () = tokio::time::sleep_until(d.published + PUBLISH_EVERY), if !d.pending.is_empty() => self.publish(&mut d, true),
                () = tokio::time::sleep_until(d.row_at + ROW_EVERY), if !d.finished && self.row_stale(&d) => self.refresh_row(&mut d),
            }
            // the first step shows at once: until then the row says Starting up
            if d.row_label.is_none() && !d.finished && self.row_stale(&d) {
                self.refresh_row(&mut d);
            }
        }
        self.wind_down(d, &mut *driver, &handle, &sys).await;
    }

    /// The process is gone or let go: its last words, then the turn ends (if it hasn't) and the run is
    /// released, which starts the next queued follow-up.
    async fn wind_down(self: &Arc<Self>, mut d: Drive, driver: &mut dyn RunDriver, handle: &ChildHandle, sys: &ProcSys) {
        for line in take_lines(&mut d.out, &[], true) {
            let out = self.feed(driver, ParserInput::Stdout(&line), sys).await;
            self.apply(&mut d, out, handle).await;
        }
        if d.finished && handle.running() {
            let _ = handle.kill().await;
        }
        let exit = handle.exited().await;
        let stopping =
            self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&d.sid).is_some_and(|l| l.stopping || l.interrupting);
        let tail: String = d.stderr.chars().rev().take(STDERR_TAIL).collect::<Vec<_>>().into_iter().rev().collect();
        let exit =
            Exit { code: exit.code, signal: exit.signal, stderr: tail.trim().to_owned(), stopped: stopping, stop_requested: stopping };
        if !d.finished {
            let out = self.feed(driver, ParserInput::Exit(&exit), sys).await;
            self.apply(&mut d, out, handle).await;
        }
        if !d.finished {
            let effect = if exit.stderr.is_empty() || stopping {
                activity::exit_without_result(&mut d.head)
            } else {
                Effect::Finish {
                    text: exit.stderr.clone(),
                    is_error: true,
                    stopped: false,
                    stats: None,
                    auth_failed: false,
                    stop_reason: String::new(),
                }
            };
            self.finish(&mut d, effect).await;
        }
        self.publish(&mut d, true);
        // the shared buffer stays open for the other turns: this one's last lines commit now
        d.coal.flush().await;
        self.env_remember(&d.sid, &d.head.env_values);
        self.release_run(&d.sid).await;
    }

    async fn feed(&self, driver: &mut dyn RunDriver, input: ParserInput<'_>, sys: &ProcSys) -> ParserOutput {
        let mut out = ParserOutput::default();
        driver.on(input, self.now(), sys, &mut out).await;
        out
    }

    /// One driver answer: events into the head and log, stdin writes, a stop. true: the turn just ended.
    async fn apply(self: &Arc<Self>, d: &mut Drive, out: ParserOutput, handle: &ChildHandle) -> bool {
        for w in out.writes {
            let _ = handle.write(w);
        }
        if out.close_stdin {
            handle.close_stdin();
        }
        match out.stop {
            Some(StopHow::Interrupt) => {
                let _ = handle.signal(wisp_proc::Signal::Int, true);
            }
            Some(StopHow::Terminate) => {
                let _ = handle.kill().await;
            }
            None => {}
        }
        let fallback = |t: &str| wisp_runtime_core::blocks::parse(t).fallback;
        let mut ended = false;
        let mut now = false;
        for ev in &out.events {
            if d.finished {
                break;
            }
            match activity::apply(&mut d.head, ev, false, &fallback) {
                Effect::Append(a) => {
                    // a tool's result is something to read now (and the next step waits on it): no window
                    now |= matches!(a, Activity::ToolOutput { .. } | Activity::ToolError { .. } | Activity::Tool { output: Some(_), .. });
                    let step = self.append(d, a);
                    d.pending.push(step);
                }
                Effect::Waiting(a) => {
                    now = true;
                    let step = self.append(d, a);
                    d.pending.push(step);
                }
                Effect::Limits(limits) => self.limits(d, limits),
                e @ Effect::Finish { .. } => {
                    self.publish(d, true);
                    self.finish(d, e).await;
                    ended = true;
                }
                Effect::Billing(_) | Effect::None => {}
            }
            if let Some(id) = d.head.resume_id.clone() {
                self.resumable(&d.sid, &id);
            }
        }
        if now && !d.finished {
            d.coal.step().await;
        }
        self.publish(d, false);
        ended
    }

    /// The runtime's one activity buffer, made on first use (inside the server's tokio runtime, for its timer).
    pub(crate) fn coalescer(&self) -> Coalescer {
        self.coal
            .get_or_init(|| {
                Coalescer::new(&self.repo.store, CoalescerOptions { interval_ms: ACTIVITY_WINDOW_MS, ..CoalescerOptions::default() })
            })
            .clone()
    }

    fn append(&self, d: &mut Drive, a: Activity) -> Step {
        let t = self.now();
        let step = Step { seq: self.alloc_seq_now(&d.sid, d.seq), t, activity: a };
        d.seq = step.seq + 1;
        if let Ok(row) = rows::event_row(&d.sid, &d.tid, step.seq, t, &step.activity) {
            let _ = d.coal.append("event", vec![row]);
        }
        if let Activity::Tool { label, hide, .. } = &step.activity
            && let Some(l) = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_mut(&d.sid)
        {
            // the list row's live line: never a .env value it quotes (ADR-0050)
            l.label = Some(hide.get("label").map_or_else(|| label.clone(), |h| wisp_envmask::mask_spans(label, h)));
        }
        step
    }

    /// The run's live line differs from what its row was last written with.
    fn row_stale(&self, d: &Drive) -> bool {
        let l = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&d.sid).and_then(|l| l.label.clone());
        l.is_some() && l != d.row_label
    }

    /// Rewrites the task's row with the run's live line (the list says what the agent does, not Starting up for the
    /// whole turn). Off the drive's loop: it takes the task's lock, and output keeps being read meanwhile.
    fn refresh_row(self: &Arc<Self>, d: &mut Drive) {
        d.row_label = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&d.sid).and_then(|l| l.label.clone());
        d.row_at = Instant::now();
        let (rt, sid) = (self.clone(), d.sid.clone());
        tokio::spawn(async move {
            let _guard = rt.task_lock(&sid).await;
            // only a task still running: the turn's end writes its own row
            let Ok(mut s) = rt.repo.session(&sid).await else { return };
            if s.state != wisp_runtime_api::model::SessionState::Running || rt.live_label(&sid).is_none() {
                return;
            }
            if let Err(e) = rt.persist(&mut s, Vec::new()).await {
                tracing::warn!(task = %sid, error = %e, "runtime: the row's live line wasn't written");
            }
        });
    }

    /// Publishes the pending steps when `now`, or when the last batch is old enough.
    fn publish(&self, d: &mut Drive, now: bool) {
        if d.pending.is_empty() || (!now && d.published.elapsed() < PUBLISH_EVERY) {
            return;
        }
        d.published = Instant::now();
        // what the turn read so far, known to the task now: a /btw or a row written mid-turn masks it too (ADR-0050)
        self.env_remember(&d.sid, &d.head.env_values);
        let items = std::mem::take(&mut d.pending);
        self.host.emit(Emitted::Activity(TurnActivity { session_id: d.sid.as_str().into(), turn_id: d.tid.as_str().into(), items }));
    }

    fn limits(&self, d: &Drive, limits: serde_json::Value) {
        let agent = self.lock_list().get(&d.sid).map(|s| s.agent.clone()).unwrap_or_default();
        // the run's account: a second account's windows are its own, never the CLI's own login's
        let account = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&d.sid).and_then(|l| l.account.clone());
        self.host.emit(Emitted::Limits(TurnLimits {
            session_id: d.sid.as_str().into(),
            turn_id: d.tid.as_str().into(),
            agent,
            account,
            limits,
        }));
    }

    fn resumable(&self, sid: &str, _id: &str) {
        if let Some(l) = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_mut(sid) {
            l.resumable = true;
            if !l.stopping
                && !l.interrupting
                && !l.finished
                && self.pending_interrupt.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(sid)
            {
                l.interrupting = true;
                let _ = l.ctl.send(Ctl::Interrupt);
            }
        }
    }
}
