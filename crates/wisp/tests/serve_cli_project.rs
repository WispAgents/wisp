//! Setup audit F4 (H): tasks started from the CLI inside a project folder got no project and no worktree, where the
//! same prompt from New task got both. The CLI's task is the project's, as the UI's is.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::{Value, json};

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .unwrap();
    assert!(ok.success(), "git {args:?}");
}

fn wisp(dir: &Path, cwd: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The task's fields that say where it runs.
async fn place(api: &Api, title: &str) -> Value {
    let list = api.cmd("runtime.list", json!({})).await;
    let t = list.as_array().unwrap().iter().find(|t| t["title"] == title).unwrap_or_else(|| panic!("no task {title}: {list}")).clone();
    // what was asked for: its spec (the task's own cwd is its worktree's, once it has one)
    api.cmd("runtime.get", json!({ "id": t["id"] })).await["ext"]["wisp.runtime"]["spec"].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_started_from_a_project_folder_is_that_project_s_as_new_task_s_is() {
    let dir = temp("serve-cli-project");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let repo = dir.join("repo2");
    std::fs::create_dir_all(repo.join("web")).unwrap();
    std::fs::write(repo.join("web/a.txt"), "a\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let (code, out, err) = wisp(&dir, &dir, port, &["add", repo.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let projects = api.cmd("resources.export", json!({ "kinds": ["project"] })).await;
    let path = projects[0]["spec"]["path"].as_str().unwrap().to_owned();

    // from the project's folder (the shell's cwd, macOS's /private/var/… spelling), and from a folder inside it
    let (code, out, err) = wisp(&dir, &repo, port, &["--no-open", "cli task here"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, &repo.join("web"), port, &["--no-open", "cli task in web"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, &repo, port, &["--no-open", "--no-worktree", "cli task in place"]);
    assert_eq!(code, 0, "{out}{err}");

    // the project's own path, as New task sends it, so the app files it under the project
    let here = place(&api, "cli task here").await;
    assert_eq!(here["cwd"], json!(path), "{here}");
    let web = place(&api, "cli task in web").await;
    assert_eq!(web["cwd"], json!(format!("{path}/web")), "{web}");
    // Worktree per task (on by default): its own worktree, as New task's; --no-worktree keeps it in place
    assert_eq!(here["worktree"], json!(true), "{here}");
    let in_place = place(&api, "cli task in place").await;
    assert_eq!(in_place["worktree"], json!(false), "{in_place}");
    // outside any project: as it was, no worktree asked for
    let (code, out, err) = wisp(&dir, &dir, port, &["--no-open", "cli task outside"]);
    assert_eq!(code, 0, "{out}{err}");
    let outside = place(&api, "cli task outside").await;
    assert!(outside["worktree"].is_null(), "{outside}");
    drop(serve);
}
