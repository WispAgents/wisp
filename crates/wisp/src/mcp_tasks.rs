//! `wisp mcp`'s task tools (setup audit F28): what v1's stdio server gave editors and other agents
//! (server/mcp-wisp.js), with its names, over the catalogue's commands on the running wisp (ADR-0040): hand a task to
//! wisp, list, read, wait on, follow up and stop tasks, and see which agents and machines there are.
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};
use wisp_cli_core::{Renderers, Style};

use crate::client::{CliError, Client, Result};

/// v1's default wait: under typical MCP tool-call timeouts; call `wisp_wait` again for more.
const WAIT_S: u64 = 240;

const NAMES: [&str; 7] = ["wisp_launch", "wisp_list", "wisp_get", "wisp_wait", "wisp_reply", "wisp_stop", "wisp_agents"];

/// Whether `name` is one of these tools.
pub fn is_task_tool(name: &str) -> bool {
    NAMES.contains(&name)
}

/// The tools, as `tools/list` lists them.
pub fn tools() -> Vec<Value> {
    let id = json!({ "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] });
    vec![
        json!({
            "name": "wisp_launch",
            "description": "Hand a coding task to a background agent in wisp. The agent works on its own in the given folder and ends with a presentation (summary, diffs, test results). Returns the task's id at once; pass wait=true to block until it finishes or asks a question.",
            "inputSchema": { "type": "object", "properties": {
                "prompt": { "type": "string", "description": "The task, written as you would brief a colleague: goal, constraints, how to verify." },
                "cwd": { "type": "string", "description": "Absolute path of the project folder. Default: this MCP server's working directory." },
                "agent": { "type": "string", "description": "Harness to run (see wisp_agents); auto lets wisp pick. Default: the user's pick." },
                "mode": { "type": "string", "enum": ["plan", "edits", "auto"], "description": "plan = no file edits, edits = auto-accept edits, auto = no permission prompts. Default: the user's pick." },
                "model": { "type": "string", "description": "Model id for that harness. Default: the user's pick." },
                "effort": { "type": "string", "description": "Thinking level, e.g. low | medium | high." },
                "worktree": { "type": "boolean", "description": "Its own git worktree. Default: on in a project's folder, as `wisp` from the command line (Settings → Worktree per task)." },
                "goal": { "type": "string", "description": "Keep going until this holds: a goal declared by name (goals: in the user's formula), or what done means in words. wisp judges it after every round." },
                "formula": { "description": "Run it under a formula, nothing applied: a formula file (relative to cwd), a list of them, or a formula inline ({ agents: { … } }). Its agents, skills, connectors, teams and goals are this task's; agent and goal may name them.", "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }, { "type": "object" }] },
                "wait": { "type": "boolean", "description": "Block until the agent finishes or asks (up to timeout_seconds)." },
                "timeout_seconds": { "type": "number", "description": format!("Max wait when wait=true. Default {WAIT_S}.") }
            }, "required": ["prompt"] }
        }),
        json!({
            "name": "wisp_list",
            "description": "List wisp tasks, newest first, with state (running | waiting | review | done) and a one-line summary.",
            "inputSchema": { "type": "object", "properties": {
                "state": { "type": "string", "description": "Comma-separated filter, e.g. \"running,waiting\"." },
                "cwd": { "type": "string", "description": "Only tasks in this folder (or below)." },
                "limit": { "type": "number", "description": "Default 20." }
            } }
        }),
        json!({ "name": "wisp_get", "description": "Read a task: its state and its latest presentation as markdown (diffs, tests, next steps), plus any question the agent is waiting on.", "inputSchema": id }),
        json!({
            "name": "wisp_wait",
            "description": "Block until a task finishes or asks a question, then return it like wisp_get. Returns early, still running, after timeout_seconds.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "timeout_seconds": { "type": "number", "description": format!("Default {WAIT_S}.") } }, "required": ["id"] }
        }),
        json!({
            "name": "wisp_reply",
            "description": "Send a follow-up to a task; it resumes the same agent conversation, and answers a question it's waiting on. Sent while it runs, it interrupts it (queue=true waits for the run to end). mode, agent, model and effort switch them from this follow-up on.",
            "inputSchema": { "type": "object", "properties": {
                "id": { "type": "string" },
                "text": { "type": "string" },
                "queue": { "type": "boolean" },
                "mode": { "type": "string", "enum": ["plan", "edits", "auto"] },
                "agent": { "type": "string", "description": "Harness id, e.g. claude, codex, opencode" },
                "model": { "type": "string", "description": "Model id for that harness; empty for its default" },
                "effort": { "type": "string", "description": "Thinking level; empty for the default" }
            }, "required": ["id", "text"] }
        }),
        json!({ "name": "wisp_stop", "description": "Stop a running task.", "inputSchema": id }),
        json!({ "name": "wisp_agents", "description": "Which harnesses are installed, with their models and thinking levels, and the user's machines (other computers, cloud computers).", "inputSchema": { "type": "object", "properties": {} } }),
    ]
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn text(t: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": t }] })
}

/// A task, as `wisp_get` reads it: its line, the question it waits on, and its answer as markdown.
async fn read(c: &Client, id: &str) -> Result<String> {
    let task = c.cmd("runtime.get", json!({ "id": id })).await?;
    let p = c.cmd("runtime.presentation", json!({ "id": id })).await.unwrap_or(Value::Null);
    let blocks = p.get("blocks").and_then(Value::as_array).cloned().unwrap_or_default();
    let answer =
        if blocks.is_empty() { s(&p, "text").to_owned() } else { Renderers::builtin().answer(&blocks, &Style::plain(100)).join("\n") };
    // the question it waits on, its options and how to answer, as `wisp attach` shows it
    let question = crate::scripted::asking(&task);
    Ok(format!("{} [{}] {id}\n\n{answer}{question}", s(&task, "title"), s(&task, "state")).trim_end().to_owned())
}

/// `dir` with its links resolved, as the folder the CLI runs in is spelled (`/tmp/x` → `/private/tmp/x` on macOS);
/// as given when it isn't there (runtime.create says so).
async fn spelled(dir: String) -> String {
    #[cfg(feature = "proc")]
    if let Ok(p) = wisp_proc::canonical(&dir).await {
        return p.display().to_string();
    }
    dir
}

/// `wisp_launch`'s `goal` on `runtime.create`'s args: a declared goal's name is that goal (wisp.goal reads it,
/// ADR-0110); other words are what done means.
fn with_goal(args: &mut Value, a: &Value) {
    if let Some(g) = Some(s(a, "goal").trim()).filter(|g| !g.is_empty()) {
        args["goal"] = json!(g);
    }
}

/// `wisp_launch`'s `formula` (ADR-0111): files relative to the task's folder (one, a comma list or a list), or a formula
/// inline; resolved as `wisp "…" --formula` resolves it, nothing applied.
async fn with_formula(c: &Client, args: &mut Value, a: &Value, dir: &Path) -> Result<()> {
    match a.get("formula") {
        Some(Value::Object(inline)) => {
            let mut doc = inline.clone();
            doc.insert("wisp".into(), json!(1));
            let out = crate::tasks_cli::resolve(c, json!({ "text": Value::Object(doc).to_string(), "name": "formula.json", "cwd": dir.display().to_string(), "task": args.clone() })).await?;
            *args = crate::tasks_cli::create_args(&out["task"], &out["overlay"], &crate::code::Opts::default(), dir);
            Ok(())
        }
        Some(Value::String(list)) => {
            let files: Vec<String> = list.split(',').map(str::trim).filter(|f| !f.is_empty()).map(str::to_owned).collect();
            crate::tasks_cli::with_formula(c, args, &files, &crate::code::Opts::default(), dir).await
        }
        Some(Value::Array(list)) => {
            let files: Vec<String> = list.iter().filter_map(Value::as_str).map(str::to_owned).collect();
            crate::tasks_cli::with_formula(c, args, &files, &crate::code::Opts::default(), dir).await
        }
        _ => Ok(()),
    }
}

/// One tool call; `cwd` is where `wisp mcp` runs (`wisp_launch`'s default folder).
pub async fn call(c: &Client, name: &str, args: &Value, cwd: &Path) -> Value {
    match run(c, name, args, cwd).await {
        Ok(v) => v,
        Err(e) => json!({ "content": [{ "type": "text", "text": e.to_string() }], "isError": true }),
    }
}

async fn run(c: &Client, name: &str, a: &Value, cwd: &Path) -> Result<Value> {
    // whole seconds, at least one (`timeout_seconds` may come as 1.5 or a negative)
    let secs = |a: &Value| {
        // a day at most: `from_secs_f64` panics past `Duration::MAX`, and NaN is v1's default
        a.get("timeout_seconds")
            .and_then(Value::as_f64)
            .filter(|t| !t.is_nan())
            .map_or(Duration::from_secs(WAIT_S), |t| Duration::from_secs_f64(t.clamp(1.0, 86_400.0)))
    };
    let id = || s(a, "id").to_owned();
    Ok(match name {
        "wisp_launch" => {
            // nothing to do is never a run (it would start an agent, maybe a paid one, on an empty prompt)
            if s(a, "prompt").trim().is_empty() {
                return Err(CliError::said("wisp_launch needs a prompt: what the agent should do"));
            }
            let dir = Some(s(a, "cwd")).filter(|d| !d.is_empty()).map_or_else(|| cwd.display().to_string(), str::to_owned);
            // spelled as the folder really is (macOS' `/tmp` is `/private/tmp`), so a project's folder is found as the
            // CLI finds it from its own folder and gets its worktree (audit C5: it ran in the user's checkout)
            let dir = spelled(dir).await;
            let mut args = json!({ "prompt": s(a, "prompt"), "cwd": dir });
            if let Some(w) = a.get("worktree").and_then(Value::as_bool) {
                args["worktree"] = json!(w);
            }
            for k in ["agent", "mode", "model", "effort"] {
                if let Some(v) = Some(s(a, k)).filter(|v| !v.is_empty() && *v != "auto") {
                    args[k] = json!(v);
                }
            }
            with_goal(&mut args, a);
            crate::scripted::in_project(c, &mut args).await;
            with_formula(c, &mut args, a, Path::new(&dir)).await?;
            let made = c.cmd("runtime.create", args).await?;
            let tid = s(&made, "id").to_owned();
            if a.get("wait") == Some(&json!(true)) {
                crate::scripted::wait(c, &tid, secs(a)).await?;
                text(&read(c, &tid).await?)
            } else {
                text(&format!("launched {tid}: {}\nwisp_get or wisp_wait with this id reads it.", s(a, "prompt")))
            }
        }
        "wisp_list" => {
            let states: Vec<&str> = s(a, "state").split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
            let under = s(a, "cwd");
            let limit = a.get("limit").and_then(Value::as_u64).and_then(|l| usize::try_from(l).ok()).unwrap_or(20);
            // filtered here, so the whole list when a filter is given; just the first page when not
            let page = if states.is_empty() && under.is_empty() {
                json!({ "archived": false, "limit": limit })
            } else {
                json!({ "archived": false })
            };
            let mut list = c.cmd("runtime.list", page).await?.as_array().cloned().unwrap_or_default();
            // newest first, as v1's wisp_list
            let made = |t: &Value| t.get("createdAt").or_else(|| t.get("created_at")).and_then(Value::as_i64).unwrap_or(0);
            list.sort_by_key(|t| std::cmp::Reverse(made(t)));
            let lines: Vec<String> = list
                .iter()
                .filter(|t| t.get("archived") != Some(&json!(true)))
                .filter(|t| states.is_empty() || states.contains(&s(t, "state")))
                .filter(|t| under.is_empty() || wisp_runtime_api::paths::is_within(s(t, "cwd"), under))
                .take(limit)
                .map(|t| format!("{}  {:<7}  {}  — {}", s(t, "id"), s(t, "state"), s(t, "title"), s(t, "preview")))
                .collect();
            text(&if lines.is_empty() { "No tasks.".to_owned() } else { lines.join("\n") })
        }
        "wisp_get" => text(&read(c, &id()).await?),
        "wisp_wait" => {
            let tid = id();
            c.cmd("runtime.get", json!({ "id": tid })).await?;
            let w = crate::scripted::wait(c, &tid, secs(a)).await?;
            let still = if w.get("timedOut") == Some(&json!(true)) { "still running: call wisp_wait again for more\n" } else { "" };
            text(&format!("{still}{}", read(c, &tid).await?))
        }
        "wisp_reply" => {
            let mut args = json!({ "id": id(), "text": s(a, "text") });
            if let Some(q) = a.get("queue").and_then(Value::as_bool) {
                args["queue"] = json!(q);
            }
            if !s(a, "mode").is_empty() {
                args["mode"] = json!(s(a, "mode"));
            }
            let run: Map<String, Value> =
                ["agent", "model", "effort"].iter().filter(|k| a.get(**k).is_some()).map(|k| ((*k).to_owned(), json!(s(a, k)))).collect();
            if !run.is_empty() {
                args["run"] = Value::Object(run);
            }
            let r = c.cmd("runtime.followUp", args).await?;
            text(if r.get("queued").is_some_and(|q| !q.is_null() && *q != json!(false)) { "queued" } else { "sent" })
        }
        "wisp_stop" => {
            let tid = id();
            c.cmd("runtime.stop", json!({ "id": tid })).await?;
            // said once it has stopped (audit C16: "stopped" while `runtime.get` still said running)
            let w = crate::scripted::wait(c, &tid, Duration::from_secs(10)).await?;
            text(if w.get("timedOut") == Some(&json!(true)) {
                "stopping: its run hasn't let go yet (wisp_get reads it once it has)"
            } else {
                "stopped"
            })
        }
        "wisp_agents" => {
            let harnesses = c.cmd("accounts.harnesses", Value::Null).await.unwrap_or(Value::Null);
            let machines = c.cmd("machines.list", json!({})).await.unwrap_or(Value::Null);
            text(&serde_json::to_string_pretty(&json!({ "harnesses": harnesses, "machines": machines })).unwrap_or_default())
        }
        other => json!({ "content": [{ "type": "text", "text": format!("{other}: not a tool here") }], "isError": true }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_task_tools_are_v1_s_with_their_schemas() {
        let t = tools();
        let names: Vec<&str> = t.iter().map(|x| x["name"].as_str().unwrap_or_default()).collect();
        assert_eq!(names, NAMES);
        assert!(t.iter().all(|x| x["inputSchema"]["type"] == "object" && !x["description"].as_str().unwrap_or_default().is_empty()));
        assert_eq!(t[0]["inputSchema"]["required"], json!(["prompt"]));
        assert!(is_task_tool("wisp_launch") && !is_task_tool("wisp_history"));
    }
}
