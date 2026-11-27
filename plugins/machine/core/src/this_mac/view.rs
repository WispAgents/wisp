//! What the page shows (v1 Machine.view, #rows, #settleRes, reclaim, fileFix's prompt), from the
//! placed snapshot, the guard and the tables the host looked up.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};
use wisp_machine_api::this_mac::{
    BatteryState, CpuInfo, CrewUse, DiskInfo, Facts, Guard, HistoryPoint, Hold, Leftover, LeftoverGroup, LoadReading, MachineView, MemInfo,
    MemoryPer, ReclaimGroup, Res, TaskUse, WispUse,
};

use super::attribute::Placed;
use super::parse::Battery;
use super::{RES_CPU, RES_MOVE, RES_RSS, round, round1};

/// A task, as This Mac needs it: from `runtime.get` and the `machine.facts@1` contributors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    /// `running`, `review`, `done`, `error`
    pub state: String,
    pub cwd: String,
    pub facts: Facts,
}

/// Settings → Worktrees' last look, as much of it as This Mac reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WtView {
    pub items: Vec<WtItem>,
    pub leftovers: Vec<WtLeftover>,
    pub size: Option<u64>,
    /// (free, total)
    pub disk: Option<(u64, u64)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WtItem {
    pub id: String,
    /// nothing keeps it (Settings → Worktrees' `idle`)
    pub idle: bool,
    pub size: Option<u64>,
    /// a task is working in it (its build output stays too)
    pub busy: bool,
    /// of `size`, the build output (ADR-0144)
    pub build: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WtLeftover {
    pub name: String,
    pub size: Option<u64>,
    /// files of its own nobody else has; `None`: unread
    pub only: Option<u32>,
    /// of `size`, the build output (ADR-0144)
    pub build: Option<u64>,
}

/// One running task's tree, summed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub rss: u64,
    pub cpu: f64,
    pub procs: u32,
}

/// Per running task: `{rss, cpu, procs}` over its tree.
pub fn rows(placed: &Placed) -> BTreeMap<String, Usage> {
    placed
        .trees
        .iter()
        .map(|(id, set)| {
            let ps: Vec<_> = set.iter().filter_map(|pid| placed.get(*pid)).collect();
            let u = Usage {
                rss: ps.iter().map(|p| p.rss).sum(),
                cpu: round1(ps.iter().map(|p| p.cpu).sum()),
                procs: u32::try_from(ps.len()).unwrap_or(u32::MAX),
            };
            (id.clone(), u)
        })
        .collect()
}

fn moved(a: f64, b: f64) -> bool {
    (a - b).abs() > a.max(1.0) * RES_MOVE
}

/// A heavy run's summary carries `res`; it changes only when it moved 20% (or went light, or ended).
/// Returns the tasks whose summary to send again.
#[allow(clippy::cast_precision_loss)] // bytes compared as a share
pub fn settle_res<S: std::hash::BuildHasher>(rows: &BTreeMap<String, Usage>, res: &mut HashMap<String, Res, S>) -> Vec<String> {
    let mut changed = Vec::new();
    for (id, r) in rows {
        let heavy = r.rss > RES_RSS || r.cpu > RES_CPU;
        match res.get(id) {
            Some(prev) if heavy && !moved(prev.rss as f64, r.rss as f64) && !moved(prev.cpu, r.cpu) => {
                continue;
            }
            _ if heavy => {
                res.insert(id.clone(), Res { rss: r.rss, cpu: r.cpu });
            }
            Some(_) => {
                res.remove(id);
            }
            None => continue,
        }
        changed.push(id.clone());
    }
    let gone: Vec<String> = res.keys().filter(|id| !rows.contains_key(*id)).cloned().collect();
    for id in gone {
        res.remove(&id);
        changed.push(id);
    }
    changed
}

/// Groups of folders Clean up can take, each a list for `POST /worktrees/clean`: the build output of every folder no
/// task is working in (ADR-0144: `target/`, `node_modules/`, … go, the folders and their changes stay; its `leftovers`
/// are the left-over folders' names), then merged or closed PRs, Done tasks, finished crew jobs, and leftover folders
/// that hold nothing of their own (only idle ones). The groups' bytes don't overlap: a folder's build output counts in
/// the first group only, so their sum is what all of them free.
pub fn reclaim(wt: Option<&WtView>, task: &dyn Fn(&str) -> Option<TaskInfo>) -> Vec<ReclaimGroup> {
    let Some(wt) = wt else { return Vec::new() };
    let mut build = (Vec::new(), Vec::new(), 0u64);
    // each folder on its own line of the Space tab, biggest first: the task's title, else the folder's name
    let mut folders: Vec<(u64, Value)> = Vec::new();
    for it in wt.items.iter().filter(|it| !it.busy && it.build.unwrap_or(0) > 0) {
        let bytes = it.build.unwrap_or(0);
        build.0.push(it.id.clone());
        build.2 += bytes;
        let title = task(&it.id).map(|s| s.title).filter(|t| !t.is_empty()).unwrap_or_else(|| it.id.clone());
        folders.push((bytes, serde_json::json!({ "id": it.id, "title": title, "bytes": bytes, "leftover": false })));
    }
    for l in wt.leftovers.iter().filter(|l| l.build.unwrap_or(0) > 0) {
        let bytes = l.build.unwrap_or(0);
        build.1.push(l.name.clone());
        build.2 += bytes;
        folders.push((bytes, serde_json::json!({ "id": l.name, "title": l.name, "bytes": bytes, "leftover": true, "only": l.only })));
    }
    folders.sort_by_key(|f| std::cmp::Reverse(f.0));
    let rest_item = |it: &WtItem| it.size.unwrap_or(0).saturating_sub(if it.busy { 0 } else { it.build.unwrap_or(0) });
    let mut groups: [(&str, Vec<String>, u64); 4] =
        [("merged", vec![], 0), ("done", vec![], 0), ("crew", vec![], 0), ("leftover", vec![], 0)];
    for it in wt.items.iter().filter(|it| it.idle) {
        let Some(s) = task(&it.id) else { continue };
        let i = if s.facts.pr_closed {
            0
        } else if s.facts.crew.is_some() && matches!(s.state.as_str(), "done" | "review") {
            2
        } else if s.state == "done" {
            1
        } else {
            continue;
        };
        groups[i].1.push(it.id.clone());
        groups[i].2 += rest_item(it);
    }
    // a leftover that holds work of its own, or wasn't read, is for Settings → Worktrees, by hand
    for l in wt.leftovers.iter().filter(|l| l.only == Some(0)) {
        groups[3].1.push(l.name.clone());
        groups[3].2 += l.size.unwrap_or(0).saturating_sub(l.build.unwrap_or(0));
    }
    let (ids, lefts, bytes) = build;
    let first = (!ids.is_empty() || !lefts.is_empty()).then(|| {
        let count = u32::try_from(ids.len() + lefts.len()).unwrap_or(u32::MAX);
        let mut extra = Map::new();
        extra.insert("leftovers".into(), Value::from(lefts));
        extra.insert("folders".into(), Value::Array(folders.into_iter().map(|(_, v)| v).collect()));
        ReclaimGroup { kind: "build".to_owned(), count, bytes, ids, extra }
    });
    first
        .into_iter()
        .chain(groups.into_iter().filter(|g| !g.1.is_empty()).map(|(kind, ids, bytes)| ReclaimGroup {
            kind: kind.to_owned(),
            count: u32::try_from(ids.len()).unwrap_or(u32::MAX),
            bytes,
            ids,
            extra: Map::new(),
        }))
        .collect()
}

/// What File a fix asks for, word for word v1's.
pub fn fix_prompt(g: &LeftoverGroup, task: Option<&TaskInfo>, project: Option<&str>, leftovers: &[Leftover], now: i64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    #[allow(clippy::cast_precision_loss)]
    let mins = |since: i64| format!("{} min", round((now - since) as f64 / 60_000.0, 0).unwrap_or(0.0).max(1.0));
    #[allow(clippy::cast_precision_loss)]
    let mb = |b: u64| format!("{} MB", round(b as f64 / MB, 0).unwrap_or(0.0));
    let n = g.count;
    let mut out = vec![
        format!(
            "A run left {n} process{} running after it ended: wisp found {} on this computer, and the user asked for a fix.",
            if n == 1 { "" } else { "es" },
            if n == 1 { "it" } else { "them" }
        ),
        String::new(),
        match task {
            Some(s) => {
                let title = if s.title.is_empty() { &s.id } else { &s.title };
                format!("From the task \"{title}\" ({}){}.", s.id, project.map(|p| format!(", in {p}")).unwrap_or_default())
            }
            None => "No task is known for them: their folder is under wisp’s worktrees.".to_owned(),
        },
        format!("Started by: {}", g.command),
        "Still running:".to_owned(),
    ];
    let rows = g.pids.iter().filter_map(|pid| leftovers.iter().find(|l| l.pid == *pid)).take(20);
    out.extend(rows.map(|l| format!("- pid {}, {}, up {}: {}", l.pid, mb(l.rss), mins(l.since), l.command)));
    out.push(String::new());
    out.push("Find what starts these (a test helper, a QA script, a dev server a script forgets) and make it kill what it spawns, even when the run is stopped or fails: spawn in a process group of its own (detached) and kill the group on exit, SIGINT and SIGTERM, and in the tests' after() hooks. Don't stop these processes yourself: the user does that from This Mac. Say which file and what you changed.".to_owned());
    out.join("\n")
}

/// Where a fix for `task`'s leftovers is filed: its project, else its folder.
pub fn project_of(task: Option<&TaskInfo>) -> Option<String> {
    let s = task?;
    s.facts.project.clone().filter(|p| !p.is_empty()).or_else(|| (!s.cwd.is_empty()).then(|| s.cwd.clone()))
}

/// Everything `GET /machine` is made of.
#[derive(Debug, Clone)]
pub struct ViewInput<'a> {
    pub now: i64,
    pub mem_total: u64,
    pub cores: u32,
    pub memory: MemoryPer,
    pub reading: &'a LoadReading,
    /// the guard's hold, the user's and the power's included
    pub hold: Option<&'a Hold>,
    pub paused_until: i64,
    pub battery: Option<Battery>,
    pub battery_hold: Option<u32>,
    pub rows: &'a BTreeMap<String, Usage>,
    pub tasks: &'a HashMap<String, TaskInfo>,
    pub worktrees: Option<&'a WtView>,
    /// the harnesses' own reclaim group
    pub harness: Option<ReclaimGroup>,
    pub wisp: WispUse,
    pub leftovers: Vec<Leftover>,
    pub groups: Vec<LeftoverGroup>,
    pub history: Vec<HistoryPoint>,
}

fn tasks_and_crew(v: &ViewInput<'_>) -> (Vec<TaskUse>, Vec<CrewUse>) {
    let sizes: HashMap<&str, Option<u64>> =
        v.worktrees.map(|w| w.items.iter().map(|x| (x.id.as_str(), x.size)).collect()).unwrap_or_default();
    let mut tasks: Vec<TaskUse> = v
        .rows
        .iter()
        .map(|(id, x)| {
            let s = v.tasks.get(id);
            TaskUse {
                id: id.clone(),
                title: s.map(|s| s.title.clone()).unwrap_or_default(),
                crew: s.and_then(|s| s.facts.crew.clone()),
                rss: x.rss,
                cpu: x.cpu,
                procs: x.procs,
                worktree_bytes: sizes.get(id.as_str()).copied().flatten(),
            }
        })
        .collect();
    tasks.sort_by_key(|t| std::cmp::Reverse(t.rss));
    let mut crew: Vec<CrewUse> = Vec::new();
    for t in &tasks {
        let Some(c) = &t.crew else { continue };
        let i = crew.iter().position(|x| &x.id == c).unwrap_or_else(|| {
            let name = v.tasks.get(&t.id).and_then(|s| s.facts.crew_name.clone()).unwrap_or_default();
            crew.push(CrewUse { id: c.clone(), name, rss: 0, cpu: 0.0, jobs: 0 });
            crew.len() - 1
        });
        if let Some(x) = crew.get_mut(i) {
            x.rss += t.rss;
            x.cpu = round1(x.cpu + t.cpu);
            x.jobs += 1;
        }
    }
    crew.sort_by_key(|c| std::cmp::Reverse(c.rss));
    (tasks, crew)
}

/// `GET /machine`, from `v` and the reclaim groups (worktrees' then the harnesses').
pub fn build(v: ViewInput<'_>, mut reclaim: Vec<ReclaimGroup>) -> MachineView {
    let (tasks, crew) = tasks_and_crew(&v);
    if let Some(h) = v.harness.clone() {
        reclaim.push(h);
    }
    let r = v.reading;
    let wt = v.worktrees;
    MachineView {
        at: v.now,
        mem: MemInfo { total: v.mem_total, free_pct: r.mem_free_pct.and_then(|n| round(n, 1)), pressure: r.pressure, per: v.memory },
        cpu: CpuInfo { pct: r.cpu.and_then(|n| round(n, 1)), cores: v.cores },
        disk: DiskInfo {
            free: wt.and_then(|w| w.disk).map(|d| d.0),
            total: wt.and_then(|w| w.disk).map(|d| d.1),
            worktrees: wt.and_then(|w| w.size),
            reclaimable: reclaim.iter().map(|g| g.bytes).sum(),
        },
        wisp: v.wisp,
        guard: Guard {
            held: v.hold.map(|h| h.kind),
            why: v.hold.map(|h| h.why.clone()),
            floor: r.limits.floor,
            ceiling: r.limits.ceiling,
            max_runs: r.limits.max_runs,
            runs: r.runs,
            paused_until: v.paused_until,
            battery: v.battery.map(|b| BatteryState { pct: b.pct, charging: b.charging, hold: v.battery_hold }),
        },
        tasks,
        crew,
        leftovers: v.leftovers,
        leftover_groups: v.groups,
        history: v.history,
        reclaim,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod reclaim_tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    fn done(id: &str) -> TaskInfo {
        TaskInfo { id: id.into(), title: "Fix the list".into(), state: "done".into(), ..TaskInfo::default() }
    }

    /// The disk that filled (2026-11-27): a left-over folder with 580 changed files and a 120 GB `target/` counted for
    /// nothing, since the leftover group takes only folders with no work of their own. Its build output counts now.
    #[test]
    fn a_leftover_with_work_counts_its_build_output() {
        let wt = WtView {
            leftovers: vec![WtLeftover { name: "wisp-8fb6".into(), size: Some(127 * GB), only: Some(580), build: Some(120 * GB) }],
            ..WtView::default()
        };
        let g = reclaim(Some(&wt), &|_| None);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].kind, "build");
        assert_eq!(g[0].bytes, 120 * GB);
        assert_eq!(g[0].ids, Vec::<String>::new());
        assert_eq!(g[0].extra["leftovers"], serde_json::json!(["wisp-8fb6"]));
    }

    #[test]
    fn groups_dont_count_a_folders_build_output_twice_and_a_busy_one_keeps_its() {
        let wt = WtView {
            items: vec![
                WtItem { id: "d".into(), idle: true, size: Some(10 * GB), busy: false, build: Some(8 * GB) },
                WtItem { id: "run".into(), idle: false, size: Some(5 * GB), busy: true, build: Some(4 * GB) },
            ],
            leftovers: vec![WtLeftover { name: "clean".into(), size: Some(3 * GB), only: Some(0), build: Some(GB) }],
            ..WtView::default()
        };
        let g = reclaim(Some(&wt), &|id| (id == "d").then(|| done(id)));
        let kinds: Vec<(&str, u64)> = g.iter().map(|x| (x.kind.as_str(), x.bytes / GB)).collect();
        assert_eq!(kinds, vec![("build", 9), ("done", 2), ("leftover", 2)]);
        assert_eq!(g[0].ids, vec!["d"]);
        assert_eq!(g[0].extra["folders"][0]["title"], "Fix the list");
        assert_eq!(g[0].extra["folders"][1]["leftover"], true, "biggest first: the task's 8 GB, then the leftover's 1 GB");
        let total: u64 = g.iter().map(|x| x.bytes).sum();
        assert_eq!(total, 13 * GB, "everything but the running task's folder");
    }
}
