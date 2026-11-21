//! This Mac's process view (v1 Machine.snapshot, routes/machine.js): every tick that something runs, a run
//! ended in the last 3 minutes, or the page was read in the last 30 s (and once a minute otherwise), one
//! `ps` (macOS with `-E`, so each process's `WISP_TASK` tag shows; Linux reads `/proc/<pid>/environ` once
//! per process), attributed by `wisp_machine_core::this_mac::attribute`, summed per task, with the
//! leftovers, Clean up's groups and a day of 1-minute points. Two rows carry it, so clients watch instead of
//! polling: `machine.snapshot` (the whole reading, topic `plugin:wisp.machine:snapshot`) and
//! `machine.load` (the chip's cheap one, `plugin:wisp.machine:load`), both id `this`.
//!
//! The runs: `machine.runs@1` contributors, else each running task's harness CLI is the process wisp
//! started whose tag names it.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use wisp_kernel::{Call, Ctx, Result, Setup, WireError};
use wisp_machine_api::this_mac::commands::{self as c, Cleaned, FixFiled, Stopped};
use wisp_machine_api::this_mac::{FACTS, FIXER, HARNESS_DISK, HistoryPoint, LeftoverGroup, LiveRun, MachineView, MemoryPer, RUNS, WispUse};
use wisp_machine_core::this_mac::attribute::{Attributor, Placed};
use wisp_machine_core::this_mac::history::History;
use wisp_machine_core::this_mac::load::Load;
use wisp_machine_core::this_mac::view::{self, ViewInput};
use wisp_machine_core::this_mac::{AFTER_MS, KILL_MS, POINT_MS, Platform, READ_MS, Refusal, RefusalKind, TICK_MS, holds, parse};
use wisp_runtime_api::events::{SessionChanged, SessionCreated, SessionDeleted};
use wisp_store::Store;

use crate::calls;
use crate::rows::{LOAD, SNAPSHOT, platform_name, rows};
use crate::this_mac::{Guard, lock, run, run_any};

/// Sampling when nothing runs and nobody reads the page: once a minute (the history's pace).
const IDLE_MS: i64 = 60_000;

/// The last view, the pids Stop may take, and what started wisp.
type Last = (MachineView, HashSet<u32>, HashSet<u32>);

/// What sampling keeps between ticks.
pub struct Procs {
    attr: Mutex<Attributor>,
    history: Mutex<History>,
    /// the last view, and the pids Stop may take (leftovers and the runs' trees)
    last: Mutex<Option<Last>>,
    read_at: Mutex<i64>,
    sampled_at: Mutex<i64>,
    /// when `last` was made: a `machine.read` within `TICK_MS` of it answers it (perf audit 2026-11-19 #3)
    last_at: Mutex<i64>,
    point_at: Mutex<i64>,
    ran_at: Mutex<i64>,
    /// the tasks a sample names, kept current from runtime's events (never every task each sample)
    index: crate::index::TaskIndex,
}

impl std::fmt::Debug for Procs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Procs").finish_non_exhaustive()
    }
}

fn this_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

fn refusal(r: Refusal) -> WireError {
    match r.kind {
        RefusalKind::Invalid => WireError::invalid(r.message),
        RefusalKind::Forbidden => WireError::forbidden(r.message),
        RefusalKind::NotFound => WireError::not_found(r.message),
        RefusalKind::Conflict => WireError::conflict(r.message),
    }
}

/// The runs going now: the contributors' (runtime), else for each running task the process wisp itself
/// started whose `WISP_TASK` names it (the harness CLI the runner spawned): in its `ps` line (Linux), or in its
/// environment as read once (`env_task`: macOS reads each process's with one `ps -E`; audit I IS12: a run whose CLI hadn't
/// said anything yet was never listed, its line carrying no environment).
pub fn live_runs(
    contributed: Vec<LiveRun>,
    procs: &[parse::Proc],
    me: u32,
    running: &HashSet<String>,
    env_task: &dyn Fn(u32) -> Option<String>,
) -> Vec<LiveRun> {
    if !contributed.is_empty() {
        return contributed;
    }
    let mut out: Vec<LiveRun> = Vec::new();
    for p in procs.iter().filter(|p| p.ppid == me) {
        if let Some(t) = p.task.clone().or_else(|| env_task(p.pid)).filter(|t| running.contains(t))
            && !out.iter().any(|r| r.task == t)
        {
            out.push(LiveRun { task: t, pid: p.pid });
        }
    }
    out
}

/// Windows' table as `ps` would give it, without what Toolhelp doesn't say (memory, CPU, start, command): pids and parents.
fn windows_table(t: &[wisp_proc::ProcInfo]) -> Vec<parse::Proc> {
    t.iter()
        .filter(|p| !p.zombie)
        .map(|p| parse::Proc { pid: p.pid, ppid: p.ppid, rss: 0, cpu: 0.0, since: 0, command: String::new(), task: None, time: 0 })
        .collect()
}

impl Procs {
    fn new(home: &str) -> Self {
        Self {
            attr: Mutex::new(Attributor::new(this_platform(), std::process::id(), home)),
            history: Mutex::new(History::default()),
            last: Mutex::new(None),
            read_at: Mutex::new(0),
            sampled_at: Mutex::new(0),
            last_at: Mutex::new(i64::MIN),
            point_at: Mutex::new(0),
            ran_at: Mutex::new(0),
            index: crate::index::TaskIndex::default(),
        }
    }

    /// Samples when it's due (see the module's doc), then writes the rows.
    pub async fn tick(&self, ctx: &Ctx, g: &Guard, proc: &wisp_proc::Proc, now: i64) {
        let runs = g.with(Load::reading).runs;
        if runs > 0 {
            *lock(&self.ran_at) = now;
        }
        let busy = runs > 0 || now - *lock(&self.ran_at) < AFTER_MS || now - *lock(&self.read_at) < READ_MS;
        if !busy && now - *lock(&self.sampled_at) < IDLE_MS {
            return;
        }
        *lock(&self.sampled_at) = now;
        let v = self.sample(ctx, g, proc, now).await;
        self.publish(ctx, g, &v, now).await;
    }

    async fn ps(proc: &wisp_proc::Proc, platform: Platform, now: i64) -> Vec<parse::Proc> {
        match platform {
            // without -E: the environments of only the pids not seen before come after (`environs`)
            Platform::Mac => run(proc, "ps", &["-axww", "-o", "pid=,ppid=,rss=,%cpu=,etime=,command="])
                .await
                .map(|t| parse::parse_ps(&t, now, false))
                .unwrap_or_default(),
            Platform::Linux => run(proc, "ps", &["-axww", "-o", "pid=,ppid=,rss=,%cpu=,etime=,command="])
                .await
                .map(|t| parse::parse_ps(&t, now, false))
                .unwrap_or_default(),
            // no `ps`: the process table's pids and parents (Toolhelp, through wisp-proc), so a run's tree is placed from
            // the pid wisp spawned (audit I IS12); memory and CPU aren't read here yet (v1's Win32_Process)
            Platform::Windows => tokio::task::spawn_blocking(wisp_proc::processes)
                .await
                .ok()
                .and_then(std::result::Result::ok)
                .map(|t| windows_table(&t))
                .unwrap_or_default(),
        }
    }

    /// macOS: the task tags in the environments of `wanted` (perf audit 2026-11-19 #3: `ps -E` of every process at every
    /// sample printed 1.2 MB for 600 processes; a process's environment is read once).
    async fn environs(&self, proc: &wisp_proc::Proc, procs: &[parse::Proc], wanted: &[u32]) {
        for chunk in wanted.chunks(512) {
            let list = chunk.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
            // a pid gone meanwhile makes ps exit 1 with the others printed: read what it printed either way
            let text = run_any(proc, "ps", &["-E", "-ww", "-o", "pid=,command=", "-p", &list]).await.unwrap_or_default();
            let tags: HashMap<u32, Option<String>> = parse::parse_env_ps(&text).into_iter().collect();
            let since: HashMap<u32, i64> = procs.iter().map(|p| (p.pid, p.since)).collect();
            let mut a = lock(&self.attr);
            for pid in chunk {
                if let Some(s) = since.get(pid) {
                    a.set_environ(*pid, *s, tags.get(pid).cloned().flatten());
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)] // one pass: ps, attribution, the tables, the view
    async fn sample(&self, ctx: &Ctx, g: &Guard, proc: &wisp_proc::Proc, now: i64) -> MachineView {
        let call = ctx.root_call();
        let platform = this_platform();
        let me = std::process::id();
        let mut procs = Self::ps(proc, platform, now).await;
        // each new process's environment, once: macOS in one `ps -E` for just those pids
        let wanted = lock(&self.attr).environ_wanted(&procs);
        if platform == Platform::Mac {
            self.environs(proc, &procs, &wanted).await;
        }
        for pid in wanted.into_iter().take(256).filter(|_| platform == Platform::Linux) {
            let text =
                tokio::fs::read(format!("/proc/{pid}/environ")).await.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            if let Some(p) = procs.iter().find(|p| p.pid == pid) {
                let since = p.since;
                lock(&self.attr).set_environ(pid, since, parse::tag_in_environ(&text));
            }
        }
        let crate::index::Known { mut tasks, running } = self.index.current(ctx, &call).await;
        let contributed: Vec<LiveRun> = ctx.contributions(&RUNS).iter().flat_map(|r| r.live()).collect();
        if platform == Platform::Mac {
            let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
            let got = tokio::task::spawn_blocking(move || wisp_proc::footprints(&pids)).await.unwrap_or_default();
            for p in &mut procs {
                if let Some(b) = got.get(&p.pid) {
                    p.rss = *b;
                }
            }
        }
        let runs = {
            let a = lock(&self.attr);
            live_runs(contributed, &procs, me, &running, &|pid| a.environ_task(pid))
        };
        let known = |t: &str| tasks.contains_key(t);
        let (placed, leftovers, groups): (Placed, _, _) = {
            let mut a = lock(&self.attr);
            let placed = a.place(procs, &runs, &known);
            let (l, gr) = a.leftovers(&placed, &known, now);
            (placed, l, gr)
        };
        let usage = view::rows(&placed);
        let wisp = placed.get(me).map_or(WispUse { rss: 0, cpu: None }, |p| WispUse { rss: p.rss, cpu: Some(p.cpu) });
        let wt = calls::worktrees(ctx, &call).await;
        // the facts other plugins add, for the tasks the page names
        let named: HashSet<String> = usage
            .keys()
            .cloned()
            .chain(leftovers.iter().filter_map(|l| l.task.clone()))
            .chain(wt.iter().flat_map(|w| w.items.iter().map(|i| i.id.clone())))
            .collect();
        let facts = ctx.contributions(&FACTS);
        if !facts.is_empty() {
            let ids: Vec<String> = named.into_iter().filter(|id| tasks.contains_key(id)).collect();
            for id in &ids {
                let session = match calls::session(ctx, &call, id).await {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(error = %err.message, task = %id, "machine: runtime.get for its facts");
                        continue;
                    }
                };
                if let Some(t) = tasks.get_mut(id) {
                    for f in &facts {
                        f.facts(&session, &mut t.facts);
                    }
                }
            }
        }
        let reclaim = view::reclaim(wt.as_ref(), &|id| tasks.get(id).cloned());
        let harness = ctx.contributions(&HARNESS_DISK).iter().find_map(|h| h.view());
        let reading = g.with(Load::reading);
        let hold = g.with(Load::hold);
        let knobs = lock(&g.knobs).clone();
        let paused_until = holds::paused_until(&knobs, now);
        let battery = *lock(&g.battery);
        let history = {
            let mut h = lock(&self.history);
            let mut at = lock(&self.point_at);
            if now - *at >= POINT_MS {
                *at = now;
                h.push(HistoryPoint {
                    at: now,
                    free_pct: reading.mem_free_pct,
                    cpu: reading.cpu,
                    runs: reading.runs,
                    held: hold.is_some(),
                    worktree_bytes: wt.as_ref().and_then(|w| w.size),
                });
            }
            h.points()
        };
        let stoppable: HashSet<u32> = placed.trees.values().flatten().copied().chain(leftovers.iter().map(|l| l.pid)).collect();
        let v = view::build(
            ViewInput {
                now,
                mem_total: *lock(&g.total),
                cores: u32::try_from(std::thread::available_parallelism().map_or(1, std::num::NonZero::get)).unwrap_or(1),
                memory: if platform == Platform::Mac { MemoryPer::Footprint } else { MemoryPer::Rss },
                reading: &reading,
                hold: hold.as_ref(),
                paused_until,
                battery,
                battery_hold: knobs.battery_hold.as_deref().and_then(|b| b.parse().ok()),
                rows: &usage,
                tasks: &tasks,
                worktrees: wt.as_ref(),
                harness,
                wisp,
                leftovers,
                groups,
                history,
            },
            reclaim,
        );
        *lock(&self.last) = Some((v.clone(), stoppable, placed.ancestors));
        *lock(&self.last_at) = now;
        v
    }

    /// `machine.read`: the last sample when it's under a tick old (the ticks run while someone reads the page), with
    /// the guard as it is now; else a new one (perf audit 2026-11-19 #3: each read waited ~0.1 s on `ps`).
    pub async fn read(&self, ctx: &Ctx, g: &Guard, proc: &wisp_proc::Proc, now: i64) -> MachineView {
        *lock(&self.read_at) = now;
        let fresh = now.saturating_sub(*lock(&self.last_at)) < TICK_MS;
        if let Some(mut v) = lock(&self.last).as_ref().filter(|_| fresh).map(|(v, _, _)| v.clone()) {
            v.guard.paused_until = holds::paused_until(&lock(&g.knobs), now);
            let hold = g.with(Load::hold);
            v.guard.held = hold.as_ref().map(|h| h.kind);
            v.guard.why = hold.map(|h| h.why);
            return v;
        }
        let v = self.sample(ctx, g, proc, now).await;
        self.publish(ctx, g, &v, now).await;
        v
    }

    /// The last view again, with the guard as it is now: a pause or a resume shows at once, not at the next sample
    /// (audit A F14: the pane kept "Running as usual" until a reload). Nothing before the first sample.
    pub async fn republish(&self, ctx: &Ctx, g: &Guard, now: i64) {
        let Some(mut v) = lock(&self.last).as_ref().map(|(v, _, _)| v.clone()) else { return };
        v.guard.paused_until = holds::paused_until(&lock(&g.knobs), now);
        let hold = g.with(Load::hold);
        v.guard.held = hold.as_ref().map(|h| h.kind);
        v.guard.why = hold.map(|h| h.why);
        self.publish(ctx, g, &v, now).await;
    }

    async fn publish(&self, ctx: &Ctx, g: &Guard, v: &MachineView, now: i64) {
        let Ok(store) = ctx.service::<Store>() else { return };
        let reading = g.with(Load::reading);
        let paused = holds::paused_until(&lock(&g.knobs), now);
        let cleanup = ctx.setting("worktrees.cleanup").and_then(|v| v.as_str().map(str::to_owned));
        let (snap, load) = rows(v, &reading, paused, cleanup.as_deref(), platform_name());
        if let Err(e) = store.write(move |w| w.upsert(SNAPSHOT.kind, snap).and_then(|_| w.upsert(LOAD.kind, load)).map(|_| ())).await {
            tracing::warn!(error = %e, "machine: writing the snapshot");
        }
    }

    fn group(&self, id: &str) -> Option<(LeftoverGroup, MachineView)> {
        let last = lock(&self.last);
        let (v, _, _) = last.as_ref()?;
        v.leftover_groups.iter().find(|g| g.id == id).cloned().map(|g| (g, v.clone()))
    }
}

/// Stop: SIGTERM, then SIGKILL `KILL_MS` on if it's still there. Only a leftover or a process in a running
/// task's tree, as the last sample placed it; never wisp or what started it.
fn stop(p: &Procs, pid: &Value) -> Result<Stopped> {
    let pid = holds::check_pid(pid).map_err(refusal)?;
    let (mine, ancestor, since) = {
        let last = lock(&p.last);
        last.as_ref().map_or((false, false, None), |(v, ok, anc)| {
            let n = u32::try_from(pid).unwrap_or(0);
            (ok.contains(&n), anc.contains(&n), v.leftovers.iter().find(|l| l.pid == n).map(|l| l.since))
        })
    };
    let pid = holds::check_stop(pid, std::process::id(), ancestor, mine).map_err(refusal)?;
    match wisp_proc::signal_pid(pid, wisp_proc::Signal::Term) {
        Ok(()) => {}
        Err(wisp_proc::ProcError::Exited) => return Ok(Stopped { stopped: 0, since }),
        Err(e) => return Err(WireError::internal(e.to_string())),
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(u64::try_from(KILL_MS).unwrap_or(5_000))).await;
        if wisp_proc::pid_alive(pid) {
            match wisp_proc::signal_pid(pid, wisp_proc::Signal::Kill) {
                Ok(()) | Err(wisp_proc::ProcError::Exited) => {}
                Err(e) => tracing::warn!(error = %e, pid, "machine: SIGKILL after SIGTERM"),
            }
        }
    });
    if let Some(l) = lock(&p.last).as_mut() {
        l.0.leftovers.retain(|x| x.pid != pid);
    }
    Ok(Stopped { stopped: 1, since })
}

/// File a fix: the crew's fixer takes it (`machine.fixer@1`), else a task in the project the processes
/// came from, with v1's prompt.
async fn fix(p: &Procs, ctx: &Ctx, call: &Call, group: &Value, now: i64) -> Result<FixFiled> {
    let id = group.as_str().unwrap_or_default();
    let Some((g, v)) = p.group(id) else {
        return Err(WireError::not_found("That group of processes is gone: look again."));
    };
    let task = match g.task.as_deref() {
        Some(t) => calls::session(ctx, call, t)
            .await
            .map(|s| calls::task_of_session(&s))
            .map_err(|err| tracing::warn!(error = %err.message, task = %t, "machine: runtime.get for a fix"))
            .ok(),
        None => None,
    };
    let project = view::project_of(task.as_ref());
    let prompt = view::fix_prompt(&g, task.as_ref(), project.as_deref(), &v.leftovers, now);
    for f in ctx.contributions(&FIXER) {
        if let Some(filed) = f.file(call, prompt.clone()).await? {
            return Ok(filed);
        }
    }
    let id = calls::create(ctx, call, prompt, format!("Fix: {} left running", g.command), project.as_deref()).await?;
    Ok(FixFiled { session: Some(id), ..FixFiled::default() })
}

pub(crate) fn setup(s: &mut Setup, g: &Arc<Guard>) -> Arc<Procs> {
    let home = std::env::var("WISP_HOME").ok().or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.wisp"))).unwrap_or_default();
    let p = Arc::new(Procs::new(&home));
    // the sample's tasks follow runtime's events (perf audit 2026-11-18 #4)
    let ip = p.clone();
    s.on::<SessionChanged, _, _>(move |_ctx, e| {
        ip.index.changed(e.id.as_str());
        async {}
    });
    let ip = p.clone();
    s.on::<SessionCreated, _, _>(move |_ctx, e| {
        ip.index.created(&e.session);
        async {}
    });
    let ip = p.clone();
    s.on::<SessionDeleted, _, _>(move |_ctx, e| {
        ip.index.deleted(e.id.as_str());
        async {}
    });
    let (rp, rg) = (p.clone(), g.clone());
    s.command::<c::Read, _, _>(move |ctx, _call, _| {
        let (p, g) = (rp.clone(), rg.clone());
        async move {
            let now = ctx.clock()?.now_ms();
            let proc = ctx.service::<wisp_proc::Proc>()?;
            Ok(p.read(&ctx, &g, &proc, now).await)
        }
    });
    let sp = p.clone();
    s.command::<c::Stop, _, _>(move |_ctx, _call, a| {
        let p = sp.clone();
        async move { stop(&p, &a.pid) }
    });
    let fp = p.clone();
    s.command::<c::Fix, _, _>(move |ctx, call, a| {
        let p = fp.clone();
        async move {
            let now = ctx.clock()?.now_ms();
            fix(&p, &ctx, &call, &a.group, now).await
        }
    });
    let hp = p.clone();
    s.command::<c::HarnessClean, _, _>(move |ctx, _call, a| {
        let p = hp.clone();
        async move {
            let commands: Vec<String> =
                lock(&p.last).as_ref().map(|(v, _, _)| v.leftovers.iter().map(|l| l.command.clone()).collect()).unwrap_or_default();
            let Some(h) = ctx.contributions(&HARNESS_DISK).into_iter().next() else { return Ok(Cleaned::default()) };
            h.clean(a.ids, commands).await
        }
    });
    p
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use serde_json::json;
    use wisp_machine_api::this_mac::{Limits, LoadReading};

    fn proc(pid: u32, ppid: u32, task: Option<&str>) -> parse::Proc {
        parse::Proc { pid, ppid, rss: 1, cpu: 0.0, since: 0, command: "claude".into(), task: task.map(str::to_owned), time: 0 }
    }

    /// Without a runtime contribution, a running task's run is the process wisp started whose tag names it;
    /// a tagged child of something else, or a task that isn't running, is no run.
    #[test]
    fn a_running_tasks_run_is_the_tagged_process_wisp_started() {
        let me = 100;
        let procs = vec![proc(200, me, Some("t1")), proc(201, 200, Some("t1")), proc(300, me, Some("t2")), proc(400, 1, Some("t1"))];
        let running: HashSet<String> = ["t1".to_owned()].into();
        let runs = live_runs(Vec::new(), &procs, me, &running, &|_| None);
        assert_eq!(runs, vec![LiveRun { task: "t1".into(), pid: 200 }]);
        let given = vec![LiveRun { task: "t9".into(), pid: 9 }];
        assert_eq!(live_runs(given.clone(), &procs, me, &running, &|_| None), given, "runtime's own answer wins");
    }

    /// Audit I IS12: on macOS a process's `ps` line has no environment (it's read once, with `ps -E`): a run whose CLI
    /// hadn't said anything yet was never listed. Its tag from the environment read counts as one in its line.
    #[test]
    fn a_run_tagged_only_in_its_environment_read_is_a_run_audit_i_is12() {
        let me = 100;
        let procs = vec![proc(200, me, None), proc(300, me, None)];
        let running: HashSet<String> = ["t1".to_owned()].into();
        let env = |pid: u32| (pid == 200).then(|| "t1".to_owned()).or_else(|| (pid == 300).then(|| "t2".to_owned()));
        assert_eq!(live_runs(Vec::new(), &procs, me, &running, &env), vec![LiveRun { task: "t1".into(), pid: 200 }]);
        assert!(live_runs(Vec::new(), &procs, me, &running, &|_| None).is_empty(), "before: none");
    }

    /// Audit I IS12, every OS: a run is placed by the pid wisp spawned (runtime's `machine.runs@1`), so it's listed from
    /// its spawn, before its CLI says anything and with no tag read from its environment: its whole tree where the table
    /// holds it, its root alone where there's no table (Windows before a read) or the CLI started after the read.
    #[test]
    fn a_run_wisp_spawned_is_listed_by_its_pid_on_every_os_audit_i_is12() {
        let me = 100;
        let runs = live_runs(vec![LiveRun { task: "t1".into(), pid: 200 }], &[], me, &HashSet::new(), &|_| None);
        let known = |t: &str| t == "t1";
        let mut a = Attributor::new(Platform::Windows, me, "C:\\Users\\u\\.wisp");
        let table = windows_table(&[
            wisp_proc::ProcInfo { pid: 200, ppid: me, pgid: 0, zombie: false, start: 0 },
            wisp_proc::ProcInfo { pid: 201, ppid: 200, pgid: 0, zombie: false, start: 0 },
            wisp_proc::ProcInfo { pid: 300, ppid: me, pgid: 0, zombie: false, start: 0 },
        ]);
        let placed = a.place(table, &runs, &known);
        assert_eq!(placed.trees.get("t1"), Some(&HashSet::from([200, 201])), "its tree, untagged");
        assert_eq!(view::rows(&placed).get("t1").map(|u| u.procs), Some(2));
        let placed = a.place(Vec::new(), &runs, &known);
        assert_eq!(placed.trees.get("t1"), Some(&HashSet::from([200])), "no table: its root");
        assert!(view::rows(&placed).contains_key("t1"), "listed all the same");
        let none = a.place(Vec::new(), &[LiveRun { task: "t1".into(), pid: 0 }], &known);
        assert!(none.trees.is_empty(), "a CLI not spawned yet (pid 0) has no process to list");
    }

    /// Stop takes only what the last sample placed as a task's (a leftover, a run's tree): never wisp,
    /// never what started it, never a stranger; a pid that isn't a number is refused before any signal.
    #[test]
    fn stop_refuses_wisp_its_parents_and_strangers() {
        let p = Procs::new("/h");
        let me = std::process::id();
        let reading = LoadReading { mem_free_pct: None, cpu: None, pressure: None, at: 0, runs: 0, limits: Limits::default(), held: None };
        let v = view::build(
            ViewInput {
                now: 0,
                mem_total: 0,
                cores: 1,
                memory: MemoryPer::Rss,
                reading: &reading,
                hold: None,
                paused_until: 0,
                battery: None,
                battery_hold: None,
                rows: &std::collections::BTreeMap::default(),
                tasks: &std::collections::HashMap::new(),
                worktrees: None,
                harness: None,
                wisp: WispUse { rss: 0, cpu: None },
                leftovers: Vec::new(),
                groups: Vec::new(),
                history: Vec::new(),
            },
            Vec::new(),
        );
        *lock(&p.last) = Some((v, HashSet::from([4_000_000]), HashSet::from([4_000_001])));
        assert!(stop(&p, &json!(me)).is_err(), "wisp itself");
        assert!(stop(&p, &json!(4_000_001)).is_err(), "what started wisp");
        assert!(stop(&p, &json!(4_000_002)).is_err(), "not a task's");
        assert!(stop(&p, &json!("abc")).is_err(), "not a pid");
        // a placed pid that already ended: nothing to stop, not an error
        assert_eq!(stop(&p, &json!(4_000_000)).unwrap().stopped, 0);
    }
}
