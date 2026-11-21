//! Audit F (2026-11-19), the CLI's words, end to end on a real `wisp serve` with the fake agent (nothing paid runs):
//!   - setup F1: with a built-in plugin off, `wisp settings get crew.tint` / `wisp learn view` launched a Full-auto
//!     task; they're refused with the plugin's state (wisp.settings itself now stays on: setup F7).
//!   - setup F5, F10, F11: `wisp update` never says "latest" unchecked; `wisp update view` needs no --peer;
//!     `wisp connectors remove <id>` takes the id as its word.
//!   - orchestration F2: a fork of a merged task (its worktree and branch gone) gets a folder of its own, fresh from the
//!     project, never the gone folder; a follow-up there says the branch is gone, in plain words.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, ended, start, temp};
use serde_json::json;

fn wisp(dir: &Path, cwd: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn an_off_plugins_cli_words_are_refused_never_a_task_audit_f_setup_f1() {
    let dir = temp("serve-auditF-off");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let here = dir.join("here");
    std::fs::create_dir_all(&here).unwrap();
    for id in ["wisp.learn", "wisp.budget"] {
        api.cmd("plugins.setEnabled", json!({ "id": id, "enabled": false })).await;
    }
    for (words, id) in [(&["learn", "view"][..], "wisp.learn"), (&["budget", "view", "please", "fix", "it"], "wisp.budget")] {
        let (code, out, err) = wisp(&dir, &here, port, words);
        assert_eq!(code, 2, "{words:?}: {out}{err}");
        assert!(!out.contains("launched") && err.contains(&format!("wisp plugin enable {id}")), "{words:?}: {out}{err}");
    }
    assert!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().is_empty(), "no task was started");
    // back on, the verbs answer again
    api.cmd("plugins.setEnabled", json!({ "id": "wisp.learn", "enabled": true })).await;
    let (code, out, err) = wisp(&dir, &here, port, &["learn", "view"]);
    assert_eq!(code, 0, "{out}{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_and_connectors_say_what_they_do_audit_f_setup_f5_f10_f11() {
    let dir = temp("serve-auditF-words");
    let (_serve, port) = start(&dir);
    let here = dir.join("here");
    std::fs::create_dir_all(&here).unwrap();
    // F5: a debug build reads as a git clone, which never checks
    let (code, out, err) = wisp(&dir, &here, port, &["update"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("is the latest") && out.contains("git clone"), "{out}{err}");
    // F10: this wisp's own, no --peer asked for
    let (code, out, err) = wisp(&dir, &here, port, &["update", "view"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("needs --peer"), "{err}");
    // F11: the id as its word (none such: refused for that, never "doesn't take")
    let (code, out, err) = wisp(&dir, &here, port, &["connectors", "remove", "nope"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(!err.contains("doesn't take"), "{err}");
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

#[tokio::test(flavor = "multi_thread")]
async fn a_merged_tasks_fork_and_follow_up_say_where_they_are_audit_f_orchestration_f2() {
    let dir = temp("serve-auditF-merged");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let repo = dir.join("proj");
    std::fs::create_dir_all(&repo).unwrap();
    let repo = wisp_proc::canonical_blocking(&repo).unwrap();
    std::fs::write(repo.join("math.js"), "export const add = (a, b) => a + b;\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let made = api.cmd("runtime.create", json!({ "prompt": "in a worktree", "cwd": repo, "agent": "claude", "worktree": true })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "its turn ends", |t| t.first().is_some_and(ended)).await;
    let task = api.cmd("runtime.get", json!({ "id": id })).await;
    let cwd = task["cwd"].as_str().unwrap().to_owned();
    assert_ne!(Path::new(&cwd), repo.as_path(), "it ran in a worktree");
    let branch = git_out(&repo, &["branch", "--format=%(refname:short)"]).lines().find(|b| b.starts_with("wisp/")).unwrap().to_owned();
    // merged: ship's tidy removes its worktree and deletes its branch
    git(&repo, &["worktree", "remove", "--force", &cwd]);
    git(&repo, &["branch", "-D", &branch]);

    let fork = api.cmd("runtime.fork", json!({ "id": id })).await;
    let fid = fork["id"].as_str().unwrap().to_owned();
    let fcwd = api.cmd("runtime.get", json!({ "id": fid })).await["cwd"].as_str().unwrap_or_default().to_owned();
    assert!(Path::new(&fcwd).is_dir(), "the fork has a folder of its own: {fcwd:?}");
    assert_ne!(fcwd, cwd, "never the merged task's gone worktree");

    api.cmd("runtime.followUp", json!({ "id": id, "text": "one more thing" })).await;
    let said = api.until(&id, "the follow-up is refused", |t| t.len() >= 2 && ended(&t[1])).await;
    let text = said[1].to_string();
    assert!(text.contains("is gone") && text.contains("merged"), "{text}");
    assert!(!text.contains("invalid reference"), "{text}");
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Audit I IS4 and IS10, end to end: `wisp git push origin main` launched a Full-auto task with that line as its prompt;
/// `wisp help reply` was "no command or plugin named reply"; `wisp import --help` read `--help` as its folder and
/// `wisp run --help`, `wisp open --help` … as their argument. Nothing launches, and each prints its usage.
#[tokio::test(flavor = "multi_thread")]
async fn gits_own_commands_are_refused_and_every_command_has_its_help_audit_i_is4_is10() {
    let dir = temp("serve-auditI-cli");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let before = api.cmd("runtime.list", json!({})).await.as_array().map_or(0, Vec::len);
    for line in [&["git", "push", "origin", "main"][..], &["git", "pull", "--rebase", "origin", "main"], &["git", "log", "--oneline", "-5"]]
    {
        let (code, _, err) = wisp(&dir, &proj, port, line);
        assert_eq!(code, 2, "{line:?}: {err}");
        assert!(err.contains(&format!("wisp git has no {}: that's git's own command", line[1])) && err.contains("wisp -- \""), "{err}");
    }
    for cmd in [
        "reply",
        "ls",
        "import",
        "run",
        "show",
        "export",
        "open",
        "resume",
        "track",
        "adopt",
        "untrack",
        "blame",
        "init",
        "secret",
        "workspace",
        "log",
    ] {
        let (code, out, err) = wisp(&dir, &proj, port, &["help", cmd]);
        assert_eq!(code, 0, "wisp help {cmd}: {err}");
        assert!(
            out.contains(&format!("wisp {cmd}")) || out.contains(&format!("{cmd}|")) || out.contains(&format!("|{cmd}")),
            "wisp help {cmd}: {out}"
        );
        let (code, out2, err) = wisp(&dir, &proj, port, &[cmd, "--help"]);
        assert_eq!(code, 0, "wisp {cmd} --help: {err}");
        assert!(
            !err.contains("--help") && out2.contains(&format!("wisp {cmd}"))
                || out2.contains(&format!("{cmd}|"))
                || out2.contains(&format!("|{cmd}")),
            "wisp {cmd} --help: {out2}{err}"
        );
    }
    let after = api.cmd("runtime.list", json!({})).await.as_array().map_or(0, Vec::len);
    assert_eq!(after, before, "nothing launched");
}
