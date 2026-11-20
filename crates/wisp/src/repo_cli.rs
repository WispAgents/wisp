//! A repo's agent history (ADR-0033, ADR-0035) from the terminal: `wisp track` / `untrack`, `wisp blame`,
//! `wisp log`, `wisp tasks --touching|--tracked`, `wisp ls --repo`, `wisp resume`, `wisp adopt`,
//! `wisp skill install`, and `wisp mcp` (a stdio MCP server with `wisp_history`, `wisp_blame`, `wisp_task`
//! and `wisp_resume`). Reading needs no running wisp: it's `.wisp/tasks` and git (commit trailers).
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wisp_history_core::repo;

use crate::client::{CliError, Client, Result};
use crate::code::Opts;
use crate::scripted::say;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new("git").args(args).current_dir(root).env("GIT_OPTIONAL_LOCKS", "0").output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `git args` with `input` on its stdin.
fn git_in(root: &Path, args: &[&str], input: &str) -> Option<String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let text = input.to_owned();
    // written from its own thread: a long list would fill the pipe while git fills stdout
    let feed = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let o = child.wait_with_output().ok()?;
    let _ = feed.join();
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The repo `cwd` is in (its top), or `cwd`. Unix: the nearest folder up with a `.git` (a folder, or a worktree's
/// file), resolved as git resolves it, with no git spawned (perf audit 2026-11-19 #6: ~20 ms of every `wisp blame`,
/// `log` and `tasks` call). Windows asks git, whose spelling of the path the rest expects.
pub fn root(cwd: &Path) -> PathBuf {
    #[cfg(unix)]
    if let Some(top) = cwd.ancestors().find(|d| d.join(".git").exists()) {
        // canonicalize: Unix only (no verbatim prefix there), so the path is spelled as git's toplevel spells it
        return std::fs::canonicalize(top).unwrap_or_else(|_| top.to_owned());
    }
    git(cwd, &["rev-parse", "--show-toplevel"]).map_or_else(|| cwd.to_owned(), |r| PathBuf::from(r.trim()))
}

/// A tracked task: its file (relative to the repo) and what it says.
#[derive(Debug, Clone)]
pub struct Tracked {
    pub path: String,
    pub doc: Value,
}

impl Tracked {
    pub fn id(&self) -> &str {
        s(&self.doc, "id")
    }
    fn files(&self) -> Vec<String> {
        self.doc.get("files").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
    }
    fn line(&self) -> String {
        let at = self.doc.get("createdAt").and_then(Value::as_i64).unwrap_or(0);
        let day = repo::year_month(at).replace('/', "-");
        let commits = self.doc.get("commits").and_then(Value::as_array).map_or(0, Vec::len);
        let pr = s(&self.doc, "pr");
        format!(
            "{}  {day}  {:<8}  {}{}{}",
            repo::short(self.id()),
            s(&self.doc["spec"], "agent"),
            s(&self.doc, "title"),
            if commits > 0 { format!("  ({commits} commit{})", if commits == 1 { "" } else { "s" }) } else { String::new() },
            if pr.is_empty() { String::new() } else { format!("  {pr}") }
        )
    }
}

/// Every task file in the repo's `.wisp/tasks`, newest first.
pub fn tracked(root: &Path) -> Vec<Tracked> {
    tracked_where(root, None)
}

/// [`tracked`], only the files whose text holds `needle` parsed (a path they touched: a file that doesn't name it
/// can't list it). The files are read and parsed on every core (perf audit 2026-11-19 #6: ~75 µs a file, one by one,
/// was 183 ms for `wisp tasks --touching` at 2,000 tasks).
fn tracked_where(root: &Path, needle: Option<&str>) -> Vec<Tracked> {
    let mut files = Vec::new();
    let mut stack = vec![root.join(".wisp/tasks")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(".task.yml") {
                files.push(p);
            }
        }
    }
    let one = |p: &PathBuf| -> Option<Tracked> {
        let text = std::fs::read_to_string(p).ok()?;
        if needle.is_some_and(|n| !text.contains(n)) {
            return None;
        }
        let doc = wisp_resources_core::formula::documents(&text, wisp_resources_core::formula::Format::Yaml).ok()?.into_iter().next()?;
        Some(Tracked { path: p.strip_prefix(root).map_or_else(|_| p.display().to_string(), |r| r.display().to_string()), doc })
    };
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get).min(8);
    let mut out: Vec<Tracked> = if files.len() < 64 || cores == 1 {
        files.iter().filter_map(one).collect()
    } else {
        let per = files.len().div_ceil(cores);
        std::thread::scope(|sc| {
            let parts: Vec<_> = files.chunks(per).map(|c| sc.spawn(move || c.iter().filter_map(one).collect::<Vec<_>>())).collect();
            parts.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        })
    };
    out.sort_by_key(|t| std::cmp::Reverse(t.doc.get("createdAt").and_then(Value::as_i64).unwrap_or(0)));
    out
}

fn find<'a>(all: &'a [Tracked], id: &str) -> Option<&'a Tracked> {
    all.iter().find(|t| t.id() == id || (id.len() >= 6 && t.id().starts_with(id)))
}

/// The commits that name `id` in their trailer.
fn commits_of(root: &Path, id: &str) -> Vec<String> {
    git(root, &["log", "--all", "--format=%H", &format!("--grep=Wisp-Task: {}", repo::short(id))])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .filter(|l| !l.is_empty())
        .collect()
}

/// `wisp track <id> [--level full|report|spec] [--repo <path>]`: `history.track` writes the task into the
/// repo's history (its store: files, notes or LFS), for you to commit.
#[allow(clippy::many_single_char_names)]
pub async fn track(c: &Client, o: &Opts, cwd: &Path) -> Result<i32> {
    let id = o.words.first().ok_or_else(|| CliError::said("wisp track <task id>"))?;
    let mut a = json!({ "id": id });
    if let Some(l) = &o.level {
        a["level"] = json!(l);
    }
    if let Some(r) = &o.project {
        a["repo"] = json!(root(Path::new(r)).display().to_string());
    } else if root(cwd).join(".wisp").is_dir() {
        a["repo"] = json!(root(cwd).display().to_string());
    }
    let t = c.cmd("history.track", a).await?;
    let st = crate::cli::style();
    for f in t["files"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        say(&format!("  {} {f}", wisp_cli_core::term::paint(&st, wisp_cli_core::term::GREEN, "+")));
    }
    let n = t["scrubbed"].as_u64().unwrap_or(0);
    let pr = t["pr"].as_str().map_or_else(String::new, |p| format!(", {p}"));
    say(&format!(
        "  tracked {} ({}{}{pr}): commit .wisp/ with your changes",
        repo::short(id),
        s(&t, "store"),
        if n > 0 { format!(", {n} things scrubbed") } else { String::new() }
    ));
    Ok(0)
}

/// Removes a task's files, or (with `keep`) what's above that level. → what went.
fn remove_task(root: &Path, id: &str, keep: Option<&str>) -> Vec<String> {
    let all = tracked(root);
    let Some(t) = find(&all, id) else { return Vec::new() };
    let base = t.path.trim_end_matches(".task.yml").to_owned();
    let full_id = t.id().to_owned();
    let mut gone = Vec::new();
    let mut rm = |p: PathBuf| {
        if std::fs::remove_file(&p).is_ok() {
            gone.push(p.strip_prefix(root).map_or_else(|_| p.display().to_string(), |r| r.display().to_string()));
        }
    };
    rm(root.join(".wisp/history").join(format!("{full_id}.jsonl.zst")));
    if keep != Some("report") {
        rm(root.join(format!("{base}.report.md")));
    }
    if keep.is_none() {
        rm(root.join(format!("{base}.task.yml")));
    }
    gone
}

/// `wisp untrack <id> [--keep report|spec]`: through `history.untrack` when a wisp answers (its store),
/// else the files here.
pub async fn untrack_cmd(c: Option<&Client>, o: &Opts, cwd: &Path) -> Result<i32> {
    let id = o.words.first().ok_or_else(|| CliError::said("wisp untrack <task id> [--keep report|spec]"))?;
    let Some(c) = c else { return untrack(o, cwd) };
    let mut a = json!({ "id": id, "repo": root(cwd).display().to_string() });
    if let Some(k) = o.level.as_deref().filter(|l| matches!(*l, "report" | "spec")) {
        a["keep"] = json!(k);
    }
    let g = c.cmd("history.untrack", a).await?;
    for f in g["files"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        say(&format!("  - {f}"));
    }
    say("  (git history keeps what was committed before)");
    Ok(0)
}

/// `wisp untrack` with no wisp running: the files here.
pub fn untrack(o: &Opts, cwd: &Path) -> Result<i32> {
    let id = o.words.first().ok_or_else(|| CliError::said("wisp untrack <task id> [--keep report|spec]"))?;
    let keep = o.level.as_deref().filter(|l| matches!(*l, "report" | "spec"));
    let gone = remove_task(&root(cwd), id, keep);
    if gone.is_empty() {
        return Err(CliError::said(format!("{id} isn't tracked here")));
    }
    for g in gone {
        say(&format!("  - {g}"));
    }
    say("  (git history keeps what was committed before)");
    Ok(0)
}

/// Who wrote what: (task id or "", lines) per commit of a blame.
fn blame_lines(root: &Path, file: &str, range: Option<(u32, u32)>) -> Vec<(String, u32)> {
    let mut args = vec!["blame".to_owned(), "--porcelain".to_owned()];
    if let Some((a, b)) = range {
        args.push("-L".into());
        args.push(format!("{a},{b}"));
    }
    args.push("--".into());
    args.push(file.to_owned());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git(root, &refs).unwrap_or_default();
    let mut per: BTreeMap<String, u32> = BTreeMap::new();
    for l in out.lines() {
        let mut w = l.split_whitespace();
        if let (Some(sha), Some(_), Some(_)) = (w.next(), w.next(), w.next())
            && sha.len() == 40
            && sha.bytes().all(|b| b.is_ascii_hexdigit())
        {
            *per.entry(sha.to_owned()).or_default() += 1;
        }
    }
    per.into_iter().collect()
}

/// Which tasks each commit is for (perf audit 2026-11-19 #2: one `git show` per commit and every task file parsed
/// again per commit took 1.2 s for a file, 2.7 s for a folder): every commit's message from one `git log`, and the
/// task files that list a commit (a backfilled task) indexed once from what `blame` already read.
struct Commits {
    bodies: std::collections::HashMap<String, String>,
    listed: std::collections::HashMap<String, Vec<String>>,
}

/// `%H NUL %B RS` records, as `git log --format=%H%x00%B%x1e` writes them.
fn parse_bodies(text: &str) -> std::collections::HashMap<String, String> {
    text.split('\u{1e}')
        .filter_map(|r| r.trim_start_matches('\n').split_once('\0'))
        .map(|(sha, body)| (sha.trim().to_owned(), body.to_owned()))
        .filter(|(sha, _)| !sha.is_empty())
        .collect()
}

impl Commits {
    fn index(all: &[Tracked]) -> std::collections::HashMap<String, Vec<String>> {
        let mut listed: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for t in all {
            for c in t.doc.get("commits").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                let ids = listed.entry(c.to_owned()).or_default();
                if !ids.iter().any(|x| x == t.id()) {
                    ids.push(t.id().to_owned());
                }
            }
        }
        listed
    }

    /// The messages of `shas`, from one `git log --no-walk` fed the list on its stdin.
    fn of(root: &Path, all: &[Tracked], shas: &[String]) -> Self {
        let mut want: Vec<&str> = shas.iter().map(String::as_str).collect();
        want.sort_unstable();
        want.dedup();
        let bodies = if want.is_empty() {
            std::collections::HashMap::new()
        } else {
            let list = want.join("\n") + "\n";
            parse_bodies(&git_in(root, &["log", "--no-walk=unsorted", "--stdin", "--format=%H%x00%B%x1e"], &list).unwrap_or_default())
        };
        Self { bodies, listed: Self::index(all) }
    }

    /// The messages of every commit that touched `path`, newest first, from one `git log` (a folder's blame).
    fn touching(root: &Path, all: &[Tracked], path: &str) -> (Vec<String>, Self) {
        let text = git(root, &["log", "--format=%H%x00%B%x1e", "--", path]).unwrap_or_default();
        let order: Vec<String> =
            text.split('\u{1e}').filter_map(|r| r.trim_start_matches('\n').split_once('\0')).map(|(s, _)| s.trim().to_owned()).collect();
        (order, Self { bodies: parse_bodies(&text), listed: Self::index(all) })
    }

    /// The tasks a commit is for: its trailers, else the task files that list it (a backfilled task).
    fn tasks(&self, sha: &str) -> Vec<String> {
        let ids = repo::trailers(self.bodies.get(sha).map_or("", String::as_str));
        if !ids.is_empty() {
            return ids;
        }
        self.listed.get(sha).cloned().unwrap_or_default()
    }
}

/// The answer to `wisp blame <target>`: tasks (with lines or commits) and what no task wrote.
pub fn blame(root: &Path, target: &str) -> Value {
    let all = tracked(root);
    let card = |id: &str, extra: Value| {
        let t = find(&all, id);
        json!({ "task": t.map_or(id, Tracked::id), "title": t.map(|t| s(&t.doc, "title")), "prompt": t.map(|t| s(&t.doc["spec"], "prompt")), "agent": t.map(|t| s(&t.doc["spec"], "agent")), "pr": t.map(|t| s(&t.doc, "pr")), "file": t.map(|t| t.path.clone()), "tracked": t.is_some(), "what": extra })
    };
    // a task id: its commits, PR and files
    if let Some(t) = find(&all, target) {
        let mut commits = commits_of(root, t.id());
        for c in t.doc.get("commits").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !commits.iter().any(|x| x == c) {
                commits.push(c.to_owned());
            }
        }
        return json!({ "target": target, "kind": "task", "tasks": [card(t.id(), json!({ "commits": commits, "files": t.files() }))] });
    }
    // a PR: the tasks that made it
    if target.starts_with('#') || target.contains("/pull/") {
        let n = target.trim_start_matches('#');
        let hits: Vec<Value> = all
            .iter()
            .filter(|t| s(&t.doc, "pr") == target || s(&t.doc, "pr").ends_with(&format!("/pull/{n}")) || s(&t.doc, "pr") == format!("#{n}"))
            .map(|t| card(t.id(), json!({})))
            .collect();
        return json!({ "target": target, "kind": "pr", "tasks": hits });
    }
    // file:line or file:a-b, a file, a folder
    let (path, range) = match target.rsplit_once(':') {
        Some((p, r)) if r.chars().all(|c| c.is_ascii_digit() || c == '-') && !r.is_empty() => {
            let (a, b) = r.split_once('-').unwrap_or((r, r));
            (p.to_owned(), Some((a.parse().unwrap_or(1), b.parse().unwrap_or(1))))
        }
        _ => (target.to_owned(), None),
    };
    let full = root.join(&path);
    if full.is_file() {
        let lines = blame_lines(root, &path, range);
        let total: u32 = lines.iter().map(|(_, n)| n).sum();
        let commits = Commits::of(root, &all, &lines.iter().map(|(sha, _)| sha.clone()).collect::<Vec<_>>());
        let mut by: BTreeMap<String, u32> = BTreeMap::new();
        for (sha, n) in &lines {
            let ids = commits.tasks(sha);
            *by.entry(ids.first().cloned().unwrap_or_default()).or_default() += n;
        }
        let hand = by.remove("").unwrap_or(0);
        let mut tasks: Vec<(String, u32)> = by.into_iter().collect();
        tasks.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        return json!({ "target": target, "kind": if range.is_some() { "lines" } else { "file" }, "lines": total, "byHand": hand, "tasks": tasks.iter().map(|(id, n)| card(id, json!({ "lines": n, "share": if total > 0 { f64::from(*n) / f64::from(total) } else { 0.0 } }))).collect::<Vec<_>>() });
    }
    if full.is_dir() {
        let (log, commits) = Commits::touching(root, &all, &path);
        let mut seen: Vec<String> = Vec::new();
        for sha in &log {
            for id in commits.tasks(sha) {
                if !seen.contains(&id) {
                    seen.push(id);
                }
            }
        }
        for t in all.iter().filter(|t| t.files().iter().any(|f| f.starts_with(path.trim_end_matches('/')))) {
            if !seen.iter().any(|x| t.id().starts_with(x.as_str()) || x == t.id()) {
                seen.push(t.id().to_owned());
            }
        }
        return json!({ "target": target, "kind": "folder", "tasks": seen.iter().map(|id| card(id, json!({}))).collect::<Vec<_>>() });
    }
    // a commit or a range
    let list = if target.contains("..") {
        git(root, &["rev-list", target])
    } else {
        git(root, &["rev-parse", "--verify", &format!("{target}^{{commit}}")])
    };
    let shas: Vec<String> = list.unwrap_or_default().lines().map(str::to_owned).collect();
    let commits = Commits::of(root, &all, &shas);
    let mut tasks: Vec<Value> = Vec::new();
    let mut hand = 0;
    for sha in &shas {
        let ids = commits.tasks(sha);
        if ids.is_empty() {
            hand += 1;
        }
        for id in ids {
            tasks.push(card(&id, json!({ "commit": sha })));
        }
    }
    json!({ "target": target, "kind": if shas.is_empty() { "unknown" } else { "commits" }, "commits": shas.len(), "byHand": hand, "tasks": tasks })
}

fn show_blame(b: &Value) {
    let tasks = b["tasks"].as_array().cloned().unwrap_or_default();
    if tasks.is_empty() {
        say(&format!("  {}: no task behind it (written by hand, or not tracked)", s(b, "target")));
    }
    for t in &tasks {
        let what = &t["what"];
        let share =
            what.get("share").and_then(Value::as_f64).map(|x| format!("  {:.0}% ({} lines)", x * 100.0, what["lines"])).unwrap_or_default();
        say(&format!(
            "  {}  {}{share}",
            repo::short(s(t, "task")),
            t["title"].as_str().unwrap_or("(not in .wisp/: a task from another clone, or untracked)")
        ));
        if let Some(p) = t["prompt"].as_str().filter(|p| !p.is_empty()) {
            say(&format!("      asked: {}", p.lines().next().unwrap_or_default().chars().take(100).collect::<String>()));
        }
        if let Some(pr) = t["pr"].as_str().filter(|p| !p.is_empty()) {
            say(&format!("      {pr}"));
        }
    }
    if let Some(n) = b.get("byHand").and_then(Value::as_u64).filter(|n| *n > 0) {
        say(&format!("  {n} {} by hand", if b["kind"] == "commits" { "commits" } else { "lines" }));
    }
}

/// `wisp blame <file:line|file|folder|commit|range|#PR|task id> [--json]`.
#[allow(clippy::unnecessary_wraps)] // every command answers a Result
pub fn blame_cmd(o: &Opts, cwd: &Path) -> Result<i32> {
    let target = o.words.first().ok_or_else(|| CliError::said("wisp blame <file>:<line> | <file> | <commit> | #<pr> | <task id>"))?;
    let b = blame(&root(cwd), target);
    if o.json {
        say(&serde_json::to_string_pretty(&b).unwrap_or_default());
    } else {
        show_blame(&b);
    }
    Ok(0)
}

/// The tracked tasks, filtered: a path they touched, an agent, words, `--since`.
pub fn filtered(root: &Path, path: Option<&str>, agent: Option<&str>, grep: Option<&str>) -> Vec<Tracked> {
    tracked_where(root, path.map(|p| p.trim_end_matches('/')).filter(|p| !p.is_empty()))
        .into_iter()
        .filter(|t| path.is_none_or(|p| t.files().iter().any(|f| f.starts_with(p.trim_end_matches('/')))))
        .filter(|t| agent.is_none_or(|a| s(&t.doc["spec"], "agent") == a))
        .filter(|t| {
            grep.is_none_or(|g| {
                format!("{} {}", s(&t.doc, "title"), s(&t.doc["spec"], "prompt")).to_lowercase().contains(&g.to_lowercase())
            })
        })
        .collect()
}

/// `wisp log [path]`, `wisp tasks --touching <path>|--tracked`, `wisp ls --repo [--path p] [--agent a] [--grep w]`.
#[allow(clippy::unnecessary_wraps)] // every command answers a Result
pub fn log(argv: &[String], o: &Opts, cwd: &Path) -> Result<i32> {
    let flag = |k: &str| argv.iter().position(|a| a == k).and_then(|i| argv.get(i + 1)).map(String::as_str);
    let path =
        flag("--touching").or_else(|| flag("--path")).or_else(|| o.words.first().map(String::as_str).filter(|w| !w.starts_with('-')));
    let list = filtered(&root(o.project.as_ref().map_or(cwd, Path::new)), path, flag("--agent"), flag("--grep"));
    if o.json {
        say(&serde_json::to_string_pretty(&list.iter().map(|t| t.doc.clone()).collect::<Vec<_>>()).unwrap_or_default());
        return Ok(0);
    }
    if list.is_empty() {
        say("  no tracked tasks here yet (wisp track <id>, or history: { track: auto } in .wisp/wisp.yml)");
    }
    for t in list {
        say(&format!("⎇ {}", t.line()));
    }
    Ok(0)
}

/// `wisp resume <id>`: a new task that carries on from where that one stopped, on today's code.
pub async fn resume(c: &Client, o: &Opts, cwd: &Path) -> Result<i32> {
    let id = o.words.first().ok_or_else(|| CliError::said("wisp resume <task id>"))?;
    let root = root(cwd);
    let all = tracked(&root);
    let (title, spec, report, full) = if let Some(t) = find(&all, id) {
        let md = std::fs::read_to_string(root.join(t.path.replace(".task.yml", ".report.md"))).unwrap_or_default();
        (s(&t.doc, "title").to_owned(), t.doc["spec"].clone(), md, t.id().to_owned())
    } else {
        let task = c.cmd("runtime.get", json!({ "id": id })).await?;
        let spec = c.cmd("runtime.spec", json!({ "id": id })).await.unwrap_or_default();
        let p = c.cmd("share.preview", json!({ "id": id, "level": "report" })).await.unwrap_or_default();
        let md = p.pointer("/bundle/report").map(|r| repo::report_md(s(&task, "title"), r)).unwrap_or_default();
        (s(&task, "title").to_owned(), spec, md, s(&task, "id").to_owned())
    };
    let report: String = report.chars().take(6000).collect();
    let prompt = format!(
        "Carry on with an earlier task: \"{title}\".\n\nWhat was asked then:\n{}\n\nWhat happened (its report):\n{report}\n\nPick it up from there, on the code as it is today.",
        s(&spec, "prompt")
    );
    let mut a = json!({ "prompt": prompt, "title": format!("{title} (resumed)"), "cwd": root.display().to_string(), "lineage": { "forkOf": full } });
    if let Some(ag) = spec.get("agent").and_then(Value::as_str).filter(|x| !x.is_empty()) {
        a["agent"] = json!(ag);
    }
    for (k, v) in &o.over {
        a[k] = v.clone();
    }
    let made = c.cmd("runtime.create", a).await?;
    say(&format!("  ◌ resumed {} as {}  wisp attach {}", repo::short(&full), s(&made, "id"), s(&made, "id")));
    Ok(0)
}

fn track_mode(root: &Path, key: &str, default: &str) -> String {
    let text = std::fs::read_to_string(root.join(".wisp/wisp.yml")).unwrap_or_default();
    let doc = wisp_resources_core::formula::documents(&text, wisp_resources_core::formula::Format::Yaml)
        .ok()
        .and_then(|d| d.into_iter().next())
        .unwrap_or_default();
    doc.pointer(&format!("/history/{key}")).and_then(Value::as_str).unwrap_or(default).to_owned()
}

/// `wisp adopt [<session id>|--all [--since 7d]] [--track]`, and `--hook` (Claude Code's `SessionEnd`: the
/// session on stdin, adopted per `history: { adopt: ask|auto|off }`).
pub async fn adopt(c: &Client, argv: &[String], o: &Opts, cwd: &Path) -> Result<i32> {
    let has = |k: &str| argv.iter().any(|a| a == k);
    let root = root(cwd);
    let flags = ["--track", "--hook", "--all", "--session"];
    let (mut want, mut track): (Vec<String>, bool) =
        (o.words.iter().filter(|w| !flags.contains(&w.as_str())).cloned().collect(), has("--track"));
    let mut dir = cwd.display().to_string();
    if has("--hook") {
        let mut input = String::new();
        let _ = std::io::stdin().lock().read_line(&mut input);
        let v: Value = serde_json::from_str(&input).unwrap_or_default();
        if let Some(c) = v.get("cwd").and_then(Value::as_str) {
            c.clone_into(&mut dir);
        }
        let root = self::root(Path::new(&dir));
        match track_mode(&root, "adopt", "ask").as_str() {
            "auto" => {
                want = vec![s(&v, "session_id").to_owned()];
                track = track_mode(&root, "track", "ask") == "auto";
            }
            "off" => return Ok(0),
            _ => {
                say(&format!("wisp: track this session? wisp adopt {} --track", s(&v, "session_id")));
                return Ok(0);
            }
        }
    }
    if let Some(i) = argv.iter().position(|a| a == "--session") {
        want = argv.get(i + 1).cloned().into_iter().collect();
    }
    let list = c.cmd("runtime.sessions", json!({ "cwd": dir })).await?;
    let sessions = list.as_array().cloned().unwrap_or_default();
    if has("--all") {
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
        let since = o.since.as_deref().and_then(|t| crate::history_cli::since_ms(t, now).ok()).unwrap_or(0);
        want = sessions.iter().filter(|x| x["updatedAt"].as_i64().unwrap_or(0) >= since).map(|x| s(x, "id").to_owned()).collect();
    }
    if want.is_empty() {
        if sessions.is_empty() {
            say("  no sessions outside wisp in this folder (Claude Code, Codex)");
        }
        for x in &sessions {
            say(&format!("  {}  {:<7}  {} prompts  {}", s(x, "id"), s(x, "harness"), x["prompts"], s(x, "title")));
        }
        if !sessions.is_empty() {
            say("  wisp adopt <id> [--track], or --all --since 7d");
        }
        return Ok(0);
    }
    for id in want {
        let harness = sessions
            .iter()
            .find(|x| s(x, "id") == id)
            .map_or_else(|| if id.len() == 36 && id.contains('-') { "claude" } else { "codex" }.to_owned(), |x| s(x, "harness").to_owned());
        let made = c.cmd("runtime.adoptSession", json!({ "harness": harness, "id": id })).await?;
        let task = s(&made, "id").to_owned();
        say(&format!("  ✓ {harness} session {id} → task {task} (a follow-up continues it there)"));
        if track {
            let mut o2 = o.clone();
            o2.words = vec![task];
            track_in(c, &o2, &root).await?;
        }
    }
    Ok(0)
}

async fn track_in(c: &Client, o: &Opts, root: &Path) -> Result<i32> {
    let mut o = o.clone();
    o.project = Some(root.display().to_string());
    track(c, &o, root).await
}

/// `wisp blame`: `history.blame` when a wisp answers (with how much is still there), else offline here.
pub async fn blame_any(c: Option<&Client>, o: &Opts, cwd: &Path) -> Result<i32> {
    let Some(c) = c else { return blame_cmd(o, cwd) };
    let target = o.words.first().ok_or_else(|| CliError::said("wisp blame <file>:<line> | <file> | <commit> | #<pr> | <task id>"))?;
    let b = c.cmd("history.blame", json!({ "target": target, "repo": root(cwd).display().to_string() })).await?;
    if o.json {
        say(&serde_json::to_string_pretty(&b).unwrap_or_default());
    } else {
        show_blame(&b);
        if let Some(x) = b.pointer("/tasks/0/what/stillThere").and_then(Value::as_f64) {
            say(&format!("  {:.0}% of what it wrote is still there", x * 100.0));
        }
    }
    Ok(0)
}

/// `wisp skill install --agent claude|codex`: wisp's MCP server (`wisp mcp`) for that harness, and for
/// Claude Code its `SessionEnd` hook (`wisp adopt --hook`) and wisp's skill.
pub fn skill_install(argv: &[String], cwd: &Path) -> Result<i32> {
    let agent = argv.iter().position(|a| a == "--agent").and_then(|i| argv.get(i + 1)).map_or("claude", String::as_str);
    let root = root(cwd);
    let exe = std::env::current_exe().map_or_else(|_| "wisp".into(), |p| p.display().to_string());
    let rw = |p: &Path, f: &dyn Fn(&mut Value)| -> Result<()> {
        let mut v: Value = std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
        f(&mut v);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| CliError::said(e.to_string()))?;
        }
        std::fs::write(p, format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())).map_err(|e| CliError::said(e.to_string()))
    };
    // `--agent all`: each in turn, one outcome each (setup audit F29: three ✓, then "not all yet")
    let agents: &[&str] = if agent == "all" { &["claude", "codex", "opencode"] } else { std::slice::from_ref(&agent) };
    for agent in agents {
        match *agent {
            "claude" => {
                rw(&root.join(".mcp.json"), &|v| v["mcpServers"]["wisp"] = json!({ "command": exe, "args": ["mcp"] }))?;
                rw(&root.join(".claude/settings.json"), &|v| {
                    // wisp's own entry is replaced (one written before the path was quoted broke on a space); the
                    // user's other SessionEnd hooks stay as they are
                    let hook = json!({ "hooks": [adopt_hook(&exe, cfg!(windows))] });
                    let mut list = v["hooks"]["SessionEnd"].as_array().cloned().unwrap_or_default();
                    match list.iter().position(|h| h.to_string().contains("adopt --hook")) {
                        Some(i) => list[i] = hook,
                        None => list.push(hook),
                    }
                    v["hooks"]["SessionEnd"] = Value::Array(list);
                })?;
                let skill = root.join(".claude/skills/wisp/SKILL.md");
                std::fs::create_dir_all(skill.parent().unwrap_or(&root)).map_err(|e| CliError::said(e.to_string()))?;
                std::fs::write(&skill, "---\nname: wisp\ndescription: Read why code is the way it is: the wisp tasks behind a file, a line, a commit or a PR.\n---\n\nBefore changing a file, ask wisp which tasks shaped it: the `wisp_history` tool (paths) or `wisp_blame` (a file:line, a commit, a PR), then `wisp_task` for one task's prompt and report. To pick an old task up, `wisp_resume` asks the user. Without MCP: `wisp tasks --touching <path> --json`, `wisp blame <target> --json`, or read `.wisp/tasks/`.\n").map_err(|e| CliError::said(e.to_string()))?;
                say("  ✓ .mcp.json (wisp mcp), .claude/settings.json (SessionEnd: wisp adopt --hook), .claude/skills/wisp/SKILL.md");
            }
            "codex" => {
                let home = std::env::var("CODEX_HOME")
                    .map_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".codex"), PathBuf::from);
                let cfg = home.join("config.toml");
                let had = std::fs::read_to_string(&cfg).unwrap_or_default();
                if !had.contains("[mcp_servers.wisp]") {
                    std::fs::create_dir_all(&home).map_err(|e| CliError::said(e.to_string()))?;
                    std::fs::write(&cfg, format!("{had}{}", codex_mcp_table(&exe))).map_err(|e| CliError::said(e.to_string()))?;
                }
                say(&format!("  ✓ {} ([mcp_servers.wisp]); Codex has no session-end hook: wisp adopt --all --since 1d", cfg.display()));
            }
            "opencode" => say("  · opencode: its skill is in place; add wisp mcp to its MCP servers by hand (opencode.json) for the tools"),
            other => return Err(CliError::said(format!("wisp skill install --agent claude|codex|opencode|all (not {other})"))),
        }
    }
    Ok(0)
}

/// Claude Code's `SessionEnd` hook running `wisp adopt --hook`, with wisp's path quoted for the shell that runs it:
/// `sh -c` on macOS and Linux (single quotes, a `'` as `'\''`), and on Windows PowerShell, named by the hook's own
/// `shell` (Claude Code would use Git Bash when it's installed and PowerShell when it isn't; PowerShell is always
/// there): `& '<path>'` with a `'` (and the curly ones PowerShell reads as one) doubled. Unquoted, a path with a space (`C:\Program Files\…`, `/Users/John Smith/…`)
/// ran `C:\Program` and the hook never adopted anything.
fn adopt_hook(exe: &str, windows: bool) -> Value {
    if windows {
        // PowerShell takes ‘ ’ ‚ ‛ for a single quote too: each is doubled like '
        let quoted: String =
            exe.chars().flat_map(|c| if matches!(c, '\'' | '‘' | '’' | '‚' | '‛') { vec![c, c] } else { vec![c] }).collect();
        json!({ "type": "command", "shell": "powershell", "command": format!("& '{quoted}' adopt --hook") })
    } else {
        json!({ "type": "command", "command": format!("'{}' adopt --hook", exe.replace('\'', r"'\''")) })
    }
}

/// Codex's `[mcp_servers.wisp]` table for wisp at `exe`. The path is a TOML string quoted as JSON quotes it (`\\` for
/// a backslash, valid TOML escapes): written raw between quotes, Windows' `C:\Users\…` had `\U` read as an escape and
/// Codex refused its whole config.
fn codex_mcp_table(exe: &str) -> String {
    format!("\n[mcp_servers.wisp]\ncommand = {}\nargs = [\"mcp\"]\n", Value::String(exe.to_owned()))
}

fn tool_text(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

/// One MCP call on the repo's history. `resume` only asks: starting work is the user's.
fn mcp_tool(root: &Path, name: &str, args: &Value) -> Value {
    match name {
        "wisp_history" => {
            let paths: Vec<String> =
                args.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
            let mut seen = Vec::new();
            let mut out = Vec::new();
            for p in if paths.is_empty() { vec![String::new()] } else { paths } {
                for t in filtered(root, Some(&p).filter(|p| !p.is_empty()).map(String::as_str), None, None) {
                    if !seen.contains(&t.id().to_owned()) {
                        seen.push(t.id().to_owned());
                        out.push(json!({ "id": t.id(), "title": s(&t.doc, "title"), "prompt": s(&t.doc["spec"], "prompt"), "agent": s(&t.doc["spec"], "agent"), "pr": s(&t.doc, "pr"), "files": t.files(), "file": t.path }));
                    }
                }
            }
            tool_text(&serde_json::to_string_pretty(&out).unwrap_or_default())
        }
        "wisp_blame" => tool_text(
            &serde_json::to_string_pretty(&blame(root, args.get("target").and_then(Value::as_str).unwrap_or_default())).unwrap_or_default(),
        ),
        "wisp_task" => {
            let all = tracked(root);
            match find(&all, args.get("id").and_then(Value::as_str).unwrap_or_default()) {
                Some(t) => {
                    let md = std::fs::read_to_string(root.join(t.path.replace(".task.yml", ".report.md"))).unwrap_or_default();
                    tool_text(&format!("{}\n\n{md}", wisp_resources_core::export::yaml(&t.doc)))
                }
                None => json!({ "content": [{ "type": "text", "text": "no tracked task with that id" }], "isError": true }),
            }
        }
        "wisp_resume" => tool_text(&format!(
            "No wisp is running here, so nothing was asked. Ask the user to run: wisp resume {}",
            args.get("id").and_then(Value::as_str).unwrap_or_default()
        )),
        _ => json!({ "content": [{ "type": "text", "text": format!("{name} isn't a tool here") }], "isError": true }),
    }
}

fn mcp_tools() -> Value {
    let one = |name: &str, desc: &str, prop: &str, kind: Value| json!({ "name": name, "description": desc, "inputSchema": { "type": "object", "properties": { prop: kind }, "required": [prop] } });
    json!([
        one(
            "wisp_history",
            "The wisp tasks that touched these paths (newest first): what was asked, the agent, the PR. Read them before changing those files.",
            "paths",
            json!({ "type": "array", "items": { "type": "string" } })
        ),
        one(
            "wisp_blame",
            "Which task made this: a file:line or file:10-40, a file or folder, a commit or range, a PR (#812) or a task id.",
            "target",
            json!({ "type": "string" })
        ),
        one("wisp_task", "One tracked task: its spec and its report.", "id", json!({ "type": "string" })),
        one("wisp_resume", "Ask the user to resume an earlier task (they start it).", "id", json!({ "type": "string" })),
    ])
}

/// The reply to one JSON-RPC message (None for a notification).
pub fn mcp_reply(root: &Path, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let result = match msg.get("method").and_then(Value::as_str).unwrap_or_default() {
        "initialize" => {
            json!({ "protocolVersion": msg.pointer("/params/protocolVersion").cloned().unwrap_or(json!("2025-06-18")), "capabilities": { "tools": {} }, "serverInfo": { "name": "wisp", "version": env!("CARGO_PKG_VERSION") } })
        }
        // the history tools, and v1's task tools (setup audit F28; they need a wisp, started on the first call)
        "tools/list" => {
            json!({ "tools": mcp_tools().as_array().cloned().unwrap_or_default().into_iter().chain(crate::mcp_tasks::tools()).collect::<Vec<_>>() })
        }
        "tools/call" => mcp_tool(
            root,
            msg.pointer("/params/name").and_then(Value::as_str).unwrap_or_default(),
            msg.pointer("/params/arguments").unwrap_or(&json!({})),
        ),
        "ping" => json!({}),
        m => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("{m}: not a method here") } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// A tool this server answers itself: the history tools and v1's task tools.
fn is_own_tool(name: &str) -> bool {
    crate::mcp_tasks::is_task_tool(name)
        || mcp_tools().as_array().into_iter().flatten().any(|t| t.get("name").and_then(Value::as_str) == Some(name))
}

/// A catalogue tool's call, run through `/api/mcp`; its error as the tool's when wisp doesn't answer.
async fn through(client: Option<&Client>, msg: &Value, at: &Path) -> Value {
    let failed = |why: String| json!({ "jsonrpc": "2.0", "id": msg.get("id").cloned().unwrap_or(Value::Null), "result": { "content": [{ "type": "text", "text": why }], "isError": true } });
    match client {
        Some(c) => c.mcp(msg, at).await.unwrap_or_else(|e| failed(e.to_string())),
        None => failed("wisp isn't running and didn't start: run wisp serve".to_owned()),
    }
}

/// `reply` (this server's `tools/list` answer) with `/api/mcp`'s tools after its own, each name once.
fn merge_catalogue(reply: &mut Value, catalogue: Option<&Value>) {
    let Some(theirs) = catalogue.and_then(|c| c.pointer("/result/tools")).and_then(Value::as_array) else { return };
    let Some(ours) = reply.pointer_mut("/result/tools").and_then(Value::as_array_mut) else { return };
    let mut have: Vec<String> = ours.iter().filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned)).collect();
    for t in theirs {
        let Some(name) = t.get("name").and_then(Value::as_str) else { continue };
        if !have.iter().any(|h| h == name) {
            have.push(name.to_owned());
            ours.push(t.clone());
        }
    }
}

/// `wisp_resume` with a wisp running: a request the user approves (nothing starts until then).
async fn resume_request(c: &Client, root: &Path, args: &Value) -> Value {
    let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
    let why = args.get("why").and_then(Value::as_str).unwrap_or_default();
    match c
        .cmd("history.resumeRequest", json!({ "id": id, "by": "an agent (wisp mcp)", "why": why, "repo": root.display().to_string() }))
        .await
    {
        Ok(r) => tool_text(&format!(
            "Asked: request {} waits for the user's yes (wisp: Resume requests). Nothing starts before that.",
            s(&r, "id")
        )),
        Err(e) => json!({ "content": [{ "type": "text", "text": e.to_string() }], "isError": true }),
    }
}

/// `wisp mcp [--repo <path>]`: the history tools over stdio, from the files and git (no wisp needed;
/// `wisp_resume` asks the running wisp, when there is one).
#[allow(clippy::unnecessary_wraps)] // every command answers a Result
pub async fn mcp(argv: &[String], cwd: &Path, port: u16) -> Result<i32> {
    let at = argv.iter().position(|a| a == "--repo").and_then(|i| argv.get(i + 1)).map_or_else(|| cwd.to_owned(), PathBuf::from);
    let root = root(&at);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut client = Client::connect(port, false).await.ok();
    for line in stdin.lock().lines().map_while(std::result::Result::ok) {
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let tool = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default().to_owned();
        // the catalogue, as `/api/mcp` offers it to the user (audit B #4: 11 tools here, 240 there): listed with a wisp
        // running, after this server's own; any other tool runs through it (a wisp started headless if none answers)
        if method == "tools/list"
            && let (Some(c), Some(mut reply)) = (&client, mcp_reply(&root, &msg))
        {
            merge_catalogue(&mut reply, c.mcp(&msg, &at).await.ok().as_ref());
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
            continue;
        }
        if method == "tools/call" && !is_own_tool(&tool) {
            if client.is_none() {
                client = Client::connect(port, true).await.ok();
            }
            let reply = through(client.as_ref(), &msg, &at).await;
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
            continue;
        }
        if msg.get("method").and_then(Value::as_str) == Some("tools/call") && crate::mcp_tasks::is_task_tool(&tool) {
            // a task tool: the running wisp, started headless when none answers (as v1's server did)
            if client.is_none() {
                client = Client::connect(port, true).await.ok();
            }
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let result = match &client {
                Some(c) => crate::mcp_tasks::call(c, &tool, msg.pointer("/params/arguments").unwrap_or(&json!({})), &at).await,
                None => {
                    json!({ "content": [{ "type": "text", "text": "wisp isn't running and didn't start: run wisp serve" }], "isError": true })
                }
            };
            let _ = writeln!(out, "{}", json!({ "jsonrpc": "2.0", "id": id, "result": result }));
            let _ = out.flush();
            continue;
        }
        let reply = match (&client, msg.pointer("/params/name").and_then(Value::as_str)) {
            (Some(c), Some("wisp_resume")) if msg.get("method").and_then(Value::as_str) == Some("tools/call") => Some(
                json!({ "jsonrpc": "2.0", "id": msg.get("id").cloned().unwrap_or(Value::Null), "result": resume_request(c, &root, msg.pointer("/params/arguments").unwrap_or(&json!({}))).await }),
            ),
            _ => mcp_reply(&root, &msg),
        };
        if let Some(r) = reply {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
    Ok(0)
}

/// How `wisp open` runs a task's harness: the program, its args, where, and the session it'll be.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPlan {
    pub harness: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    /// resume | restore | continue
    pub how: String,
    /// the session it resumes (what's adopted back after); None for a continuation (found after)
    pub session: Option<String>,
    pub note: String,
}

/// The harness's interactive CLI (`WISP_CLAUDE_BIN`… as wisp's own runs, else its name on PATH).
pub fn harness_program(h: &str) -> String {
    let (var, name) = match h {
        "claude" => ("WISP_CLAUDE_BIN", "claude"),
        "codex" => ("WISP_CODEX_BIN", "codex"),
        "cursor" => ("WISP_CURSOR_BIN", "cursor-agent"),
        "opencode" => ("WISP_OPENCODE_BIN", "opencode"),
        "copilot" => ("WISP_COPILOT_BIN", "copilot"),
        "grok" => ("WISP_GROK_BIN", "grok"),
        "antigravity" => ("WISP_ANTIGRAVITY_BIN", "agy"),
        other => ("", other),
    };
    std::env::var(var).ok().filter(|b| !b.is_empty()).unwrap_or_else(|| name.to_owned())
}

/// `program args` with this terminal, found as wisp's own runs find it: on Windows `claude` is `claude.exe` or npm's
/// `claude.cmd` (PATHEXT), and a shim runs with its arguments intact (`wisp_proc::terminal_command`).
pub fn terminal(program: &str, args: &[String]) -> std::process::Command {
    #[cfg(feature = "proc")]
    {
        wisp_proc::terminal_command(program, args)
    }
    #[cfg(not(feature = "proc"))]
    {
        let mut c = std::process::Command::new(program);
        c.args(args);
        c
    }
}

/// A harness's "resume this session" args, when it has one.
pub fn resume_args(h: &str, session: &str) -> Option<Vec<String>> {
    match h {
        "claude" => Some(vec!["--resume".into(), session.into()]),
        "codex" => Some(vec!["resume".into(), session.into()]),
        _ => None,
    }
}

/// What `wisp open` does, from what's known: the task's harness and session, the harness asked for, and
/// whether a native log could be restored.
pub fn open_plan(task_harness: &str, session: Option<&str>, asked: Option<&str>, cwd: &str, prime: &str) -> OpenPlan {
    let harness = asked.unwrap_or(task_harness).to_owned();
    let program = harness_program(&harness);
    if harness == task_harness
        && let Some(s) = session
        && let Some(args) = resume_args(&harness, s)
    {
        return OpenPlan {
            harness,
            program,
            args,
            cwd: cwd.to_owned(),
            how: "resume".into(),
            session: Some(s.to_owned()),
            note: String::new(),
        };
    }
    let note = format!("a new {harness} session that continues the task (not the same session)");
    OpenPlan {
        harness,
        program,
        args: vec![format!("You are continuing a task someone (or another agent) started.\n\n{prime}")],
        cwd: cwd.to_owned(),
        how: "continue".into(),
        session: None,
        note,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

/// The folder a task opens in: its own, else (gone) a fresh worktree at its base commit.
fn open_cwd(cwd: &str, spec: &Value) -> Result<String> {
    if Path::new(cwd).is_dir() {
        return Ok(cwd.to_owned());
    }
    let sha = spec
        .pointer("/base/sha")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CliError::said(format!("{cwd} is gone and the task has no base commit to open at")))?;
    let from = spec
        .get("cwd")
        .and_then(Value::as_str)
        .map(Path::new)
        .filter(|p| p.is_dir())
        .ok_or_else(|| CliError::said(format!("{cwd} is gone, and so is its repo")))?;
    let dir = std::env::temp_dir().join(format!("wisp-open-{}", &sha[..sha.len().min(12)]));
    if !dir.is_dir() {
        git(&root(from), &["worktree", "add", "--detach", &dir.display().to_string(), sha])
            .ok_or_else(|| CliError::said(format!("couldn't check out {sha} to open it")))?;
    }
    Ok(dir.display().to_string())
}

/// `wisp open <task> [--in <harness>] [--print] [--no-adopt]` (v1's Take over).
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
pub async fn open_task(c: &Client, argv: &[String], o: &Opts, cwd: &Path) -> Result<i32> {
    let flag = |k: &str| argv.iter().position(|a| a == k).and_then(|i| argv.get(i + 1)).map(String::as_str);
    let has = |k: &str| argv.iter().any(|a| a == k);
    let id = o.words.first().ok_or_else(|| CliError::said("wisp open <task id> [--in claude|codex|…]"))?;
    let asked = flag("--in");
    let plan = if let Ok(task) = c.cmd("runtime.get", json!({ "id": id })).await {
        let spec = c.cmd("runtime.spec", json!({ "id": id })).await.unwrap_or_default();
        let report = c
            .cmd("share.preview", json!({ "id": id, "level": "report" }))
            .await
            .ok()
            .and_then(|p| p.pointer("/bundle/report").map(|r| repo::report_md(s(&task, "title"), r)))
            .unwrap_or_default();
        let prime = wisp_history_core::resume_prompt(s(&task, "title"), s(&spec, "prompt"), &report);
        let dir = open_cwd(s(&task, "cwd"), &spec)?;
        open_plan(s(&task, "agent"), task.get("resumeId").and_then(Value::as_str), asked, &dir, &prime)
    } else {
        let root = root(cwd);
        let all = tracked(&root);
        let t = find(&all, id)
            .ok_or_else(|| CliError::said(format!("no task {id}: not on this wisp, and not in {}'s .wisp/", root.display())))?;
        let (h, sid) = (s(&t.doc["session"], "harness").to_owned(), s(&t.doc["session"], "id").to_owned());
        let native = root.join(".wisp/history").join(format!("{}.native.jsonl.zst", t.id()));
        let mut restored = false;
        if !sid.is_empty()
            && let Ok(z) = std::fs::read(&native)
        {
            use base64::Engine as _;
            let raw = base64::engine::general_purpose::STANDARD.encode(repo::decompress(&z));
            restored = c
                .cmd("runtime.restoreSession", json!({ "harness": h, "id": sid, "cwd": root.display().to_string(), "raw": raw }))
                .await
                .is_ok_and(|r| r["restored"] == true);
        }
        let md = std::fs::read_to_string(root.join(t.path.replace(".task.yml", ".report.md"))).unwrap_or_default();
        let prime = wisp_history_core::resume_prompt(s(&t.doc, "title"), s(&t.doc["spec"], "prompt"), &md);
        let task_h = if h.is_empty() { s(&t.doc["spec"], "agent").to_owned() } else { h };
        let mut p = open_plan(&task_h, restored.then_some(sid.as_str()), asked, &root.display().to_string(), &prime);
        if restored && p.how == "resume" {
            "restore".clone_into(&mut p.how);
        }
        p
    };
    if has("--print") || o.json {
        say(&serde_json::to_string_pretty(&plan).unwrap_or_default());
        return Ok(0);
    }
    if !plan.note.is_empty() {
        say(&format!("  {} {}", plan.how, plan.note));
    }
    let started = now_ms();
    let status = terminal(&plan.program, &plan.args)
        .current_dir(&plan.cwd)
        .status()
        .map_err(|e| CliError::said(format!("{}: {e}", plan.program)))?;
    if has("--no-adopt") {
        return Ok(status.code().unwrap_or(0));
    }
    // what was done there comes back as the task's next turns (or, a continuation, as its own task)
    let session = match &plan.session {
        Some(sid) => Some(sid.clone()),
        None => c.cmd("runtime.sessions", json!({ "cwd": plan.cwd })).await.ok().and_then(|l| {
            l.as_array().and_then(|a| {
                a.iter()
                    .find(|x| s(x, "harness") == plan.harness && x["startedAt"].as_i64().unwrap_or(0) >= started - 1000)
                    .map(|x| s(x, "id").to_owned())
            })
        }),
    };
    if let Some(sid) = session {
        match c.cmd("runtime.adoptSession", json!({ "harness": plan.harness, "id": sid })).await {
            Ok(m) => say(&format!("  ✓ back in wisp: task {}", s(&m, "id"))),
            Err(e) => say(&format!("  (not adopted back: {e})")),
        }
    }
    Ok(status.code().unwrap_or(0))
}

#[cfg(test)]
#[path = "repo_cli_tests.rs"]
mod tests;
