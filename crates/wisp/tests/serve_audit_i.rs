//! Audit I (orchestration, 2026-11-20, docs/qa/audit-I-orchestration.md), end to end on a real `wisp serve` with the
//! fake agent: the CLI findings.
//!   - I2: a misspelled verb after a command word (`wisp crew pasue Zed`) launched a Full-auto task with those words.
//!   - I9: `--effort banana`, `--model plan` and `--agent auto` (Auto off) launched anyway.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env_remove("WISP_TASK")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

async fn tasks(api: &Api) -> usize {
    api.cmd("runtime.list", json!({})).await.as_array().map_or(0, Vec::len)
}

async fn mcp(http: &reqwest::Client, port: u16, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let r = http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    let text = r.text().await.unwrap();
    let json = text.lines().find_map(|l| l.strip_prefix("data:")).map_or(text.as_str(), str::trim).to_owned();
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_misspelled_verb_after_a_command_word_launches_nothing_audit_i2() {
    let dir = temp("serve-auditI-verb");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let before = tasks(&api).await;
    for (args, close) in [
        (&["crew", "pasue", "Zed"][..], "pause"),
        (&["crew", "paus", "Zed"], "pause"),
        (&["trigger", "of", "tick"], "off"),
        (&["goal", "sett", "abc", "never"], "set"),
        (&["later", "cancle", "x"], "cancel"),
        (&["factory", "stp", "x"], "stop"),
        (&["held", "x", "strat"], "start"),
    ] {
        let (code, out, err) = wisp(&dir, port, args);
        assert!(code == 1 || code == 2, "{args:?} exit {code}: {out}{err}");
        assert!(err.contains(&format!("did you mean {close}?")), "{args:?}: {err}");
        assert!(err.contains(&format!("To start a task with these words: wisp -- \"{}\"", args.join(" "))), "{args:?}: {err}");
        assert!(!out.contains("launched") && !err.contains("launched"), "{args:?}: {out}{err}");
    }
    assert_eq!(tasks(&api).await, before, "nothing launched");
    // quoted as one, or after `--`, the words are the task
    let (code, out, err) = wisp(&dir, port, &["crew pasue Zed", "-d"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["-d", "--", "trigger", "of", "tick"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(tasks(&api).await, before + 2);
    // the owner's refinement: after a word a prompt may start with, far words are a prompt, unquoted
    for args in [&["-d", "review", "the", "auth", "module"][..], &["-d", "plugin", "for", "prettier"]] {
        let (code, out, err) = wisp(&dir, port, args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    assert_eq!(tasks(&api).await, before + 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_effort_a_mode_as_a_model_and_auto_while_off_launch_nothing_audit_i9() {
    let dir = temp("serve-auditI-flags");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let before = tasks(&api).await;
    for (args, said) in [
        (&["I effort", "--effort", "banana"][..], "--effort takes minimal, low, medium, high, xhigh or max, not banana"),
        (&["I model plan", "--model", "plan"], "plan is a mode, not a model: --mode plan"),
        (&["I model high", "--model", "high"], "high is a thinking level, not a model: --effort high"),
        (&["I agent auto", "--agent", "auto"], "Auto is off: name a harness with --agent"),
    ] {
        let (code, out, err) = wisp(&dir, port, args);
        assert!(code == 1 || code == 2, "{args:?} exit {code}: {out}{err}");
        assert!(err.contains(said) && err.contains("Nothing was launched"), "{args:?}: {err}");
    }
    assert_eq!(tasks(&api).await, before, "nothing launched");
    // a real level and a real model still launch
    let (code, out, err) = wisp(&dir, port, &["I ok", "--effort", "high", "--model", "opus", "-d"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// Audit I7: `wisp crew create … --on task.done` kept `"on":[]` (only v1's `done` was read) and `crew update --on` said
/// "on changed" for no change. `--on` takes v1's kinds and the triggers' names, refuses a word that wakes nothing, and
/// `crew update` says only what changed.
#[tokio::test(flavor = "multi_thread")]
async fn crew_on_sets_wakes_and_update_says_only_what_changed_audit_i7() {
    let dir = temp("serve-auditI-crew-on");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (code, out, err) = wisp(&dir, port, &["crew", "create", "--name", "Bob", "--role", "QA", "--agent", "claude", "--on", "task.done"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Bob joined the crew"), "a sentence, not a field dump: {out}");
    let bob = api.cmd("crew.get", json!({ "id": "bob" })).await;
    let wakes: Vec<&str> = bob["wakes"].as_array().unwrap().iter().filter_map(|w| w["on"].as_str()).collect();
    assert_eq!(wakes, ["task.done"], "{bob}");
    // the same again: nothing changed, said so
    let (code, out, err) = wisp(&dir, port, &["crew", "update", "bob", "--on", "task.done"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("nothing changed") && !out.contains("on changed"), "{out}");
    // v1's word for it, and a word that wakes nothing
    let (code, out, err) = wisp(&dir, port, &["crew", "update", "bob", "--on", "ci"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("on changed"), "{out}");
    let (code, out, err) = wisp(&dir, port, &["crew", "update", "bob", "--on", "nosuch"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("nosuch wakes nothing") && err.contains("task.done"), "{err}");
}

/// Audit I10: after New task's In place pick (`worktrees.worktree` off) a user's `wisp_launch` over MCP ran in the user's
/// checkout while `wisp "…"` made a worktree. The pick is New task's own: both make a worktree for a project's task.
#[tokio::test(flavor = "multi_thread")]
async fn the_cli_and_mcp_agree_on_where_a_task_runs_audit_i10() {
    let dir = temp("serve-auditI-where");
    let proj = dir.join("proj");
    for args in
        [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]]
    {
        assert!(Command::new("git").args(args).current_dir(&proj).status().unwrap().success());
    }
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (code, out, err) = wisp(&dir, port, &["add", "."]);
    assert_eq!(code, 0, "{out}{err}");
    // New task's In place, as the app sends it
    api.cmd("settings.set", json!({ "key": "worktrees.worktree", "value": "off" })).await;
    let real = wisp_proc::canonical_blocking(&proj).unwrap().display().to_string();
    let r = mcp(
        &api.http,
        port,
        "tools/call",
        json!({ "name": "wisp_launch", "arguments": { "prompt": "I where mcp", "cwd": real, "agent": "claude" } }),
    )
    .await;
    assert!(r["result"]["isError"] != true, "{r}");
    let (code, out, err) = wisp(&dir, port, &["I where cli", "-d"]);
    assert_eq!(code, 0, "{out}{err}");
    let list = api.cmd("runtime.list", json!({})).await;
    for title in ["I where mcp", "I where cli"] {
        let t = list
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["title"].as_str().is_some_and(|x| x.starts_with(title)))
            .unwrap_or_else(|| panic!("{title}: {list}"));
        let got = api.cmd("runtime.get", json!({ "id": t["id"] })).await;
        let cwd = got["cwd"].as_str().unwrap_or_default();
        assert!(cwd != real && got["ext"]["wisp.worktrees"]["root"].is_string(), "{title} ran in place: {cwd}");
    }
}

/// Audit I (D6, partly fixed): `automations_task_remove/run/set`, `channels_extra` and `channels_update` took only
/// `{args}` (`channels_update {"bogus":1}` → "wisp: the  channel isn't on"). Every listed tool says its fields: none is
/// `{args}` alone, and a field it doesn't take is refused.
#[tokio::test(flavor = "multi_thread")]
async fn no_listed_mcp_tool_takes_only_args_audit_i_d6() {
    let dir = temp("serve-auditI-d6");
    let (_serve, port) = start(&dir);
    let http = reqwest::Client::new();
    let list = mcp(&http, port, "tools/list", json!({})).await;
    let tools = list["result"]["tools"].as_array().cloned().unwrap_or_default();
    assert!(tools.len() > 100, "{list}");
    let bare: Vec<String> = tools
        .iter()
        .filter(|t| {
            let props = t["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
            let keys: Vec<&String> = props.keys().filter(|k| *k != "peer").collect();
            keys == ["args"]
        })
        .filter_map(|t| t["name"].as_str().map(str::to_owned))
        .collect();
    assert!(bare.is_empty(), "these take only {{args}}: {bare:?}");
    for (name, fields) in [
        ("automations_task_set", &["name", "prompt", "schedule"][..]),
        ("automations_task_remove", &["name"]),
        ("automations_task_run", &["name", "params", "words"]),
        ("channels_extra", &["channel", "what"]),
        ("channels_update", &["channel", "sender", "enabled"]),
        ("workflows_set", &["name", "tasks", "concurrency"]),
        ("workflows_remove", &["name"]),
        ("workflows_status", &["id"]),
        ("workflows_stop", &["id"]),
    ] {
        let t = tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("no {name}"));
        let props = t["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
        for f in fields {
            assert!(props.contains_key(*f), "{name} takes {f}: {t}");
        }
    }
    let r = mcp(&http, port, "tools/call", json!({ "name": "channels_update", "arguments": { "bogus": 1 } })).await;
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(r.to_string().contains("bogus"), "names the field it doesn't take: {r}");
}

/// What a help page may not say: notes for wisp's developers (a crate path, a plan section, an audit, a date it was
/// added, who decided), each with the words that gave it away. Pure, on the page's text.
fn developer_notes(page: &str) -> Vec<&'static str> {
    const NOTES: [&str; 14] = [
        "plan §",
        "ADR-0",
        "Additive (",
        "owner's delegate",
        "owner’s delegate",
        "::",
        "`wisp_",
        "capability set",
        "Set by the caller",
        "review r1",
        "QA round",
        "(v1 ",
        "HarnessInfo",
        "ext[\"",
    ];
    let mut found: Vec<&'static str> = NOTES.iter().copied().filter(|n| page.contains(n)).collect();
    // an audit cited ("audit A orch #6", "(audit G4: …)"); a long thread's compaction audit is a feature's name
    let cites =
        page.match_indices("audit ").any(|(i, _)| page[i + 6..].starts_with(|c: char| c.is_ascii_uppercase()) || page[..i].ends_with('('));
    if cites {
        found.push("audit <letter>");
    }
    found
}

/// A page whose description opens with the command's wire name (`` `goal.set`: … ``) instead of words.
fn opens_with_wire_name(doc: &str) -> bool {
    let d = doc.trim_start();
    d.starts_with('`') && d[1..].split('`').next().is_some_and(|w| w.contains('.') && !w.contains(' '))
}

/// Audit I, E11 (partly fixed): a sweep of every `--help` page found developer notes (`machines route`'s "A v2 command
/// … (plan §7.2.1)", `runtime setup ask`'s "`wisp_runtime_core::setup::WRITES`", `crew assign --from`'s "Set by the
/// caller", `runtime exec`'s "no agent capability set names it") and 75 pages opening with the wire name (`` `goal.set`:
/// … ``). No page has either. H9 and I13: the fields audit H and I named have words.
#[tokio::test(flavor = "multi_thread")]
async fn no_help_page_carries_developer_notes_or_opens_with_its_wire_name_audit_i_e11() {
    let dir = temp("serve-auditI-help");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let list: Vec<wisp_kernel::CommandInfo> = serde_json::from_value(api.cmd("cli.catalogue", json!({})).await).unwrap();
    let cat = wisp_cli_core::verbs::Catalogue::new(list);
    let mut bad: Vec<String> = Vec::new();
    let mut pages = 0;
    for (_, verbs) in cat.plugins() {
        for v in verbs {
            pages += 1;
            let page = wisp_cli_core::verbs::help_verb(v);
            let notes = developer_notes(&page);
            if !notes.is_empty() {
                let lines: Vec<&str> = page.lines().filter(|l| notes.iter().any(|n| l.contains(n))).collect();
                bad.push(format!("{}: {notes:?} in {lines:?}", v.path()));
            }
            if opens_with_wire_name(&v.spec.doc) {
                bad.push(format!("{}: opens with its wire name: {}", v.path(), v.spec.doc.chars().take(60).collect::<String>()));
            }
        }
    }
    assert!(pages > 300, "{pages} pages");
    assert!(bad.is_empty(), "{} of {pages} pages:\n{}", bad.len(), bad.join("\n"));
    // H9 and I13: each field named has words
    for (path, fields) in [
        ("crew assign", &["id", "prompt", "attachments", "kit", "confirm-limit"][..]),
        ("later plan", &["kind", "at"]),
        (
            "crew create",
            &[
                "checkin", "name", "role", "avatar", "color", "brief", "cwd", "machine", "sandbox", "agent", "model", "effort", "mode",
                "parallel", "paused", "notebook",
            ],
        ),
        ("runtime fork", &["id", "title"]),
    ] {
        let words: Vec<String> = path.split(' ').map(str::to_owned).collect();
        let (v, _) = cat.resolve(&words).unwrap_or_else(|| panic!("no {path}"));
        for f in fields {
            let field = v.spec.fields.iter().find(|x| x.flag() == *f).unwrap_or_else(|| panic!("{path} has no --{f}"));
            assert!(!field.doc.trim().is_empty(), "{path} --{f} has no words");
        }
        if path == "later plan" {
            // a word and a time, not JSON; the example has no JSON body
            let page = wisp_cli_core::verbs::help_verb(v);
            assert!(!page.contains("--kind JSON") && !page.contains("--at JSON") && page.contains("--kind task|reminder"), "{page}");
            assert!(page.contains("--prompt") && !page.contains("--body '{"), "{page}");
        }
    }
}

/// Audit H9: `wisp later plan` took the task only as a JSON body and the time as `--at JSON`. `--prompt` says it, `--in`
/// and `--at` take words, and the plan is made.
#[tokio::test(flavor = "multi_thread")]
async fn later_plan_takes_a_prompt_and_a_time_in_words_audit_h9() {
    let dir = temp("serve-auditI-later");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (code, out, err) = wisp(&dir, port, &["later", "plan", "--kind", "task", "--prompt", "H9 fix the flaky test", "--in", "30"]);
    assert_eq!(code, 0, "{out}{err}");
    let plans = api.cmd("later.plans", json!({})).await;
    assert!(plans.to_string().contains("H9 fix the flaky test"), "{plans}");
    let (code, out, err) = wisp(&dir, port, &["later", "plan", "--kind", "task", "--in", "30"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("--prompt"), "{err}");
}

/// Audit I15: `wisp trigger rm crew.bob.1` said "comes from wisp.crew: switch it there" (not how); `trigger test` on an
/// off trigger answered its next run as if on; scheduled runs were titled with the trigger's name, not what they do.
#[tokio::test(flavor = "multi_thread")]
async fn triggers_say_how_to_switch_a_crew_wake_that_they_are_off_and_runs_are_titled_by_prompt_audit_i15() {
    let dir = temp("serve-auditI-trig");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let r = api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [
                { "kind": "project", "name": "proj", "spec": { "path": proj } },
                { "kind": "trigger", "name": "tick", "spec": { "on": "schedule", "cron": "0 3 * * *", "prompt": "I15 tidy the logs", "project": "proj" } }
            ] }),
        )
        .await;
    assert_eq!(r["ok"], true, "{r}");
    let (code, out, err) = wisp(&dir, port, &["crew", "create", "--name", "Bob", "--agent", "claude", "--on", "task.done"]);
    assert_eq!(code, 0, "{out}{err}");
    for verb in ["rm", "off"] {
        let (code, out, err) = wisp(&dir, port, &["trigger", verb, "crew.bob.1"]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(err.contains("wisp crew update bob --wakes") && err.contains("wisp crew pause bob"), "{verb}: {err}");
    }
    let (code, out, err) = wisp(&dir, port, &["trigger", "off", "tick"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["trigger", "test", "tick"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("it's off") && out.contains("wisp trigger on tick"), "{out}");
    // a run it starts is titled by its prompt
    let (code, out, err) = wisp(&dir, port, &["trigger", "on", "tick"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["trigger", "fire", "tick"]);
    assert_eq!(code, 0, "{out}{err}");
    let at = std::time::Instant::now();
    loop {
        let list = api.cmd("runtime.list", json!({})).await;
        if let Some(t) = list.as_array().unwrap().iter().find(|t| t["title"].as_str().is_some_and(|x| x.contains("I15 tidy"))) {
            assert_ne!(t["title"], "tick", "{t}");
            break;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "no run titled by its prompt: {list}");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

/// ADR-0115: the question a guard change asks is answered with `wisp reply <task> Allow|Deny` (the proof run found the
/// CLI sent a follow-up there, which such a question refuses), and the harness's call reads the answer.
#[tokio::test(flavor = "multi_thread")]
async fn wisp_reply_answers_the_question_a_guard_change_asks_adr_0115() {
    let dir = temp("serve-auditI-guard-reply");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let http = api.http.clone();
    let call = tokio::spawn(async move {
        mcp(&http, port, "tools/call", json!({ "name": "settings_set", "arguments": { "key": "worktrees.projectAsk", "value": "off" } }))
            .await
    });
    let at = std::time::Instant::now();
    let id = loop {
        let list = api.cmd("runtime.list", json!({})).await;
        if let Some(t) = list
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["title"].as_str().is_some_and(|x| x.starts_with("Your harness wants to")) && t["state"] == "waiting")
        {
            break t["id"].as_str().unwrap().to_owned();
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "no task asks: {list}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let (code, out, err) = wisp(&dir, port, &["reply", &id, "Deny"]);
    assert_eq!(code, 0, "{out}{err}");
    let r = call.await.unwrap();
    assert!(r.to_string().contains("said no"), "{r}");
    assert_eq!(api.cmd("settings.get", json!({ "key": "worktrees.projectAsk" })).await, json!("on"));
}

/// `wisp reply <task> Deny --wait` answered the guard's question and exited 0 at once, never waiting or reading
/// --timeout (audit cli-r6): it now waits on the task as a follow-up's --wait does and exits as `wisp wait` would.
#[tokio::test(flavor = "multi_thread")]
async fn wisp_reply_wait_on_a_guard_question_waits_like_wisp_wait() {
    let dir = temp("serve-auditI-guard-reply-wait");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let http = api.http.clone();
    let call = tokio::spawn(async move {
        mcp(&http, port, "tools/call", json!({ "name": "settings_set", "arguments": { "key": "worktrees.projectAsk", "value": "off" } }))
            .await
    });
    let at = std::time::Instant::now();
    let id = loop {
        let list = api.cmd("runtime.list", json!({})).await;
        if let Some(t) = list
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["title"].as_str().is_some_and(|x| x.starts_with("Your harness wants to")) && t["state"] == "waiting")
        {
            break t["id"].as_str().unwrap().to_owned();
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "no task asks: {list}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let (code, out, err) = wisp(&dir, port, &["--json", "reply", &id, "Deny", "--wait", "--timeout", "10s"]);
    assert!(!out.contains("\"said\""), "--wait prints the task it waited on, not the answer's ack: {out}");
    assert!(out.contains(&id), "the waited task is shown: {out}");
    assert_ne!(code, 2, "{out}{err}");
    let r = call.await.unwrap();
    assert!(r.to_string().contains("said no"), "{r}");
}
