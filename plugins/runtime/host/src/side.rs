//! A run that is not a turn (v1 quick.js, btw.js): the harness's launch spawned through the proc
//! service, its output fed to the harness's driver, and the answer collected. Nothing is written to
//! the task's turns or activity (its steps come back with the answer); a timeout kills the process tree.

use std::time::Duration;

use tokio::sync::watch;

use wisp_proc::{Env, SpawnOpts, Stream};
use wisp_runtime_api::harness::{Exit, HarnessRun, Launch, ParserInput, ParserOutput, Stdin};
use wisp_runtime_api::model::Activity;
use wisp_runtime_api::{Harness, HarnessEvent};
use wisp_runtime_core::activity::{self, Effect, EnvValues, Head};

use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;
use crate::sys::ProcSys;

/// What a side run answered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SideOut {
    pub text: String,
    pub is_error: bool,
    pub cost_usd: Option<f64>,
    pub timed_out: bool,
    /// `runtime.quickCancel` stopped it
    pub cancelled: bool,
    /// its steps, as a turn's activity records them (tool calls, their output, texts), at most `MAX_STEPS`
    pub activity: Vec<Activity>,
}

/// Resolves once `stop` holds true; never without one (or once its sender is gone without saying).
async fn stopped(stop: Option<watch::Receiver<bool>>) {
    if let Some(mut rx) = stop
        && rx.wait_for(|s| *s).await.is_ok()
    {
        return;
    }
    std::future::pending::<()>().await;
}

/// The side run's CLI, killed (its whole tree) however its run ends: an early return, an error, or the run's future
/// dropped (a /btw's caller gone, a goal's judge cancelled). A side run's CLI never outlives its run.
struct KillOnDrop(wisp_proc::ChildHandle);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if self.0.running()
            && let Ok(rt) = tokio::runtime::Handle::try_current()
        {
            rt.spawn(self.0.kill());
        }
    }
}

/// The prompt on the CLI's stdin, when its launch asks for it there. One it can't take (it went before reading it) is
/// killed now, never left running with nobody reading it. One that already went (it answered without reading it, as
/// `side_runs`' `noeol` can on a busy runner) is no refusal: what it printed is its answer.
async fn send_prompt(handle: &wisp_proc::ChildHandle, stdin: &Stdin) -> Result<()> {
    let Stdin::Bytes(b) = stdin else { return Ok(()) };
    if let Err(e) = handle.write(b.clone()) {
        if matches!(e, wisp_proc::ProcError::Exited) {
            return Ok(());
        }
        let _ = handle.kill().await;
        return Err(e.into());
    }
    handle.close_stdin();
    Ok(())
}

/// One stdout line into the driver, its stdin writes out, its events into the answer.
async fn feed_line(
    driver: &mut dyn wisp_runtime_api::RunDriver,
    line: &[u8],
    now: i64,
    sys: &ProcSys,
    handle: &wisp_proc::ChildHandle,
    acc: (&mut SideOut, &mut Head, &mut bool),
) {
    let line = String::from_utf8_lossy(line).trim_end_matches('\r').to_owned();
    let mut out = ParserOutput::default();
    driver.on(ParserInput::Stdout(&line), now, sys, &mut out).await;
    for w in std::mem::take(&mut out.writes) {
        let _ = handle.write(w);
    }
    take(out, acc.0, acc.1, acc.2);
}

/// A side run keeps this many steps at most (a runaway judge can't grow one answer without bound).
const MAX_STEPS: usize = 500;

fn take(out: ParserOutput, side: &mut SideOut, head: &mut Head, done: &mut bool) {
    for e in out.events {
        if !*done
            && let Effect::Append(step) = activity::apply(head, &e, true, &|_| false)
            && side.activity.len() < MAX_STEPS
        {
            side.activity.push(step);
        }
        match e {
            HarnessEvent::Result { text, is_error, stats, .. } => {
                side.text = text;
                side.is_error = is_error;
                side.cost_usd = stats.and_then(|s| s.cost_usd);
                *done = true;
            }
            HarnessEvent::Text { text } if !*done => {
                side.text.push_str(&text);
            }
            _ => {}
        }
    }
}

impl Runtime {
    /// Runs `run` on `h` to its result, at most `timeout`; `stop` turning true (`runtime.quickCancel`) ends it early.
    /// Either way the process tree is killed (wisp-proc: its group on Unix, its job on Windows).
    pub(crate) async fn side_run(
        &self,
        h: &dyn Harness,
        run: &HarnessRun,
        timeout: Duration,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<SideOut> {
        // an id of its own (audit C18): a harness that names a new session up front by the turn's id (copilot's
        // `--session-id`) put every side run of a task in one session named by the task's id
        let own;
        let run = if run.turn_id.is_empty() {
            own = HarnessRun { turn_id: wisp_kernel::ids::uuid7(), ..run.clone() };
            &own
        } else {
            run
        };
        let launch = h.launch(run)?;
        // the files it asked for (codex's base instructions, grok's prompt file, copilot's usage file) in a folder of
        // its own, and `{tmp}` in its argv and env pointing there, as a turn's launch has them
        let tmp = crate::run::write_files(&format!("wisp-side-{}", wisp_kernel::ids::uuid7()), &launch).await?;
        // removed however this ends, a dropped side run too (ADR-0116)
        let _files = crate::run::LaunchDir::of(&launch, &tmp);
        self.side_spawned(h, run, &launch, &tmp, timeout, stop).await
    }

    async fn side_spawned(
        &self,
        h: &dyn Harness,
        run: &HarnessRun,
        launch: &Launch,
        tmp: &str,
        timeout: Duration,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<SideOut> {
        let fill = |x: &str| x.replace("{tmp}", tmp);
        let opts = SpawnOpts {
            // a /btw runs where its task does; a quick answer with no folder in one empty folder of its own
            cwd: Some(
                crate::run::run_folder(&launch.cwd, &run.cwd, if run.session_id.is_empty() { "quick" } else { &run.session_id }).await?,
            ),
            env: Env {
                clear: false,
                set: run
                    .env
                    .iter()
                    .chain(&run.account_env)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .chain(launch.env.iter().map(|(k, v)| (k.clone(), fill(v))))
                    .collect(),
                remove: launch.env_remove.clone(),
            },
            no_stdin: launch.stdin == Stdin::Null,
            // its tree goes with the plugin's scope (switching wisp.runtime off), as a turn's CLI does
            scope: self.host.scope(),
            ..SpawnOpts::default()
        };
        // a cancel that came before the spawn (a goal's judge dropped, its task deleted) spawns nothing; the stop's
        // value is held across the (sync) spawn, so a cancel lands before it or is seen by the loop below
        let child = {
            let held = stop.as_ref().map(watch::Receiver::borrow);
            if held.as_ref().is_some_and(|v| **v) {
                return Err(RuntimeError::Cancelled(wisp_runtime_api::commands::QUICK_CANCELLED.into()));
            }
            self.proc.spawn(&launch.program, &launch.args.iter().map(|a| fill(a)).collect::<Vec<_>>(), opts)?
        };

        let (handle, mut rx) = child.split();
        let _kill = KillOnDrop(handle.clone());
        send_prompt(&handle, &launch.stdin).await?;
        let sys = ProcSys::new(self.proc.clone());
        let mut driver = h.driver(run);
        // a /btw or a goal check on a task knows what the task's turns read (ADR-0050)
        let known = if run.session_id.is_empty() { Ok(EnvValues::default()) } else { self.env_known(&run.session_id).await };
        let known = known.unwrap_or_else(|e| {
            tracing::warn!(task = %run.session_id, error = %e, "runtime: the task's earlier .env reads are unknown to this side run");
            EnvValues::default()
        });
        let (mut side, mut head, mut done) = (SideOut::default(), Head::seeded(known), false);
        let (mut buf, mut stderr) = (Vec::<u8>::new(), String::new());
        // the deadline on the kernel's clock (a test's FakeClock drives it)
        let deadline = self.clock.sleep(u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
        tokio::pin!(deadline);
        let stopped = stopped(stop);
        tokio::pin!(stopped);
        loop {
            tokio::select! {
                o = rx.recv() => match o {
                    Some(o) if o.stream == Stream::Stderr => stderr.push_str(&String::from_utf8_lossy(&o.data)),
                    Some(o) => {
                        buf.extend_from_slice(&o.data);
                        while let Some(i) = buf.iter().position(|&b| b == b'\n') {
                            let line: Vec<u8> = buf.drain(..=i).collect();
                            feed_line(&mut *driver, &line[..line.len() - 1], self.now(), &sys, &handle, (&mut side, &mut head, &mut done)).await;
                        }
                        if done {
                            break;
                        }
                    }
                    None => {
                        // its last line, when the CLI ended without a newline after it (often its result)
                        if !buf.is_empty() && !done {
                            let line = std::mem::take(&mut buf);
                            feed_line(&mut *driver, &line, self.now(), &sys, &handle, (&mut side, &mut head, &mut done)).await;
                        }
                        break;
                    }
                },
                () = &mut deadline => {
                    side.timed_out = true;
                    break;
                }
                () = &mut stopped => {
                    side.cancelled = true;
                    break;
                }
            }
        }
        if handle.running() {
            let _ = handle.kill().await;
        }
        let exit = handle.exited().await;
        if side.cancelled {
            return Err(RuntimeError::Cancelled(wisp_runtime_api::commands::QUICK_CANCELLED.into()));
        }
        if !done && !side.timed_out {
            let x = Exit { code: exit.code, signal: exit.signal, stderr: stderr.trim().to_owned(), stopped: false, stop_requested: false };
            let mut out = ParserOutput::default();
            driver.on(ParserInput::Exit(&x), self.now(), &sys, &mut out).await;
            take(out, &mut side, &mut head, &mut done);
        }
        if side.timed_out {
            return Err(RuntimeError::Conflict(format!("It took over {} s and was stopped.", timeout.as_secs())));
        }
        Ok(side)
    }
}
