//! Software factories (ADR-0114, owner-list row 65), end to end on `wisp serve` with the fake harness, the fake gh and
//! real git in a temp repo with an `origin`:
//! - a task through the built-in `default` factory: the spec, approved by you; the build in its own worktree; the
//!   project's test failing once, so the build is sent back with the output (round 2), then passing; the reviewer's
//!   APPROVE; the PR opened by the fake gh; and the merge after your yes;
//! - a factory's names are references (`wisp validate`), and `wisp "…" --factory` names what there is;
//! - `wisp mcp`'s `wisp_launch` takes `factory`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)] // one walk through a run

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Api, start_with, temp, yaml_str};
use serde_json::{Value, json};

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// `proj` as a repo with one commit, pushed to a bare `origin` beside it.
fn repo(dir: &Path) -> PathBuf {
    let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let origin = dir.join("origin.git");
    git(dir, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    repo
}

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env("COLUMNS", "200")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The run's view on its task (`ext["wisp.factory"]`), once `f` holds it.
async fn until_run(api: &Api, id: &str, what: &str, f: impl Fn(&Value) -> bool) -> Value {
    let at = Instant::now();
    loop {
        let t = api.cmd("runtime.get", json!({ "id": id })).await;
        let v = t["ext"]["wisp.factory"].clone();
        if f(&v) {
            return v;
        }
        assert!(at.elapsed() < Duration::from_secs(90), "{what}: {v:#}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn station<'a>(run: &'a Value, id: &str) -> &'a Value {
    run["stations"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("no {id}: {run:#}"))
}

fn waits_at(run: &Value, s: &str) -> bool {
    run["waiting"]["station"] == s && run["waiting"]["askId"].as_str().is_some_and(|a| !a.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_goes_through_the_default_factory_end_to_end() {
    let dir = temp("serve-factory");
    let repo = repo(&dir);
    let bin = dir.join("ghbin");
    common::install_fakes(&bin, &["gh"]);
    let state = dir.join("gh-state.json");
    std::fs::write(&state, json!({ "prs": {} }).to_string()).unwrap();
    let (serve, port) = start_with(
        &dir,
        &[
            ("WISP_GH_BIN", bin.join("gh").to_str().unwrap()),
            ("FAKE_GH_STATE", state.to_str().unwrap()),
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
            // the spec station writes a spec, the reviewer approves: what the fake says for those prompts
            ("FAKE_SAY", "You are the spec station=>SPEC: add(a, b) returns a + b, with a test;;You are a reviewer=>Looks right.\nAPPROVE"),
        ],
    );
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    // the project's test fails the first time it runs (a mark outside the worktree), then passes
    let mark = dir.join("checked-once");
    let test = format!("[ -f '{0}' ] || {{ touch '{0}'; echo '1 failing: add(1, 2) is -1'; exit 1; }}", mark.display());
    let r = api
        .try_cmd("resources.apply", json!({ "resources": [{ "kind": "project", "name": "app", "spec": { "path": repo, "test": test } }] }))
        .await;
    assert_eq!(r["ok"], true, "{r}");

    let made = api
        .cmd(
            "runtime.create",
            json!({ "prompt": "Fix the add bug", "cwd": repo, "agent": "claude", "worktree": true, "factory": "default" }),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let parent = api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!(parent["ext"]["wisp.runtime"]["spec"]["factory"], "default", "the run keeps its factory: {parent:#}");

    // 1. the spec, written read-only, waits for your yes
    let run = until_run(&api, &id, "the spec waits for your yes", |v| waits_at(v, "spec")).await;
    assert_eq!(run["state"], "waiting");
    assert!(station(&run, "spec")["output"].as_str().unwrap().contains("SPEC: add(a, b) returns a + b"), "{run:#}");
    let spec_task = station(&run, "spec")["tasks"][0].as_str().unwrap().to_owned();
    let st = api.cmd("runtime.get", json!({ "id": spec_task })).await;
    assert_eq!((st["mode"].as_str(), st["parentId"].as_str()), (Some("plan"), Some(id.as_str())), "a child, read-only: {st:#}");
    assert!(
        parent["queue"].as_array().is_none_or(Vec::is_empty) || api.cmd("runtime.get", json!({ "id": id })).await["queue"] == json!([])
    );
    let ask = run["waiting"]["askId"].as_str().unwrap().to_owned();
    api.cmd("runtime.answer", json!({ "id": id, "text": "Approve", "askId": ask })).await;

    // 2.-5. build, the check failing once (back to build as round 2), the review approving, the PR open: the merge asks
    let run =
        until_run(&api, &id, "the PR is open and the merge waits for your yes", |v| waits_at(v, "ship") || v["state"] == "failed").await;
    assert_eq!(run["state"], "waiting", "{run:#}");
    assert_eq!(run["round"], 2, "the failing check sent it back once: {run:#}");
    let build = station(&run, "build");
    assert_eq!(build["state"], "done");
    let build_task = build["tasks"][0].as_str().unwrap().to_owned();
    let bt = api.cmd("runtime.get", json!({ "id": build_task })).await;
    assert!(bt["ext"]["wisp.worktrees"]["path"].is_string(), "built in its own worktree: {bt:#}");
    let turns = api.cmd("runtime.turns", json!({ "id": build_task })).await;
    let turns = turns.as_array().unwrap();
    assert_eq!(turns.len(), 2, "round 2 is a follow-up to the same builder");
    let again = turns[1]["prompt"].as_str().unwrap();
    assert!(again.starts_with("Round 2: the checks failed.") && again.contains("1 failing: add(1, 2) is -1"), "{again}");
    let first = turns[0]["prompt"].as_str().unwrap();
    assert!(first.contains("The spec:") && first.contains("SPEC: add(a, b) returns a + b"), "the build got the spec: {first}");
    let checks = station(&run, "checks");
    assert_eq!((checks["state"].as_str(), checks["output"].as_str()), (Some("done"), Some("1 of 1 passed")), "{run:#}");
    let review = station(&run, "review");
    assert_eq!((review["state"].as_str(), review["output"].as_str()), (Some("done"), Some("1 of 1 approve")), "{run:#}");
    let rt = api.cmd("runtime.get", json!({ "id": review["tasks"][0] })).await;
    assert_eq!(
        (rt["mode"].as_str(), rt["cwd"].as_str()),
        (Some("plan"), bt["cwd"].as_str()),
        "the reviewer reads the build's worktree: {rt:#}"
    );
    let pr = run["pr"].as_str().unwrap().to_owned();
    assert_eq!(pr, "https://github.com/acme/repo/pull/1");
    let gh: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(gh["prs"]["1"]["state"], "OPEN", "{gh:#}");
    assert!(gh["prs"]["1"]["body"].as_str().unwrap().contains("Made by the wisp factory `default` in 2 round(s)."), "{gh:#}");

    // 6. your yes merges it, and the run is done
    let ask = run["waiting"]["askId"].as_str().unwrap().to_owned();
    api.cmd("runtime.answer", json!({ "id": id, "text": "Merge", "askId": ask })).await;
    let run = until_run(&api, &id, "merged and done", |v| matches!(v["state"].as_str(), Some("done" | "failed"))).await;
    assert_eq!((run["state"].as_str(), station(&run, "ship")["state"].as_str()), (Some("done"), Some("done")), "{run:#}");
    let gh: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(gh["prs"]["1"]["state"], "MERGED", "{gh:#}");
    // the list row says where it stands
    let rows = api.cmd("runtime.list", json!({})).await;
    let row = rows.as_array().unwrap().iter().find(|r| r["id"] == id.as_str()).unwrap();
    assert_eq!((row["factory"]["name"].as_str(), row["factory"]["state"].as_str()), (Some("default"), Some("done")), "{row:#}");
    // the run's own task never runs an agent: a follow-up typed there is refused, saying where the work goes
    let r = api.try_cmd("runtime.followUp", json!({ "id": id, "text": "one more thing" })).await;
    let turns = api.until(&id, "the follow-up's turn ends", |t| t.last().is_some_and(common::ended)).await;
    let last = turns.last().unwrap();
    assert_ne!(last["status"], "done", "no agent ran: {r} {last:#}");
    assert!(last.to_string().contains("factory run"), "it says why: {last:#}");
    // `wisp show` prints the stations
    let (code, out, err) = wisp(&dir, port, &["show", &id]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("default: ✓ spec · ✓ build · ✓ checks · ✓ review · ✓ ship"), "{out}");
    drop(serve);
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_names_are_references_and_validate_names_what_exists() {
    let dir = temp("serve-factory-refs");
    let (serve, port) = start_with(&dir, &[]);
    let proj = dir.join("proj").display().to_string();
    std::fs::write(
        dir.join("f.yml"),
        format!(
            "wisp: 1\nagents:\n  critic: {{ harness: codex }}\nfactories:\n  careful:\n    stations:\n      build: {{ agent: claude }}\n      checks: {{ run: [npm test] }}\n      review: {{ agents: [critic] }}\n    loop: {{ max: 2, then: stop }}\ntasks:\n  fix: {{ prompt: fix it, factory: careful, project: {} }}\n  other: {{ prompt: x, factory: default }}\n",
            yaml_str(&proj)
        ),
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "f.yml"]);
    assert_eq!(code, 0, "a task naming a declared factory or the built-in default is fine: {out}{err}");
    std::fs::write(dir.join("bad.yml"), "wisp: 1\nfactories:\n  careful:\n    stations:\n      build: { agent: critc }\n      deploy: {}\ntasks:\n  fix: { prompt: x, factory: carefull }\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "bad.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("deploy"), "an unknown station: {out}{err}");
    std::fs::write(
        dir.join("bad2.yml"),
        "wisp: 1\nfactories:\n  careful:\n    stations:\n      build: { agent: critc }\ntasks:\n  fix: { prompt: x, factory: carefull }\n",
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "bad2.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    let said = format!("{out}{err}");
    assert!(said.contains("no factory named carefull"), "{said}");
    assert!(said.contains("no agent named critc"), "{said}");
    // a launch naming no factory is refused before anything starts, saying which there are
    let (code, out, err) = wisp(&dir, port, &["fix it", "--factory", "nope", "--cwd", &proj, "--no-worktree"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("no factory named nope: factories has default"), "{out}{err}");
    // a task file's formula declares the factory it runs through, nothing applied (ADR-0111)
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::write(
        dir.join("fix.task.yml"),
        "wisp: 1\ntask: { prompt: tidy the tests, factory: careful, worktree: false }\nformula: ./f.yml\n",
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "-f", "fix.task.yml"]);
    assert_eq!(code, 0, "{out}{err}");
    let id = out.split_whitespace().skip_while(|w| *w != "attach").nth(1).unwrap().to_owned();
    let run = until_run(&api, &id, "the formula's factory runs it", |v| v["stations"].is_array()).await;
    assert_eq!(run["factory"], "careful");
    assert_eq!(
        run["stations"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect::<Vec<_>>(),
        ["build", "checks", "review"]
    );
    assert_eq!(api.cmd("factory.names", json!({})).await.as_array().unwrap().len(), 1, "nothing applied: only the built-in default");
    api.cmd("factory.stop", json!({ "id": id })).await;
    std::fs::write(dir.join("typo.task.yml"), "wisp: 1\ntask: { prompt: x, factory: carefull }\nformula: ./f.yml\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "-f", "typo.task.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("no factory named carefull"), "{out}{err}");
    drop(serve);
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_launch_runs_a_task_through_a_factory() {
    let dir = temp("serve-factory-mcp");
    let (serve, port) = start_with(&dir, &[("FAKE_SAY", "You are the spec station=>SPEC: the add fix")]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj").display().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .arg("mcp")
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }))
            .unwrap();
        let launch = json!({ "prompt": "fix the add bug", "cwd": proj, "worktree": false, "factory": "default" });
        writeln!(
            input,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "wisp_launch", "arguments": launch } })
        )
        .unwrap();
    }
    let answers: Vec<Value> =
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let said = answers.iter().find(|a| a["id"] == 2).unwrap_or_else(|| panic!("{answers:?}"))["result"]["content"][0]["text"].clone();
    let id =
        said.as_str().unwrap().strip_prefix("launched ").and_then(|r| r.split(':').next()).unwrap_or_else(|| panic!("{said}")).to_owned();
    let run = until_run(&api, &id, "the run waits on the spec", |v| waits_at(v, "spec")).await;
    assert_eq!(run["factory"], "default");
    // Stop ends it, its stations with it
    api.cmd("factory.stop", json!({ "id": id })).await;
    let run = until_run(&api, &id, "stopped", |v| v["state"] == "stopped").await;
    assert_eq!(run["why"], "you stopped it");
    drop(serve);
}

/// A server for one repo with a project, the given factories applied, and more variables for it.
async fn with_factories(name: &str, env: &[(&str, &str)], factories: Value) -> (PathBuf, PathBuf, common::Serve, Api) {
    let dir = temp(name);
    let repo = repo(&dir);
    let mut vars = vec![("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
    vars.extend_from_slice(env);
    let (serve, port) = start_with(&dir, &vars);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let mut resources = vec![json!({ "kind": "project", "name": "app", "spec": { "path": repo } })];
    for (name, spec) in factories.as_object().unwrap() {
        resources.push(json!({ "kind": "factory", "name": name, "spec": spec }));
    }
    let r = api.try_cmd("resources.apply", json!({ "resources": resources })).await;
    assert_eq!(r["ok"], true, "{r}");
    (dir, repo, serve, api)
}

fn alive(pid: &str) -> bool {
    Command::new("kill").args(["-0", pid.trim()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Review r194 factories #3: the checks run the built code with wisp's environment minus its own variables, tokens,
/// secrets and harness keys; when the build didn't run in Full auto, the run asks before running them.
#[tokio::test(flavor = "multi_thread")]
async fn checks_ask_first_after_a_build_that_needed_approvals_and_never_see_wisps_secrets() {
    let dir0 = temp("serve-factory-env-files");
    let envfile = dir0.join("env.txt");
    let check = format!("env > '{}'", envfile.display());
    let (_dir, repo, serve, api) = with_factories(
        "serve-factory-env",
        &[("MY_API_KEY", "sekrit-key"), ("GITHUB_TOKEN", "gh-sekrit"), ("PLAIN_SETTING", "kept")],
        json!({ "gate": { "stations": { "build": {}, "checks": { "run": [check] } } } }),
    )
    .await;
    let made =
        api.cmd("runtime.create", json!({ "prompt": "Fix it", "cwd": repo, "agent": "claude", "mode": "edits", "factory": "gate" })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    let run = until_run(&api, &id, "the checks ask first", |v| waits_at(v, "checks")).await;
    assert_eq!(run["waiting"]["question"].as_str(), Some(format!("Run `{check}` on the built code?").as_str()), "{run:#}");
    assert!(!envfile.exists(), "nothing ran before your yes");
    api.cmd("runtime.answer", json!({ "id": id, "text": "Run", "askId": run["waiting"]["askId"] })).await;
    let run = until_run(&api, &id, "done", |v| matches!(v["state"].as_str(), Some("done" | "failed"))).await;
    assert_eq!(run["state"], "done", "{run:#}");
    let env = std::fs::read_to_string(&envfile).unwrap();
    for gone in ["WISP_HOME", "WISP_CLAUDE_BIN", "MY_API_KEY", "GITHUB_TOKEN", "sekrit"] {
        assert!(!env.contains(gone), "{gone} reached the check:\n{env}");
    }
    assert!(env.contains("PLAIN_SETTING=kept") && env.contains("PATH="), "{env}");
    drop(serve);
}

/// Review r194 factories #4: Stop kills a running check's process at once (it waited out the check, up to 10 minutes)
/// and stops every station; deleting a run takes its stations' tasks and their worktrees with it.
#[tokio::test(flavor = "multi_thread")]
async fn stop_and_delete_kill_a_running_check_and_take_the_stations_with_them() {
    let pids = temp("serve-factory-stop-files");
    // the build's edit set aside first, so its worktree holds no work and a delete may take it (one with work stays)
    let check = |n: &str| format!("git stash -u -q; echo $$ > '{}'; exec sleep 60", pids.join(n).display());
    let (_dir, repo, serve, api) = with_factories(
        "serve-factory-stop",
        &[],
        json!({ "slow": { "stations": { "build": {}, "checks": { "run": [check("a")] } } }, "slower": { "stations": { "build": {}, "checks": { "run": [check("b")] } } } }),
    )
    .await;
    // Stop: the check's process dies now, the run ends stopped
    let id = api
        .cmd("runtime.create", json!({ "prompt": "Fix it", "cwd": repo, "agent": "claude", "worktree": true, "factory": "slow" }))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    until_run(&api, &id, "the check runs", |v| v["station"] == "checks").await;
    let at = Instant::now();
    while !pids.join("a").exists() {
        assert!(at.elapsed() < Duration::from_secs(20), "the check never started");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let pid = std::fs::read_to_string(pids.join("a")).unwrap();
    assert!(alive(&pid));
    let at = Instant::now();
    api.cmd("factory.stop", json!({ "id": id })).await;
    let run = until_run(&api, &id, "stopped", |v| v["state"] == "stopped").await;
    assert!(at.elapsed() < Duration::from_secs(5), "Stop waited on the check: {:?}", at.elapsed());
    assert_eq!(run["why"], "you stopped it");
    let at = Instant::now();
    while alive(&pid) {
        assert!(at.elapsed() < Duration::from_secs(5), "the check's process outlived Stop");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // delete: the check dies, the stations' tasks and the build's worktree go
    let id = api
        .cmd("runtime.create", json!({ "prompt": "Fix it again", "cwd": repo, "agent": "claude", "worktree": true, "factory": "slower" }))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let run = until_run(&api, &id, "the check runs", |v| v["station"] == "checks").await;
    let build = station(&run, "build")["tasks"][0].as_str().unwrap().to_owned();
    let tree = api.cmd("runtime.get", json!({ "id": build })).await["ext"]["wisp.worktrees"]["path"].as_str().unwrap().to_owned();
    assert!(Path::new(&tree).is_dir());
    let at = Instant::now();
    while !pids.join("b").exists() {
        assert!(at.elapsed() < Duration::from_secs(20), "the check never started");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let pid = std::fs::read_to_string(pids.join("b")).unwrap();
    api.cmd("runtime.delete", json!({ "id": id })).await;
    let at = Instant::now();
    loop {
        let gone = api.try_cmd("runtime.get", json!({ "id": build })).await["ok"] == false;
        if gone && !alive(&pid) && !Path::new(&tree).exists() {
            break;
        }
        assert!(
            at.elapsed() < Duration::from_secs(20),
            "after the delete: station gone {gone}, check alive {}, worktree there {}",
            alive(&pid),
            Path::new(&tree).exists()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(serve);
    let _ = std::fs::remove_dir_all(pids);
}

/// The owner's delegate (2026-11-20): a run wisp was stopped in the middle of picks up where it was. Here it waited on
/// your yes to its spec: after the restart the same question is there, and your answer carries it on.
#[tokio::test(flavor = "multi_thread")]
async fn a_factory_run_picks_up_where_it_was_after_a_restart() {
    let (dir, repo, serve, api) =
        with_factories("serve-factory-resume", &[], json!({ "short": { "stations": { "spec": { "approve": "human" }, "build": {} } } }))
            .await;
    let id = api
        .cmd("runtime.create", json!({ "prompt": "Fix it", "cwd": repo, "agent": "claude", "worktree": false, "factory": "short" }))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = until_run(&api, &id, "the spec waits", |v| waits_at(v, "spec")).await;
    serve.stop();
    let (serve, port) = common::restart(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let after = until_run(&api, &id, "still waiting after the restart", |v| v["state"] == "waiting" || v["state"] == "stopped").await;
    assert_eq!(after["state"], "waiting", "it picked up instead of ending: {after:#}");
    assert_eq!(after["waiting"]["askId"], before["waiting"]["askId"], "the same question");
    api.cmd("runtime.answer", json!({ "id": id, "text": "Approve", "askId": after["waiting"]["askId"] })).await;
    let run = until_run(&api, &id, "built and done", |v| matches!(v["state"].as_str(), Some("done" | "failed" | "stopped"))).await;
    assert_eq!((run["state"].as_str(), station(&run, "build")["state"].as_str()), (Some("done"), Some("done")), "{run:#}");
    drop(serve);
}
