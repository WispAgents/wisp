//! A delegate on another machine (F-mach-7, ADR-0059, v1 services/relay.js), end to end on two `wisp serve`: the
//! second added to the first as a `loopback` machine (as `serve_crew_machine.rs` does). The lead's work (its branch, an
//! uncommitted edit) is copied there into a folder under that wisp's own home, never a checkout of the user's; the
//! delegate (the fake codex) runs there and edits the copy; the lead reads its report, which names the machine, and
//! the copy's changes are in the lead's folder, uncommitted, with the lead's own uncommitted edit kept.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Api, start, start_with, temp};
use serde_json::{Value, json};

fn git(at: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(at).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

async fn until(what: &str, secs: u64, mut f: impl AsyncFnMut() -> Option<Value>) -> Value {
    let at = Instant::now();
    loop {
        if let Some(v) = f().await {
            return v;
        }
        assert!(at.elapsed() < Duration::from_secs(secs), "{what}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delegate_on_another_machine_runs_there_says_where_and_its_change_comes_back() {
    let (da, db) = (temp("dlg-machine-a"), temp("dlg-machine-b"));
    // the lead stays running while it delegates (its own fake run is slow)
    let (_sa, pa) = start_with(&da, &[("FAKE_DELAY", "60000")]);
    let (_sb, pb) = start(&db);
    let a = Api { http: reqwest::Client::new(), port: pa, n: 0.into() };
    let b = Api { http: reqwest::Client::new(), port: pb, n: 0.into() };
    let added = a.cmd("machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": pb, "name": "studio" })).await;
    let machine = added["id"].as_str().unwrap().to_owned();
    until("the peer comes online", 60, async || {
        let v = a.cmd("machines.get", json!({ "id": machine })).await;
        (v["state"] == "online").then_some(v)
    })
    .await;

    // the lead's project: a commit on `fix`, and an edit it hasn't committed
    let proj = da.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    git(&proj, &["init", "-q", "-b", "main"]);
    // the lead's checkout as the test writes it: no line-ending conversion (Windows' git defaults to autocrlf=true)
    git(&proj, &["config", "core.autocrlf", "false"]);
    std::fs::write(proj.join("math.js"), "export const add = (a, b) => a - b;\n").unwrap();
    std::fs::write(proj.join("README.md"), "app\n").unwrap();
    git(&proj, &["add", "."]);
    git(&proj, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    git(&proj, &["checkout", "-qb", "fix"]);
    std::fs::write(proj.join("README.md"), "app\nthe lead's own edit\n").unwrap();
    let head = git(&proj, &["rev-parse", "HEAD"]);

    let team = json!({ "wisp.runtime": { "subagents": [{ "name": "far", "agent": "codex", "machine": machine, "place": "studio" }] } });
    let lead = a.cmd("runtime.create", json!({ "prompt": "lead", "cwd": proj, "agent": "claude", "ext": team })).await;
    let lead_id = lead["id"].as_str().unwrap().to_owned();
    a.until(&lead_id, "the lead running", |t| t.len() == 1 && t[0]["status"] == "running").await;

    let prompt = format!("fix the add bug in {}", proj.join("math.js").display());
    let out = a.cmd("runtime.delegate", json!({ "id": lead_id, "tasks": [{ "subagent": "far", "prompt": prompt }] })).await;
    assert!(out[0]["error"].is_null(), "{out}");
    assert_eq!(out[0]["place"], "studio", "the delegation names the machine: {out}");
    let key = out[0]["id"].as_str().unwrap().to_owned();
    let (on, there_id) = key.split_once('|').unwrap_or_else(|| panic!("a delegate elsewhere is `<machine>|<id>`: {key}"));
    assert_eq!(on, machine);

    // it runs on the other wisp, in a copy under that wisp's own folder, with the lead's uncommitted edit in it
    let there = b.cmd("runtime.get", json!({ "id": there_id })).await;
    let copy = std::path::PathBuf::from(there["cwd"].as_str().unwrap_or_else(|| panic!("{there}")));
    assert!(copy.starts_with(db.join("home/plugins/wisp.machines/copies")), "{}", copy.display());
    assert_eq!(std::fs::read_to_string(copy.join("README.md")).unwrap(), "app\nthe lead's own edit\n");
    assert_eq!(git(&copy, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix");
    let asked = b.cmd("runtime.turns", json!({ "id": there_id })).await;
    let first = asked.as_array().cloned().or_else(|| asked["turns"].as_array().cloned()).unwrap_or_default();
    let said = first.first().map(|t| t["prompt"].as_str().unwrap_or_default().to_owned()).unwrap_or_default();
    assert!(said.starts_with("fix the add bug in math.js"), "the lead's path, as a path in the folder: {said}");
    assert!(nothing_runs_here(&a, there_id).await, "nothing of it runs here");

    // the lead reads its report: done, it names the machine, and its changes are in the lead's folder now
    let reports = until("the delegate's report", 90, async || {
        let r = a.cmd("runtime.results", json!({ "id": lead_id, "ids": [], "wait": 5 })).await;
        (r["reports"][0]["done"] == true).then_some(r)
    })
    .await;
    let text = reports["reports"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("It ran on studio"), "{reports}");
    assert!(text.contains("file(s) are in your folder"), "{reports}");
    assert_eq!(std::fs::read_to_string(proj.join("math.js")).unwrap(), "export const add = (a, b) => a + b;\n");
    assert_eq!(std::fs::read_to_string(proj.join("notes.txt")).unwrap(), "new file\n", "its new file came too");
    assert_eq!(std::fs::read_to_string(proj.join("README.md")).unwrap(), "app\nthe lead's own edit\n", "the lead's edit is kept");
    // brought as uncommitted changes: no commit made, no branch moved
    assert_eq!(git(&proj, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&proj, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix");

    // the lead's row and its turn's record say where
    let turns = a.turns(&lead_id).await;
    assert_eq!(turns[0]["ext"]["wisp.runtime"]["delegations"][0]["place"], "studio", "{turns:?}");
    // a second read brings nothing twice
    let again = a.cmd("runtime.results", json!({ "id": lead_id, "ids": [], "wait": 0 })).await;
    assert!(!again["reports"][0]["text"].as_str().unwrap_or_default().contains("in your folder"), "{again}");
}

async fn nothing_runs_here(a: &Api, id: &str) -> bool {
    a.try_cmd("runtime.get", json!({ "id": id })).await["ok"] == false
}

/// ADR-0060: deleting the lead deletes its delegate's task on the other wisp and the copy it worked in there (only
/// under that wisp's own folder); v1 deleted the task there after each job, ADR-0059 left both behind.
#[tokio::test(flavor = "multi_thread")]
async fn deleting_the_lead_deletes_its_delegates_copy_and_task_there() {
    let (da, db) = (temp("dlg-gone-a"), temp("dlg-gone-b"));
    let (_sa, pa) = start_with(&da, &[("FAKE_DELAY", "60000")]);
    let (_sb, pb) = start(&db);
    let a = Api { http: reqwest::Client::new(), port: pa, n: 0.into() };
    let b = Api { http: reqwest::Client::new(), port: pb, n: 0.into() };
    let added = a.cmd("machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": pb, "name": "studio" })).await;
    let machine = added["id"].as_str().unwrap().to_owned();
    until("the peer comes online", 60, async || {
        let v = a.cmd("machines.get", json!({ "id": machine })).await;
        (v["state"] == "online").then_some(v)
    })
    .await;
    let proj = da.join("proj");
    git(&proj, &["init", "-q", "-b", "main"]);
    // the lead's checkout as the test writes it: no line-ending conversion (Windows' git defaults to autocrlf=true)
    git(&proj, &["config", "core.autocrlf", "false"]);
    git(&proj, &["add", "."]);
    git(&proj, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    let team = json!({ "wisp.runtime": { "subagents": [{ "name": "far", "agent": "codex", "machine": machine, "place": "studio" }] } });
    let lead = a.cmd("runtime.create", json!({ "prompt": "lead", "cwd": proj, "agent": "claude", "ext": team })).await;
    let lead_id = lead["id"].as_str().unwrap().to_owned();
    a.until(&lead_id, "the lead running", |t| t.len() == 1 && t[0]["status"] == "running").await;
    let out = a.cmd("runtime.delegate", json!({ "id": lead_id, "tasks": [{ "subagent": "far", "prompt": "fix the add bug" }] })).await;
    let key = out[0]["id"].as_str().unwrap_or_else(|| panic!("{out}")).to_owned();
    let (_, there_id) = key.split_once('|').unwrap();
    let there = b.cmd("runtime.get", json!({ "id": there_id })).await;
    let copy = std::path::PathBuf::from(there["cwd"].as_str().unwrap());
    assert!(copy.is_dir() && copy.starts_with(db.join("home/plugins/wisp.machines/copies")), "{}", copy.display());

    a.cmd("runtime.delete", json!({ "id": lead_id })).await;
    until("the delegate's task there goes", 30, async || nothing_runs_here(&b, there_id).await.then_some(Value::Null)).await;
    until("its copy there goes", 30, async || (!copy.exists()).then_some(Value::Null)).await;
    assert!(copy.parent().is_some_and(|p| !p.exists()), "the copy's own folder too");
    assert!(proj.join("math.js").is_file(), "the lead's own folder stays");
    let row = a.cmd("machines.get", json!({ "id": machine })).await;
    assert!(row["uncarry"].is_null(), "nothing kept for the sweep: {row}");
}

/// The lead's turn ends while its delegate on the other machine still works (v1 `#awaiting` counts `child.relay`): it
/// stays in Working on it, wisp watches it there through the machine's link, and when it ends there the lead gets one
/// follow-up from wisp with its report (it ran on studio, its changes are in the lead's folder), without ever calling
/// `subagent_results`. (Before r251 the lead's end let it go: it landed at once and heard nothing.)
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_whose_delegate_elsewhere_outlasts_its_turn_hears_its_report_and_gets_its_changes() {
    let (da, db) = (temp("dlg-wait-a"), temp("dlg-wait-b"));
    // the lead's run lasts long enough to delegate; the delegate's there outlasts it
    let (_sa, pa) = start_with(&da, &[("FAKE_DELAY", "3000")]);
    let (_sb, pb) = start_with(&db, &[("FAKE_DELAY", "9000")]);
    let a = Api { http: reqwest::Client::new(), port: pa, n: 0.into() };
    let added = a.cmd("machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": pb, "name": "studio" })).await;
    let machine = added["id"].as_str().unwrap().to_owned();
    until("the peer comes online", 60, async || {
        let v = a.cmd("machines.get", json!({ "id": machine })).await;
        (v["state"] == "online").then_some(v)
    })
    .await;
    let proj = da.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    git(&proj, &["init", "-q", "-b", "main"]);
    git(&proj, &["config", "core.autocrlf", "false"]);
    // nothing for the lead's own fake run to fix here (it edits only `a - b`): what comes back is the delegate's alone
    std::fs::write(proj.join("math.js"), "export const add = (a, b) => a + b;\n").unwrap();
    git(&proj, &["add", "."]);
    git(&proj, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    let team = json!({ "wisp.runtime": { "subagents": [{ "name": "far", "agent": "codex", "machine": machine, "place": "studio" }] } });
    let lead = a.cmd("runtime.create", json!({ "prompt": "lead", "cwd": proj, "agent": "claude", "ext": team })).await;
    let lead_id = lead["id"].as_str().unwrap().to_owned();
    a.until(&lead_id, "the lead running", |t| t.len() == 1 && t[0]["status"] == "running").await;
    let out = a.cmd("runtime.delegate", json!({ "id": lead_id, "tasks": [{ "subagent": "far", "prompt": "fix the add bug" }] })).await;
    assert!(out[0]["error"].is_null(), "{out}");

    // the lead's own turn ends first: it waits on far, in Working
    a.until(&lead_id, "the lead's turn ended", |t| t.len() == 1 && t[0]["status"] != "running").await;
    let s = until("the lead waits on far", 30, async || {
        let s = a.cmd("runtime.get", json!({ "id": lead_id })).await;
        (s["ext"]["wisp.runtime"]["waitsOn"] == json!(["far"])).then_some(s)
    })
    .await;
    assert_eq!(s["state"], "running", "{s}");

    // far ends there: wisp sends the lead its report, with its changes in the lead's folder
    let turns = a.until(&lead_id, "the report-back turn", |t| t.len() == 2).await;
    assert_eq!(turns[1]["by"], "wisp", "{turns:?}");
    let p = turns[1]["prompt"].as_str().unwrap_or_default();
    assert!(p.starts_with("Subagents you delegated to earlier have finished."), "{p}");
    assert!(p.contains("### far (") && p.contains("— done"), "{p}");
    assert!(p.contains("It ran on studio") && p.contains("file(s) are in your folder"), "{p}");
    assert_eq!(std::fs::read_to_string(proj.join("notes.txt")).unwrap(), "new file\n", "the new file it made there is here");
    // brought once: a read after brings nothing again
    let again = a.cmd("runtime.results", json!({ "id": lead_id, "ids": [out[0]["id"].clone()], "wait": 0 })).await;
    assert!(!again["reports"][0]["text"].as_str().unwrap_or_default().contains("in your folder"), "{again}");
}
