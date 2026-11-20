//! Run with a formula everywhere, and everything referenced (owner-list row 61, ADR-0111), end to end on `wisp serve`
//! with the fake harness. One formula file declares an agent (its model from a variable), a project, a goal and a task
//! naming them by `${…}` and by name; nothing is applied, and:
//! - `wisp validate -f` evaluates its references as `.wisp/` does, and refuses one that names nothing, saying what there is;
//! - `wisp "…" --formula f.yml` (a plain launch) runs under it: its agent is the task's;
//! - `wisp run <task> --formula f.yml` runs the task it declares by its name (not as a prompt), its goal inlined and its
//!   project by name;
//! - `wisp run -f x.task.yml` with `formula: ./f.yml` resolves the task's names against it, and refuses an unknown one;
//! - `wisp mcp`'s `wisp_launch` takes `formula`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)] // one walk through every surface

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp, yaml_str};
use serde_json::{Value, json};

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

fn formula(proj: &str) -> String {
    let proj = yaml_str(proj);
    format!(
        "wisp: 1
variables:
  model: {{ type: string, default: opus }}
agents:
  critic: {{ harness: claude, model: \"${{var.model}}\", instructions: be strict }}
projects:
  app: {{ path: {proj} }}
goals:
  green: {{ text: the build is green, maxRounds: 2 }}
tasks:
  fix-build: {{ prompt: \"Fix the build in {{area}}\", agent: \"${{agent.critic}}\", goal: green, project: app, worktree: false }}
"
    )
}

/// The task's spec and its harness, as the peer made it.
async fn made(api: &Api, id: &str) -> Value {
    api.cmd("runtime.get", json!({ "id": id })).await
}

fn id_of(out: &str) -> String {
    out.split_whitespace()
        .skip_while(|w| *w != "attach")
        .nth(1)
        .map(str::to_owned)
        .or_else(|| serde_json::from_str::<Value>(out).ok().and_then(|v| v[0]["id"].as_str().map(str::to_owned)))
        .unwrap_or_else(|| panic!("no task id in {out}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_formula_runs_everywhere_and_every_name_in_it_is_referenced() {
    let dir = temp("serve-formula-refs");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let proj_s = proj.display().to_string();
    std::fs::write(dir.join("f.yml"), formula(&proj_s)).unwrap();

    // validate: references evaluated as a folder's are, so this one is clean
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "f.yml"]);
    assert_eq!(code, 0, "{out}{err}");
    // one that names nothing, by `${…}` and by name: refused, saying what there is
    std::fs::write(dir.join("bad.yml"), "wisp: 1\nagents:\n  critic: { harness: claude }\ncrew:\n  ada: { agent: \"${agent.nope}\" }\n")
        .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "bad.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("`agent.nope` isn't declared: agents has critic"), "{out}{err}");
    std::fs::write(dir.join("worse.yml"), "wisp: 1\nagents:\n  critic: { harness: claude }\ntasks:\n  t: { prompt: x, agent: nosuch }\n")
        .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "worse.yml"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(format!("{out}{err}").contains("tasks.t: no agent named nosuch: agents has critic (or "), "{out}{err}");

    // a setting's value and a `tools:` entry name things too
    std::fs::write(
        dir.join("names.yml"),
        "wisp: 1\nsettings:\n  channels.project: nosuch\ntools:\n  nobody: { deny: [wisp_ask] }\n  plan: { deny: [wisp_ask] }\n",
    )
    .unwrap();
    let (code, out, err) = wisp(&dir, port, &["validate", "-f", "names.yml"]);
    let said = format!("{out}{err}");
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("settings.channels.project: no project named nosuch: no projects are declared"), "{said}");
    assert!(said.contains("tools.nobody: no agent named nobody: no agents are declared (or "), "{said}");
    assert!(!said.contains("tools.plan"), "a mode names no agent: {said}");

    // wisp "…" --formula: a plain launch under it, its one agent the task's, its model from the variable
    let (code, out, err) = wisp(&dir, port, &["review the diff", "--formula", "f.yml", "--cwd", &proj_s, "--no-worktree", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let t = made(&api, &id_of(&out)).await;
    assert_eq!(t["ext"]["wisp.runtime"]["spec"]["agent"], "critic", "{t}");
    assert_eq!((t["agent"].as_str(), t["model"].as_str()), (Some("claude"), Some("opus")), "{t}");
    assert_eq!(t["ext"]["wisp.runtime"]["spec"]["prompt"], "review the diff", "the words are the prompt, not --formula f.yml: {t}");

    // --agent may name the formula's own agent
    let (code, out, err) =
        wisp(&dir, port, &["look again", "--formula", "f.yml", "--agent", "critic", "--cwd", &proj_s, "--no-worktree", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(made(&api, &id_of(&out)).await["ext"]["wisp.runtime"]["spec"]["agent"], "critic");

    // wisp run <task> --formula: the task it declares, by its name; its goal inlined, its project by name
    let (code, out, err) = wisp(&dir, port, &["run", "fix-build", "--formula", "f.yml", "area=the parser"]);
    assert_eq!(code, 0, "{out}{err}");
    let t = made(&api, &id_of(&out)).await;
    let spec = &t["ext"]["wisp.runtime"]["spec"];
    assert_eq!(spec["prompt"], "Fix the build in the parser", "{t}");
    assert_eq!(spec["goal"]["text"], "the build is green", "{t}");
    assert_eq!(t["cwd"], json!(proj_s), "project: app is its folder: {t}");
    // nothing applied
    assert_eq!(api.cmd("automations.tasks", json!({})).await, json!([]));
    let agents = api.cmd("resources.export", json!({ "kinds": ["agent"] })).await;
    assert!(!agents.to_string().contains("critic"), "{agents}");
    // review r194 #6: a plain launch is always a prompt, even one that starts with a declared task's name
    let (code, out, err) = wisp(&dir, port, &["fix-build the docs too", "--formula", "f.yml", "--cwd", &proj_s, "--no-worktree", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(made(&api, &id_of(&out)).await["ext"]["wisp.runtime"]["spec"]["prompt"], "fix-build the docs too");
    // and words past a declared task's params are refused, saying how to pass them
    let (code, out, err) = wisp(&dir, port, &["run", "fix-build", "--formula", "f.yml", "area=x", "and", "more"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("fix-build takes {area}: \"and more\" is left over"), "{out}{err}");
    // a declared task's param left open: said, nothing started
    let (code, out, err) = wisp(&dir, port, &["run", "fix-build", "--formula", "f.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("needs {area}"), "{out}{err}");

    // a task file that references the formula instead of copying it; its names resolved against it
    std::fs::write(dir.join("fix.task.yml"), "wisp: 1\ntask: { prompt: tidy the tests, agent: critic, goal: green }\nformula: ./f.yml\n")
        .unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "-f", "fix.task.yml"]);
    assert_eq!(code, 0, "{out}{err}");
    let t = made(&api, &id_of(&out)).await;
    assert_eq!(
        (t["agent"].as_str(), t["ext"]["wisp.runtime"]["spec"]["goal"]["text"].as_str()),
        (Some("claude"), Some("the build is green")),
        "{t}"
    );
    std::fs::write(dir.join("typo.task.yml"), "wisp: 1\ntask: { prompt: x, agent: critc }\nformula: [./f.yml]\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "-f", "typo.task.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains("typo.task.yml: no agent named critc: did you mean critic? agents has critic (or "),
        "{out}{err}"
    );
    std::fs::write(dir.join("loop.task.yml"), "wisp: 1\ntask: { prompt: x }\nformula: [./f.yml, f.yml]\n").unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "-f", "loop.task.yml"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(format!("{out}{err}").contains("f.yml is named twice"), "{out}{err}");

    // wisp mcp's wisp_launch takes `formula` (a file relative to its folder), and names its agent
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
        let launch =
            json!({ "prompt": "check it", "cwd": proj_s, "worktree": false, "formula": "../f.yml", "agent": "critic", "goal": "green" });
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
    let mid =
        said.as_str().unwrap().strip_prefix("launched ").and_then(|r| r.split(':').next()).unwrap_or_else(|| panic!("{said}")).to_owned();
    let t = made(&api, &mid).await;
    assert_eq!((t["ext"]["wisp.runtime"]["spec"]["agent"].as_str(), t["model"].as_str()), (Some("critic"), Some("opus")), "{t}");
    assert_eq!(t["ext"]["wisp.runtime"]["spec"]["goal"]["text"], "the build is green", "{t}");
    drop(serve);
}

/// Windows CI (3411fbc0): the formula's project path was written in a double-quoted scalar, where `C:\Users`'s `\U`
/// is an escape. Single-quoted, any path reads back as itself.
#[test]
fn a_windows_path_in_the_formula_reads_back_as_itself() {
    use wisp_resources_core::formula::{Format, documents};
    for p in [r"C:\Users\x\AppData\Local\Temp\wisp-serve-formula-refs-1\proj", r"C:\it's: here #1\proj", "/tmp/x/proj", r"\\srv\share\p"] {
        let doc = documents(&formula(p), Format::Yaml).unwrap_or_else(|e| panic!("{p}: {e}\n{}", formula(p)));
        assert_eq!(doc[0]["projects"]["app"]["path"], p);
    }
}

/// Audit G8: a formula with a field no kind has (`agents.critic.brief`) launched anyway: the agent was dropped, and the
/// task showed "✗ Not started · no harness critic". A task under it is refused before it launches, in validate's words,
/// from `wisp "…" --formula` and `wisp run <task> --formula`; nothing starts.
#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_formula_is_refused_before_a_task_launches_audit_g8() {
    let dir = temp("serve-formula-invalid");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::write(
        dir.join("team.yml"),
        "wisp: 1\nagents:\n  critic: { harness: claude, brief: be strict }\ntasks:\n  release: { prompt: ship it, agent: critic }\n",
    )
    .unwrap();
    for args in [&["fix it", "--formula", "team.yml", "--agent", "critic"][..], &["run", "release", "--formula", "team.yml"]] {
        let (code, out, err) = wisp(&dir, port, args);
        assert_ne!(code, 0, "{args:?}: {out}{err}");
        assert!(format!("{out}{err}").contains("agents.critic.brief: unknown field"), "{args:?}: {out}{err}");
    }
    let all = api.cmd("runtime.list", json!({})).await;
    assert!(all.as_array().is_none_or(Vec::is_empty), "nothing launched: {all}");
}

/// Audit G9: `wisp run release --formula team.yml` from a project ran in place in the user's checkout, where `wisp "…"
/// --formula` and a plain `wisp run fix-build` each got a worktree. It gets its own worktree too; a task that says
/// `worktree: false` still runs in place.
#[tokio::test(flavor = "multi_thread")]
async fn a_declared_task_under_a_formula_runs_in_its_own_worktree_audit_g9() {
    let dir = temp("serve-formula-worktree");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    for args in
        [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]]
    {
        assert!(Command::new("git").args(args).current_dir(&dir).status().unwrap().success());
    }
    std::fs::write(
        dir.join("team.yml"),
        "wisp: 1\nagents:\n  critic: { harness: claude }\ntasks:\n  release: { prompt: ship it, agent: critic }\n  here: { prompt: tidy, agent: critic, worktree: false }\n",
    )
    .unwrap();
    let real = wisp_proc::canonical_blocking(&dir).unwrap();
    for (task, own) in [("release", true), ("here", false)] {
        let (code, out, err) = wisp(&dir, port, &["run", task, "--formula", "team.yml"]);
        assert_eq!(code, 0, "{out}{err}");
        let id = out.split_whitespace().skip_while(|w| *w != "attach").nth(1).unwrap_or_default().to_owned();
        let t = made(&api, &id).await;
        let cwd = wisp_proc::canonical_blocking(std::path::Path::new(t["cwd"].as_str().unwrap_or_default())).unwrap_or_default();
        assert_eq!(cwd != real, own, "{task}: {cwd:?} (the checkout is {real:?}): {t}");
    }
}

/// Audit I4: New task with a task file picked (`agent: critic`; critic is codex, gpt-5, high in its formula) sent the
/// bar's `"model":"opus"` beside the file's agent, and codex ran `-m opus`. New task now sends what the file says
/// (packages/shell `barArgs`, `a_picked_task_files_agent_runs_its_own_model_never_the_bars_audit_I4`): this is that body,
/// as `resources.overlay` reads the file, and the harness runs the file's model and thinking.
#[tokio::test(flavor = "multi_thread")]
async fn a_picked_task_file_runs_its_agents_model_not_the_bars_audit_i4() {
    let dir = temp("formula-pick-model");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let proj = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    let (serve, port) = common::start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::write(proj.join("team.yml"), "wisp: 1\nagents:\n  critic: { harness: codex, model: gpt-5, effort: high }\n").unwrap();
    let text = "wisp: 1\ntask: { prompt: check it, agent: critic }\nformula: ./team.yml\n";
    let out = api.cmd("resources.overlay", json!({ "text": text, "name": "a.task.yml", "cwd": proj })).await;
    assert!(out["problems"].as_array().is_none_or(Vec::is_empty), "{out}");
    // New task's body: the bar's agent (claude), none of its model, then the file's own args over it
    let mut body = json!({ "prompt": "check it", "cwd": proj, "worktree": false, "agent": "claude", "formula": out["overlay"] });
    if let (Some(b), Some(task)) = (body.as_object_mut(), out["task"].as_object()) {
        for k in ["agent", "model", "effort"] {
            if let Some(v) = task.get(k) {
                b.insert(k.into(), v.clone());
            }
        }
    }
    assert!(body.get("model").is_none(), "{body}");
    let made = api.cmd("runtime.create", body).await;
    let id = made["id"].as_str().unwrap_or_else(|| panic!("{made}")).to_owned();
    let runs = common::until_records(&rec.join("codex.jsonl"), "codex's run", |r| !r.is_empty()).await;
    let argv = common::argv(&runs[0]).join(" ");
    assert!(argv.contains("-m gpt-5") && argv.contains("model_reasoning_effort=\"high\""), "{argv}");
    assert!(!argv.contains("opus"), "{argv}");
    assert_eq!(made_task(&api, &id).await["model"], "gpt-5");
    drop(serve);
}

async fn made_task(api: &Api, id: &str) -> Value {
    api.cmd("runtime.get", json!({ "id": id })).await
}

/// Audit I8 and I11: `wisp run <task> --formula F -d` was refused ("release takes no values: \"-d\" is left over"), and
/// `--wait` returned at `review` before the task's goal was judged. `-d` works as on every run form, and `--wait` returns
/// only once the task asks, is done, or its goal is judged.
#[tokio::test(flavor = "multi_thread")]
async fn run_task_under_a_formula_takes_detach_and_waits_through_judging_audit_i8_i11() {
    let dir = temp("formula-run-flags");
    let proj = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::write(dir.join("f.yml"), formula(&proj.display().to_string())).unwrap();
    let (code, out, err) = wisp(&dir, port, &["run", "fix-build", "area=api", "--formula", "f.yml", "-d"]);
    assert_eq!(code, 0, "-d: {out}{err}");
    assert!(!format!("{out}{err}").contains("left over"), "{out}{err}");
    let (code, out, err) = wisp(&dir, port, &["run", "fix-build", "area=ui", "--formula", "f.yml", "--wait"]);
    assert_eq!(code, 0, "--wait: {out}{err}");
    let list = api.cmd("runtime.list", json!({})).await;
    let id = list.as_array().unwrap().iter().find(|t| t["title"] == "fix-build" && t["id"].as_str().is_some_and(|i| out.contains(i)));
    let id = id.and_then(|t| t["id"].as_str()).unwrap_or_else(|| panic!("{out}\n{list}")).to_owned();
    let t = made(&api, &id).await;
    let goal = t["ext"]["wisp.goal"]["state"].as_str().unwrap_or_default().to_owned();
    let state = t["state"].as_str().unwrap_or_default();
    assert!(
        matches!(state, "waiting" | "done") || matches!(goal.as_str(), "met" | "paused" | "stalled"),
        "--wait returned at {state}, goal {goal}: {t}"
    );
    drop(serve);
}
