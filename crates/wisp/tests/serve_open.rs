//! `wisp open <task>` (ADR-0033, Open in), end to end: the command line it would run (`--print`), resuming
//! a task's own session in its CLI and adopting back what was done there, `--no-adopt`, a continuation in
//! another harness, an unknown task, a task from the repo whose native log is restored on another wisp, and
//! a task whose folder is gone opening at its base commit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, ended, script, start_with, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, home: &str, cwd: &Path, port: u16, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join(home))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env("CLAUDE_CONFIG_DIR", dir.join("claude-cfg"))
        .envs(env.iter().copied())
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

fn user(text: &str, at: &str) -> Value {
    json!({ "type": "user", "message": { "role": "user", "content": text }, "timestamp": at })
}

fn said(text: &str, at: &str) -> Value {
    json!({ "type": "assistant", "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] }, "timestamp": at })
}

#[tokio::test(flavor = "multi_thread")]
async fn open_resumes_restores_continues_and_adopts_back() {
    let dir = temp("serve-open");
    let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let cwd = repo.display().to_string();
    let store =
        dir.join("claude-cfg/projects").join(cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>());
    std::fs::create_dir_all(&store).unwrap();
    // the interactive CLI: says how it was run, and (resumed) adds a turn to its session log, as Claude Code does
    let log = dir.join("cli.log");
    let turn = format!("{}\n{}", user("one more from the CLI", "2026-11-09T10:00:00.000Z"), said("Did it.", "2026-11-09T10:00:05.000Z"));
    // Windows: its turn comes from a file (cmd's echo can't print JSON safely)
    std::fs::write(dir.join("cli-turn.jsonl"), format!("{turn}\n")).unwrap();
    let script = script(
        &dir.join("claude-cli"),
        &format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$1\" = \"--resume\" ]; then printf '%s\\n' '{}' >> \"$STORE/$2.jsonl\"; fi\n",
            log.display(),
            turn.replace('\'', "'\\''")
        ),
        &format!(
            "{}>> \"{}\" echo(%A1% %A2%\nif \"%A1%\"==\"--resume\" type \"{}\" >> \"%STORE%\\%A2%.jsonl\"\n",
            common::CMD_ARGS,
            log.display(),
            dir.join("cli-turn.jsonl").display()
        ),
    );
    let cli: Vec<(&str, &str)> = vec![
        ("WISP_CLAUDE_BIN", script.to_str().unwrap()),
        ("WISP_CODEX_BIN", script.to_str().unwrap()),
        ("STORE", store.to_str().unwrap()),
    ];
    let cfg = dir.join("claude-cfg");
    let (serve, port) = start_with(&dir, &[("CLAUDE_CONFIG_DIR", cfg.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    assert_eq!(wisp(&dir, "home", &repo, port, &["init"], &[]).0, 0);

    let made = api
        .cmd("runtime.create", json!({ "prompt": "fix the add bug TOPIC[Fix add]", "cwd": repo, "agent": "claude", "worktree": false }))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    let sid = api.cmd("runtime.get", json!({ "id": id })).await["resumeId"].as_str().unwrap().to_owned();
    // Claude Code keeps the run's session log (wisp's run was a Claude Code session)
    std::fs::write(
        store.join(format!("{sid}.jsonl")),
        format!("{}\n{}\n", user("fix the add bug", "2026-11-09T09:00:00.000Z"), said("Fixed.", "2026-11-09T09:00:05.000Z")),
    )
    .unwrap();

    // open_print_gives_the_command_line
    let (code, out, err) = wisp(&dir, "home", &repo, port, &["open", &id, "--print"], &cli);
    assert_eq!(code, 0, "{out}{err}");
    let plan: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (plan["how"].clone(), plan["args"].clone(), plan["cwd"].clone()),
        (json!("resume"), json!(["--resume", sid]), json!(cwd)),
        "{plan}"
    );
    assert!(!log.exists(), "--print runs nothing");

    // open_resumes_its_own_session, open_adopts_back_after
    let (code, out, err) = wisp(&dir, "home", &repo, port, &["open", &id], &cli);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), format!("--resume {sid}"));
    let turns = api.turns(&id).await;
    assert_eq!((turns.len(), turns[1]["prompt"].clone()), (2, json!("one more from the CLI")), "back in the same task");
    // open_no_adopt_leaves_it
    let (code, _, _) = wisp(&dir, "home", &repo, port, &["open", &id, "--no-adopt"], &cli);
    assert_eq!(code, 0);
    assert_eq!(api.turns(&id).await.len(), 2, "not adopted");

    // open_in_another_harness_primes_a_continuation
    let (_, out, _) = wisp(&dir, "home", &repo, port, &["open", &id, "--in", "codex", "--print"], &cli);
    let plan: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(plan["how"], "continue");
    let prime = plan["args"][0].as_str().unwrap();
    assert!(
        prime.contains("You are continuing a task")
            && prime.contains("Fix add")
            && plan["note"].as_str().unwrap().contains("not the same session"),
        "{plan}"
    );

    // open_an_unknown_task_says_so
    let (code, _, err) = wisp(&dir, "home", &repo, port, &["open", "zzzzzzzz9"], &cli);
    assert_eq!(code, 1);
    assert!(err.contains("no task zzzzzzzz9"), "{err}");

    // open_restores_the_native_log_for_a_repo_task: tracked (with its native log), then opened on another
    // wisp, where Claude Code doesn't have it
    let (code, out, err) = wisp(&dir, "home", &repo, port, &["track", &id], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(repo.join(format!(".wisp/history/{id}.native.jsonl.zst")).is_file(), "{out}");
    std::fs::remove_file(store.join(format!("{sid}.jsonl"))).unwrap();
    serve.stop();
    let other = temp("serve-open-other");
    let (serve, port) = start_with(&other, &[("CLAUDE_CONFIG_DIR", cfg.to_str().unwrap())]);
    let (code, out, err) =
        wisp(&other, "home", &repo, port, &["open", &id[..12], "--print"], &[("CLAUDE_CONFIG_DIR", cfg.to_str().unwrap())]);
    assert_eq!(code, 0, "{out}{err}");
    let plan: Value = serde_json::from_str(&out).unwrap();
    assert_eq!((plan["how"].clone(), plan["args"].clone()), (json!("restore"), json!(["--resume", sid])), "{plan}");
    let back = std::fs::read_to_string(store.join(format!("{sid}.jsonl"))).unwrap();
    assert!(back.contains("one more from the CLI"), "the log as it was: {back}");
    drop(serve);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_gone_opens_at_its_base() {
    let dir = temp("serve-open-base");
    let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let rec = dir.join("rec");
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = api.cmd("runtime.create", json!({ "prompt": "fix it", "cwd": repo, "agent": "claude", "worktree": true })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    let tree = api.cmd("runtime.get", json!({ "id": id })).await["cwd"].as_str().unwrap().to_owned();
    assert_ne!(tree, repo.display().to_string(), "it ran in a worktree");
    // the turn ends on the agent's answer, and its process exits just after; on Windows a folder can't be removed
    // while a process works in it ("being used by another process", os error 32), as a user's `rm` after the task
    // wouldn't be either: the folder goes once the run is gone
    for r in common::records(&rec.join("claude.jsonl")) {
        let pid = u32::try_from(r["pid"].as_u64().unwrap()).unwrap();
        let at = std::time::Instant::now();
        while wisp_proc::pid_alive(pid) {
            assert!(at.elapsed() < std::time::Duration::from_secs(20), "the run (pid {pid}) never exited");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
    // CI 8bb8ccd0 and 9ea69e14 (Windows): os error 32 with the run's CLI gone and no process of wisp serve's alive a
    // moment later: the turn's end sets off wisp's own work in the task's folder (ship's `TurnEnded` recheck runs
    // `git status` there, off the event's way), and Windows refuses to remove a folder that is any process's cwd. So
    // the folder goes as a user's folder does: while wisp is stopped (its nursery job ends every git it started), and
    // the task opens on the next wisp. If it is still held then, it isn't wisp's: the dump names the holder.
    let serve_pid = serve.child.id();
    serve.stop();
    // the server is gone once `wait` returns; what it started ends with its job a moment later (a kill on Windows
    // is asynchronous): wait for that, not for time
    let at = std::time::Instant::now();
    while let Some(left) = started_by(serve_pid) {
        assert!(at.elapsed() < std::time::Duration::from_secs(20), "wisp serve's {left:?} outlived it");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    if let Err(e) = std::fs::remove_dir_all(&tree) {
        panic!("the task's folder is held with wisp stopped: {e}\n{}", holders(Path::new(&tree), serve_pid, &dir));
    }
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let (code, out, err) = wisp(&dir, "home", &repo, port, &["open", &id, "--print"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let plan: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
    let at = plan["cwd"].as_str().unwrap();
    assert!(at.contains("wisp-open-") && Path::new(at).join("math.js").is_file(), "a fresh worktree at the base: {plan}");
    drop(serve);
}

/// The live processes `root` started, and theirs (by parent pid, which a process keeps after its parent is gone);
/// `None` when there are none.
fn started_by(root: u32) -> Option<Vec<u32>> {
    let all = wisp_proc::processes().unwrap_or_default();
    let mut tree = vec![root];
    let mut i = 0;
    while let Some(&p) = tree.get(i) {
        let kids: Vec<u32> = all.iter().filter(|c| c.ppid == p && !c.zombie && !tree.contains(&c.pid)).map(|c| c.pid).collect();
        tree.extend(kids);
        i += 1;
    }
    tree.remove(0);
    (!tree.is_empty()).then_some(tree)
}

/// Who holds `tree`, for the panic (wisp serve already stopped): on Windows, the processes Restart Manager says hold a
/// file in it (or that none does: then a process has the folder itself open, as a cwd), what is still alive under this
/// test and under the stopped server, every git, wisp, harness or console process alive anywhere, and the command
/// lines that name the folder; elsewhere `ps`. And what the server logged last.
fn holders(tree: &Path, serve: u32, dir: &Path) -> String {
    let o = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", HOLDERS_PS1])
            .env("WISP_HELD", tree)
            .env("WISP_HELD_SERVE", serve.to_string())
            .env("WISP_HELD_TEST", std::process::id().to_string())
            .output()
    } else {
        std::process::Command::new("ps").args(["-eo", "pid,ppid,command"]).output()
    };
    let list = o.map_or_else(
        |e| format!("(no process list: {e})"),
        |o| format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
    );
    let log = std::fs::read_to_string(dir.join("serve.log")).unwrap_or_default();
    let tail: Vec<&str> = log.lines().rev().take(40).collect();
    format!("{list}\nwisp serve's log, last {} lines:\n{}", tail.len(), tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

/// Restart Manager (`RmGetList`) over every file left in the folder, the process tree under this test, wisp serve's
/// open handle count, and the processes whose command line names the folder.
const HOLDERS_PS1: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$rmError = $null
try { Add-Type -ErrorAction Stop -TypeDefinition @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class WispRm {
  [StructLayout(LayoutKind.Sequential)] struct Uniq { public int Pid; public System.Runtime.InteropServices.ComTypes.FILETIME Start; }
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct Info {
    public Uniq P; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string App;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string Svc; public int Type; public int Status; public int Session;
    [MarshalAs(UnmanagedType.Bool)] public bool Restartable; }
  [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)] static extern int RmStartSession(out uint h, int f, string key);
  [DllImport("rstrtmgr.dll")] static extern int RmEndSession(uint h);
  [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)] static extern int RmRegisterResources(uint h, uint n, string[] files, uint a, IntPtr apps, uint s, string[] svcs);
  [DllImport("rstrtmgr.dll")] static extern int RmGetList(uint h, out uint needed, ref uint n, [In, Out] Info[] info, ref uint reasons);
  public static string Who(string[] files) {
    uint h; if (RmStartSession(out h, 0, Guid.NewGuid().ToString()) != 0) return "(no Restart Manager session)";
    try {
      if (RmRegisterResources(h, (uint)files.Length, files, 0, IntPtr.Zero, 0, null) != 0) return "(Restart Manager refused the files)";
      uint needed = 0, n = 0, why = 0;
      int r = RmGetList(h, out needed, ref n, null, ref why);
      if (r == 0) return "";
      if (r != 234) return "(RmGetList " + r + ")";
      var info = new Info[needed]; n = needed;
      r = RmGetList(h, out needed, ref n, info, ref why);
      if (r != 0) return "(RmGetList " + r + ")";
      var s = new List<string>(); for (int i = 0; i < n; i++) s.Add(info[i].P.Pid + " " + info[i].App);
      return string.Join("; ", s);
    } finally { RmEndSession(h); }
  }
}
'@ } catch { $rmError = $_.Exception.Message }
$tree = $env:WISP_HELD; $serve = [int]$env:WISP_HELD_SERVE; $me = [int]$env:WISP_HELD_TEST
$left = @(Get-ChildItem -LiteralPath $tree -Recurse -Force)
$files = @($left | Where-Object { -not $_.PSIsContainer } | ForEach-Object { $_.FullName })
"left in the folder: $($left.Count) entries, $($files.Count) files"
$left | Select-Object -First 30 | ForEach-Object { "  $($_.FullName)" }
if ($rmError) { "(Restart Manager unavailable: $rmError)" }
elseif ($files.Count -eq 0) { 'no file left in it: the holder has the folder itself open (a cwd, an open directory)' }
else {
  try { $who = [WispRm]::Who($files) } catch { $who = "(Restart Manager failed: $($_.Exception.Message))" }
  if ($who) { "holding a file in it (Restart Manager): $who" } else { 'no process holds a file in it (Restart Manager): the holder has the folder itself open (a cwd, an open directory)' }
}
$all = @(Get-CimInstance Win32_Process)
function Under($p, $d) { foreach ($c in @($all | Where-Object { $_.ParentProcessId -eq $p })) { ('  ' * $d) + "$($c.ProcessId) $($c.CommandLine)"; Under $c.ProcessId ($d + 1) } }
"alive under this test (pid $me), this query included:"
Under $me 1
$s = Get-Process -Id $serve
if ($s) { "wisp serve (pid $serve): $($s.HandleCount) handles open" } else { "wisp serve (pid $serve) is stopped" }
"started by wisp serve (pid $serve), still alive:"
Under $serve 1
$tools = @($all | Where-Object { $_.Name -match '^(git|wisp|claude|codex|opencode|gh|conhost|OpenConsole)(\.exe)?$' -and $_.ProcessId -ne $me })
if ($tools.Count) { 'git, wisp, harness and console processes alive anywhere:'; $tools | ForEach-Object { "  $($_.ProcessId) (parent $($_.ParentProcessId)) $($_.CommandLine)" } } else { 'no git, wisp, harness or console process is alive anywhere' }
$named = @($all | Where-Object { $_.CommandLine -and $_.CommandLine.IndexOf($tree, [StringComparison]::OrdinalIgnoreCase) -ge 0 })
if ($named.Count) { 'naming the folder:'; $named | ForEach-Object { "  $($_.ProcessId) (parent $($_.ParentProcessId)) $($_.CommandLine)" } } else { 'no process names the folder in its command line' }
"#;
