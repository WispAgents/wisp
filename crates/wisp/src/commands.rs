//! `wisp <command>` for the curated verbs that talk to a running wisp (one starts if none answers), all
//! on v2's commands (ADR-0031): launch, ls, show, wait, reply, btw, stop, ship, merge, recipes, run,
//! plugin, crew.
use std::time::Duration;

use serde_json::{Value, json};

use crate::args::Parsed;
use crate::client::{CliError, Client, Result};
use crate::scripted::{self as sc, need, out};

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

#[allow(clippy::many_single_char_names)]
pub async fn run(p: &Parsed, cmd: &str, args: &[String], cwd: &str, port: u16) -> Result<()> {
    let f = &p.flags;
    let c = Client::connect(port, true).await?;
    // `wisp show last`, `stop last`, …: the newest task (QA round 14 #7)
    let resolved: Vec<String>;
    let args = if matches!(cmd, "show" | "stop" | "wait" | "reply" | "btw" | "held") && args.first().is_some_and(|a| a == "last") {
        resolved = std::iter::once(sc::task_id(&c, "last").await?).chain(args[1..].iter().cloned()).collect();
        &resolved[..]
    } else {
        args
    };
    match cmd {
        "launch" => launch(&c, p, args, cwd).await,
        "ls" | "list" => sc::ls(&c, f).await,
        "show" => sc::show(&c, f, need(args.first(), "session id")?, None).await,
        "wait" => {
            let id = need(args.first(), "session id")?;
            // a task that isn't there is said at once, never waited for (QA round 11: `wisp wait x` hung 30 minutes)
            c.cmd("runtime.get", json!({ "id": id })).await?;
            let w = sc::wait(&c, id, Duration::from_secs(30 * 60)).await?;
            sc::show(&c, f, id, Some(w)).await
        }
        "reply" => sc::reply(&c, f, args).await,
        "btw" => {
            let id = need(args.first(), "session id")?;
            let q = args[1..].join(" ");
            let r = c.cmd("runtime.btw", json!({ "id": id, "question": need(Some(&q), "question")? })).await?;
            sc::btw_out(f, &sc::btw_answer(&c, id, s(&r, "id")).await?);
            Ok(())
        }
        "stop" => {
            let r = c.cmd("runtime.stop", json!({ "id": need(args.first(), "session id")? })).await?;
            out(f, &r, "  ✓ stopped");
            Ok(())
        }
        "held" => held(&c, f, args).await,
        "ship" => {
            let id = need(args.first(), "session id")?;
            let task = c.cmd("runtime.get", json!({ "id": id })).await?;
            let pr = c.cmd("ship.ship", json!({ "session": id, "title": s(&task, "title"), "draft": f.draft })).await?;
            let draft = if f.draft { " (draft)" } else { "" };
            out(
                f,
                &pr,
                &format!("  ✓ shipped{draft}: {}", pr.pointer("/pr/url").or_else(|| pr.get("url")).and_then(Value::as_str).unwrap_or("")),
            );
            Ok(())
        }
        "merge" => {
            let id = need(args.first(), "session id")?;
            let had_tree = c
                .cmd("runtime.get", json!({ "id": id }))
                .await
                .is_ok_and(|t| t.pointer("/ext/wisp.worktrees/path").is_some_and(Value::is_string));
            let r = c.cmd("ship.merge", json!({ "session": id })).await?;
            out(f, &r, &merged_words(&r, had_tree));
            Ok(())
        }
        "recipes" => recipes(&c, f).await,
        "run" => recipe_run(&c, p, args, cwd).await,
        "plugin" | "plugins" => crate::plugin_cmd::run(&c, p, args).await,
        "crew" => crate::plugin_cmd::crew(&c, p, args).await,
        other => Err(CliError::said(format!("unknown command {other}"))),
    }
}

/// `--compare a[:model],b`: one task per harness, the same ask (ADR-0027: compare is reruns side by side).
#[allow(clippy::many_single_char_names)] // c, p, f as in the rest of the CLI
async fn launch(c: &Client, p: &Parsed, args: &[String], cwd: &str) -> Result<()> {
    let f = &p.flags;
    if let Some(no) = sc::unknown_agent(c, f.agent.as_deref()).await {
        return Err(CliError::said(no.trim().trim_start_matches("✗ ")));
    }
    let prompt = args.join(" ");
    if !f.tries.is_empty() {
        // Try it two ways: one harness, run B with what --try names changed (ADR-0056)
        return crate::trial_cli::launch(c, f, &prompt, cwd).await;
    }
    let mut made = Vec::new();
    if f.compare.is_some() {
        // one group for the runs: the task's More then offers Compare runs (v1 compare.js, F-comp-30)
        let group = format!("g-{}", crate::services::hex(&rand::random::<[u8; 8]>()));
        for r in &crate::args::compare_runs(f) {
            let mut a = sc::launch_args(f, &prompt, cwd);
            sc::in_project(c, &mut a).await;
            a["ext"] = json!({ "wisp.runtime": { "group": group } });
            a["agent"] = json!(r.agent);
            // each run's own model: `--model` is the `--agent` run's, never another harness's
            match (&r.model, a.as_object_mut()) {
                (Some(m), _) => a["model"] = json!(m),
                (None, Some(o)) => drop(o.remove("model")),
                (None, None) => {}
            }
            made.push(c.cmd("runtime.create", a).await?);
        }
    } else {
        let mut a = sc::launch_args(f, &prompt, cwd);
        sc::in_project(c, &mut a).await;
        let home =
            crate::services::home(std::env::var("WISP_HOME").ok().as_deref(), std::env::var_os("HOME").map(std::path::PathBuf::from));
        crate::termctx_cli::attach(c, p, &mut a, &prompt, cwd, &home).await;
        made.push(c.cmd("runtime.create", a).await?);
    }
    if !f.wait {
        let lines: Vec<String> = made.iter().map(|m| format!("  ◌ launched: {}  wisp attach {}", s(m, "id"), s(m, "id"))).collect();
        out(f, &json!(made), &lines.join("\n"));
        return Ok(());
    }
    for m in &made {
        let id = s(m, "id").to_owned();
        if !f.json {
            eprintln!("  ◌ launched: {id} — waiting…");
        }
        let w = sc::wait(c, &id, Duration::from_secs(30 * 60)).await?;
        sc::show(c, f, &id, Some(w)).await?;
    }
    Ok(())
}

/// `wisp recipes`: the declared tasks (`tasks:` in a formula; recipes are tasks with `{params}`).
async fn recipes(c: &Client, f: &crate::args::Flags) -> Result<()> {
    let list = c.cmd("automations.tasks", json!({})).await?;
    let lines: Vec<String> = list
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            let params = wisp_automations_params(s(r, "prompt"));
            let prompt: String = s(r, "prompt").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect();
            format!("{:<24}  {:<20}  {prompt}", s(r, "name"), params)
        })
        .collect();
    out(
        f,
        &list,
        &if lines.is_empty() {
            "No declared tasks yet: add some under tasks: in a formula (wisp apply -f).".into()
        } else {
            lines.join("\n")
        },
    );
    Ok(())
}

fn wisp_automations_params(prompt: &str) -> String {
    let mut out = Vec::new();
    let mut rest = prompt;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        out.push(rest[a..=a + b].to_owned());
        rest = &rest[a + b + 1..];
    }
    out.join(" ")
}

/// `wisp run <name> [values]` reached here: no workflow or declared task has that name, so it's a task.
async fn recipe_run(c: &Client, p: &Parsed, args: &[String], cwd: &str) -> Result<()> {
    let mut all = vec!["run".to_owned()];
    all.extend(args.iter().cloned());
    let prompt = all.join(" ");
    let mut a = sc::launch_args(&p.flags, &prompt, cwd);
    sc::in_project(c, &mut a).await;
    let m = c.cmd("runtime.create", a).await?;
    out(&p.flags, &m, &format!("  ◌ launched: {prompt}  ({})\n    wisp attach {}", s(&m, "id"), s(&m, "id")));
    Ok(())
}

/// `wisp held <id> start|without|skip`: a turn held before it started (audit D4: the only way was `wisp runtime
/// release <id> <turn-id> <action>`, its turn dug out of `runtime.turns`).
async fn held(c: &Client, f: &crate::args::Flags, args: &[String]) -> Result<()> {
    let id = need(args.first(), "session id")?;
    let action = need(args.get(1), "start, without or skip")?;
    let r = c.cmd("runtime.held", json!({ "id": id, "action": action })).await?;
    let said = match action {
        "start" => "  ✓ started",
        "without" => "  ✓ started without the repo's own programs",
        _ => "  ✓ skipped",
    };
    out(f, &r, said);
    Ok(())
}

/// What `wisp merge` did, as the page's toast says it (audit D9: only "✓ merged"): the PR, your local base branch, and
/// what became of the worktree (kept, and why, or removed).
pub fn merged_words(r: &Value, had_tree: bool) -> String {
    let n = r.pointer("/pr/number").and_then(Value::as_u64).map_or_else(String::new, |n| format!(" PR #{n}"));
    let mut lines = vec![format!("  ✓ merged{n}")];
    if let Some(t) = r.pointer("/sync/text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        let mark = if r.pointer("/sync/ok").and_then(Value::as_bool) == Some(false) { "!" } else { "·" };
        lines.push(format!("  {mark} {t}"));
    }
    let kept: Vec<&str> = r.get("kept").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    if kept.is_empty() {
        if had_tree {
            lines.push("  · The worktree was removed; its branch stays.".to_owned());
        }
    } else {
        lines.extend(kept.iter().map(|k| format!("  · {k}")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod merged_tests {
    use super::merged_words;
    use serde_json::json;

    /// Audit D9: the CLI said only "✓ merged"; it says what the toast says.
    #[test]
    fn merge_says_the_pr_your_base_branch_and_the_worktree_audit_d9() {
        let r = json!({ "pr": { "number": 3 }, "sync": { "ok": true, "text": "Local main is up to date with origin/main." } });
        assert_eq!(
            merged_words(&r, true),
            "  ✓ merged PR #3\n  · Local main is up to date with origin/main.\n  · The worktree was removed; its branch stays."
        );
        let kept = json!({ "pr": { "number": 1 }, "sync": { "ok": false, "text": "Local main has commits of its own." }, "kept": ["The worktree /w was kept: it has 1 uncommitted change."] });
        assert_eq!(
            merged_words(&kept, true),
            "  ✓ merged PR #1\n  ! Local main has commits of its own.\n  · The worktree /w was kept: it has 1 uncommitted change."
        );
        assert_eq!(merged_words(&json!({ "pr": { "number": 2 } }), false), "  ✓ merged PR #2", "in place: nothing about a worktree");
    }
}
