//! Sandboxes end to end (ADR-0047), on a local grid (`wisp-grid dev`: each box a wisp process) and this wisp signed
//! in to it: `/sandbox` is refused until Claude is signed in for sandboxes; then the project (its commits and its
//! uncommitted change) goes into a sandbox of its own and the task runs there, reachable through this wisp
//! (`/api/peer/<machine>`); the sandbox is the task's, never a place in Machines; deleting the task on the sandbox
//! takes the sandbox with it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use base64::Engine as _;
use common::{Api, start_with, temp};
use serde_json::{Value, json};

fn git(at: &Path, args: &[&str]) {
    let o = Command::new("git").current_dir(at).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

async fn approve(port: u16, url: &str, handle: &str, code: &str) {
    let b = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve("wisp.localhost", SocketAddr::from(([127, 0, 0, 1], port)))
        .build()
        .unwrap();
    let r = b.get(format!("{url}/auth/dev?handle={handle}")).send().await.unwrap();
    let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_owned();
    let page = b.get(format!("{url}/activate?code={code}")).header("cookie", &cookie).send().await.unwrap().text().await.unwrap();
    let csrf = page.split("name=\"csrf\" value=\"").nth(1).unwrap().split('"').next().unwrap().to_owned();
    let r = b
        .post(format!("{url}/activate"))
        .header("cookie", &cookie)
        .json(&json!({ "csrf": csrf, "code": code, "answer": "allow" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

async fn until(what: &str, secs: u64, mut f: impl AsyncFnMut() -> Option<Value>) -> Value {
    let at = Instant::now();
    loop {
        if let Some(v) = f().await {
            return v;
        }
        assert!(at.elapsed() < Duration::from_secs(secs), "{what}");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// This wisp (it puts the fake harnesses in dir/bin), then a local grid whose boxes use them too, and this wisp signed
/// in to it. The last item stops the grid (it holds the grid until then).
async fn signed_in(dir: &Path) -> (common::Serve, Api, u16, Box<dyn FnOnce() + Send>) {
    let (serve, port) = start_with(dir, &[("WISP_SANDBOX_TICK_MS", "500")]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let data = dir.join("grid");
    std::fs::create_dir_all(data.join("secrets")).unwrap();
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    std::fs::write(data.join("secrets/master.json"), json!({ "current": "k1", "keys": { "k1": key } }).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(data.join("secrets/master.json"), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let bin = dir.join("bin");
    let env = vec![
        ("WISP_WEB_DIR".into(), dir.join("no-web").display().to_string()),
        ("WISP_CLAUDE_BIN".into(), bin.join("claude").display().to_string()),
    ];
    let (g, gport, stop) = wisp_grid::edge::dev(&data, 0, PathBuf::from(env!("CARGO_BIN_EXE_wisp")), env, 600_000).await.unwrap();
    let url = format!("http://wisp.localhost:{gport}");
    let f = api.cmd("grid-client.signin", json!({ "grid": url })).await;
    approve(gport, &url, "ada", f["userCode"].as_str().unwrap()).await;
    until("signed in", 30, async || {
        let s = api.cmd("grid-client.signinStatus", json!({ "id": f["id"] })).await;
        (s["state"] != "waiting").then_some(s)
    })
    .await;
    (
        serve,
        api,
        port,
        Box::new(move || {
            stop.cancel();
            drop(g);
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sandbox_task_runs_in_a_box_of_its_own_with_the_project_and_goes_with_the_task() {
    let dir = temp("serve-sandbox");
    // the project: a commit on `fix`, and a change not committed
    let proj = dir.join("proj");
    git(&proj, &["init", "-q", "-b", "main"]);
    git(&proj, &["add", "."]);
    git(&proj, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    git(&proj, &["checkout", "-qb", "fix"]);
    std::fs::write(proj.join("math.js"), "export const add = (a, b) => a + b; // not committed\n").unwrap();
    let (serve, api, port, stop) = signed_in(&dir).await;
    let info = api.cmd("sandbox.info", json!({})).await;
    assert_eq!(info["enabled"], true, "{info}");
    // not signed in to Claude for sandboxes: refused up front, nothing made
    let launch = json!({ "prompt": "fix the add bug", "cwd": proj, "agent": "claude" });
    let no = api.try_cmd("sandbox.launch", launch.clone()).await;
    assert!(no.to_string().contains("Sign in to Claude for sandboxes"), "{no}");
    assert_eq!(api.cmd("grid-client.sandboxes", json!({})).await, json!([]));
    // signed in (the token `claude setup-token` gives, sealed on the grid): it goes
    api.cmd("grid-client.sandboxLogin", json!({ "agent": "claude", "token": "sk-ant-oat01-test-token-for-every-sandbox-000" })).await;
    let made = api.cmd("sandbox.launch", launch).await;
    let (machine, peer, task) = (
        made["machine"].as_str().unwrap().to_owned(),
        made["peer"].as_str().unwrap().to_owned(),
        made["task"].as_str().unwrap().to_owned(),
    );
    // the task there, through this wisp's proxy (as the app reaches it): it ran in ~/code/proj, on `fix`, with the change
    let via = |cmd: &str, args: Value| {
        let (http, port, machine, cmd) = (api.http.clone(), port, machine.clone(), cmd.to_owned());
        async move {
            let r = http
                .post(format!("http://127.0.0.1:{port}/api/peer/{machine}/api/cmd"))
                .header("content-type", "application/json")
                .header("idempotency-key", format!("t-{}", rand_id()))
                .body(json!({ "cmd": cmd, "args": args }).to_string())
                .send()
                .await
                .unwrap();
            serde_json::from_slice::<Value>(&r.bytes().await.unwrap()).unwrap()
        }
    };
    let s = until("the sandbox's task ends", 60, async || {
        let s = via("runtime.get", json!({ "id": task })).await;
        (s["result"]["state"] != "running").then_some(s)
    })
    .await;
    let cwd = s["result"]["cwd"].as_str().unwrap_or_default().to_owned();
    assert!(cwd.ends_with("/code/proj"), "{s}");
    let patch = via("sandbox.patch", json!({ "cwd": cwd })).await;
    assert_eq!(patch["ok"], true, "the project came in through a seed: {patch}");
    // the sandbox is the task's: listed with its owner, never a machine you pick
    let info = api.cmd("sandbox.info", json!({})).await;
    assert_eq!(info["sandboxes"][0]["owner"], json!({ "task": format!("{peer}:{task}") }), "{info}");
    let machines = api.cmd("machines.list", json!({})).await;
    assert!(machines.as_array().unwrap().iter().any(|m| m["id"] == machine.as_str() && m["sandbox"].is_string()));
    assert_eq!(peer, machine, "the app reaches it by the machine's own id");
    // the task deleted on the sandbox: the sandbox goes with it
    via("runtime.delete", json!({ "id": task })).await;
    until("the sandbox goes with its task", 30, async || {
        let left = api.cmd("grid-client.sandboxes", json!({})).await;
        (left == json!([])).then_some(left)
    })
    .await;
    stop();
    drop(serve);
}

fn rand_id() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())
}

/// Bytes that don't compress (a bundle of them is as big as they are).
fn noise(n: usize, mut x: u64) -> Vec<u8> {
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x.to_le_bytes()[0]
        })
        .collect()
}

/// The project used to travel inside one sync message (`machines.route`), whose body the sandbox's wisp caps at 2 MB:
/// a repo whose bundle is over that never reached its sandbox. It goes as raw bytes now (`machines.send`, then
/// `sandbox.seed` names the upload), so a 3 MB history and an uncommitted 1 MB file arrive whole.
#[tokio::test(flavor = "multi_thread")]
async fn a_project_bigger_than_a_sync_message_reaches_its_sandbox() {
    let dir = temp("serve-sandbox-big");
    let proj = dir.join("proj");
    git(&proj, &["init", "-q", "-b", "main"]);
    let big = noise(3 << 20, 0x9e37_79b9_7f4a_7c15);
    std::fs::write(proj.join("assets.bin"), &big).unwrap();
    git(&proj, &["add", "."]);
    git(&proj, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "big"]);
    let loose = noise(1 << 20, 0x2545_f491_4f6c_dd1d);
    std::fs::write(proj.join("new.bin"), &loose).unwrap();
    let (serve, api, _port, stop) = signed_in(&dir).await;
    api.cmd("grid-client.sandboxLogin", json!({ "agent": "claude", "token": "sk-ant-oat01-test-token-for-every-sandbox-000" })).await;
    let made = api.try_cmd("sandbox.launch", json!({ "prompt": "look", "cwd": proj, "agent": "claude" })).await;
    assert_eq!(made["ok"], true, "a 3 MB project reaches its sandbox: {made}");
    // the box is a process here: its folder is on this disk
    let found = find(&dir.join("grid"), "assets.bin").unwrap_or_else(|| panic!("the project isn't in the sandbox"));
    assert_eq!(std::fs::read(&found).unwrap(), big, "its history came whole");
    assert_eq!(std::fs::read(found.with_file_name("new.bin")).unwrap(), loose, "and its new file, not committed");
    stop();
    drop(serve);
}

/// The first `<name>` under `root` in a `code/` folder (a sandbox's `~/code/<project>`).
fn find(root: &Path, name: &str) -> Option<PathBuf> {
    let mut todo = vec![root.to_owned()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with(".git") {
                todo.push(p);
            } else if p.file_name().is_some_and(|n| n == name) && p.components().any(|c| c.as_os_str() == "code") {
                return Some(p);
            }
        }
    }
    None
}
