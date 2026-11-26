//! `wisp blame` and `wisp tasks --touching` after perf audit 2026-11-19 #2 and #6: every commit's message from one
//! `git log`, the task files that list a commit indexed once, and only the files that name a path parsed. The same
//! answers as before, on a real repo.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use super::{adopt_hook, blame, codex_mcp_table, filtered, parse_bodies, root, tracked};

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

fn sha(dir: &Path) -> String {
    let o = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(dir).output().unwrap();
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn task(dir: &Path, id: &str, at: i64, files: &[&str], commits: &[&str]) {
    let list = |xs: &[&str]| if xs.is_empty() { " []".to_owned() } else { xs.iter().map(|x| ["\n  - ", x].concat()).collect::<String>() };
    let text = format!(
        "id: {id}\ntitle: Task {id}\ncreatedAt: {at}\nspec:\n  agent: claude\n  prompt: do {id}\nfiles:{}\ncommits:{}\n",
        list(files),
        list(commits)
    );
    let d = dir.join(".wisp/tasks/2026/11");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(format!("{id}.task.yml")), text).unwrap();
}

fn ids(b: &Value) -> Vec<String> {
    b["tasks"].as_array().unwrap().iter().map(|t| t["task"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn blame_names_each_commits_task_from_its_trailer_or_the_file_that_lists_it() {
    let dir = std::env::temp_dir().join(format!("wisp-blame-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    git(&dir, &["init", "-q"]);
    std::fs::write(dir.join("src/a.txt"), "one\ntwo\n").unwrap();
    git(&dir, &["add", "src/a.txt"]);
    git(&dir, &["commit", "-q", "-m", "add a\n\nWisp-Task: aaaaaaaa1111"]);
    std::fs::write(dir.join("src/a.txt"), "one\ntwo\nthree by hand\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "by hand"]);
    std::fs::write(dir.join("src/a.txt"), "one\ntwo\nthree by hand\nfour\nfive\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "backfilled: no trailer"]);
    let backfilled = sha(&dir);
    task(&dir, "aaaaaaaa1111", 1, &["src/a.txt"], &[]);
    task(&dir, "bbbbbbbb2222", 2, &["src/a.txt"], &[&backfilled]);
    task(&dir, "cccccccc3333", 3, &["docs/other.md"], &[]);
    let top = root(&dir.join("src"));
    #[cfg(unix)]
    assert_eq!(top, wisp_proc::canonical_blocking(&dir).unwrap(), "the repo's top, found from a folder in it");

    let file = blame(&top, "src/a.txt");
    assert_eq!(file["kind"], "file");
    assert_eq!((file["lines"].as_u64(), file["byHand"].as_u64()), (Some(5), Some(1)));
    assert_eq!(
        ids(&file),
        ["aaaaaaaa1111", "bbbbbbbb2222"],
        "2 lines from the trailer's commit, 2 from the backfilled one's (a tie: by id)"
    );
    let line = blame(&top, "src/a.txt:1");
    assert_eq!(ids(&line), ["aaaaaaaa1111"]);
    let folder = blame(&top, "src");
    let mut got = ids(&folder);
    got.sort();
    assert_eq!(got, ["aaaaaaaa1111", "bbbbbbbb2222"], "{folder}");
    let commit = blame(&top, &backfilled);
    assert_eq!(ids(&commit), ["bbbbbbbb2222"]);

    let touching: Vec<String> = filtered(&top, Some("src/a.txt"), None, None).iter().map(|t| t.id().to_owned()).collect();
    assert_eq!(touching, ["bbbbbbbb2222", "aaaaaaaa1111"], "newest first, only the tasks that name the path");
    assert_eq!(tracked(&top).len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_logs_records_split_into_each_commits_message() {
    let text = "abc\0first\n\nWisp-Task: t1\n\u{1e}\ndef\0second\n\u{1e}\n";
    let b = parse_bodies(text);
    assert_eq!(b.len(), 2);
    assert!(b["abc"].contains("Wisp-Task: t1"));
    assert_eq!(b["def"], "second\n");
}

/// `wisp skill install --agent codex` on Windows: the exe's path reads back from Codex's config.toml as itself.
#[test]
fn the_codex_mcp_table_keeps_a_windows_path() {
    use wisp_resources_core::formula::{Format, documents};
    for exe in
        [r"C:\Users\x\AppData\Local\wisp\wisp.exe", r"C:\Program Files\wisp\wisp.exe", "/usr/local/bin/wisp", r#"C:\it's "here"\wisp.exe"#]
    {
        let text = format!("model = \"o3\"\n{}", codex_mcp_table(exe));
        let doc = documents(&text, Format::Toml).unwrap_or_else(|e| panic!("{exe}: {e}\n{text}"));
        assert_eq!(doc[0]["mcp_servers"]["wisp"]["command"], exe, "{text}");
        assert_eq!(doc[0]["mcp_servers"]["wisp"]["args"], serde_json::json!(["mcp"]));
    }
}

/// The `SessionEnd` hook's command, run by `sh -c` (macOS, Linux) and PowerShell (Windows): a path with a space and one
/// with a quote come back as one word, the path as it is. `sh` really runs here; PowerShell's quoting is read back by
/// its rule (inside '…' only '' is special).
#[test]
fn the_adopt_hook_quotes_wisps_path_for_the_shell_that_runs_it() {
    for exe in ["/Users/John Smith/.local/bin/wisp", "/opt/it's here/wisp", "/usr/local/bin/wisp"] {
        let h = adopt_hook(exe, false);
        assert!(h.get("shell").is_none(), "sh -c, Claude Code's default off Windows");
        let cmd = h["command"].as_str().unwrap();
        assert!(cmd.ends_with(" adopt --hook"), "{cmd}");
        // the words sh splits it into: the path whole, then the two arguments
        let words = cmd.replacen(" adopt --hook", "", 1);
        let out = Command::new("sh").args(["-c", &format!("printf '%s\\n' {words} adopt --hook")]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), format!("{exe}\nadopt\n--hook\n"), "{cmd}");
    }
    for (exe, want) in [
        (r"C:\Program Files\wisp\wisp.exe", r"& 'C:\Program Files\wisp\wisp.exe' adopt --hook"),
        (r"C:\Users\O'Brien\wisp.exe", r"& 'C:\Users\O''Brien\wisp.exe' adopt --hook"),
        // PowerShell reads the curly quotes as quotes too: each is doubled, or `O’Brien` ended the string
        (r"C:\Users\O’Brien\wisp.exe", r"& 'C:\Users\O’’Brien\wisp.exe' adopt --hook"),
    ] {
        let h = adopt_hook(exe, true);
        assert_eq!((h["shell"].as_str(), h["command"].as_str()), (Some("powershell"), Some(want)));
        let inner = want.strip_prefix("& '").and_then(|c| c.strip_suffix("' adopt --hook")).unwrap();
        let back = ['\'', '‘', '’', '‚', '‛'].iter().fold(inner.to_owned(), |t, q| t.replace(&format!("{q}{q}"), &q.to_string()));
        assert_eq!(back, exe, "PowerShell reads the path back whole");
    }
}

/// Audit cli-r3: an editor that started `wisp mcp` before wisp listed 11 tools and never more. The list now reaches
/// for the running wisp (started if none answers) as a catalogue tool's call does, and initialize says the list can
/// change (a wisp reached later sends `notifications/tools/list_changed`).
#[test]
fn the_tool_list_starts_wisp_and_can_change() {
    use super::{Need, mcp_reply, needs_wisp};
    assert_eq!(needs_wisp("tools/list", ""), Need::Start);
    assert_eq!(needs_wisp("tools/call", "wisp_wait"), Need::Start, "a task tool");
    assert_eq!(needs_wisp("tools/call", "runtime_list"), Need::Start, "a catalogue tool");
    assert_eq!(needs_wisp("tools/call", "wisp_resume"), Need::Running);
    assert_eq!(needs_wisp("tools/call", "wisp_history"), Need::No, "the history tools need no wisp");
    assert_eq!(needs_wisp("ping", ""), Need::No);
    let init = mcp_reply(Path::new("/nowhere"), &serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" })).unwrap();
    assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], true, "{init}");
}
