//! What to spawn for a Claude Code run (v1 adapters/claude.js `run()`): `claude -p --output-format
//! stream-json …`, the prompt on stdin. Plain data from a [`HarnessRun`]; no I/O.

use std::collections::BTreeSet;

use wisp_runtime_api::harness::{HarnessRun, Launch, LaunchFile, MCP_CONFIG_FILE, Stdin};
use wisp_runtime_api::model::{Attachment, Mode};

/// The program when detect found none and `WISP_CLAUDE_BIN` isn't set.
pub const DEFAULT_BIN: &str = "claude";

/// Its own tools wisp's gate always asks about (v1 claude.js:26): a /btw's and a probe's, which only answer.
const GATED: [&str; 5] = ["Edit", "MultiEdit", "Write", "NotebookEdit", "Bash"];
/// What a side discussion runs without asking (audit user-side-full-access): it has its main task's access, and only its
/// file writes (`EDIT_TOOLS`) go to wisp first, where one to a file the main task changed becomes a proposal there.
const SIDE_FREE: [&str; 3] = ["Bash", "WebFetch", "WebSearch"];
/// A side run that only answers (v1 claude.js:21).
const LEAN: [&str; 7] =
    ["--tools", "", "--strict-mcp-config", "--setting-sources", "user", "--disable-slash-commands", "--no-session-persistence"];
/// A repo the user hasn't trusted: none of its hooks or MCP servers (v1 claude.js:25).
const UNTRUSTED: [&str; 3] = ["--setting-sources", "user", "--strict-mcp-config"];
/// A parent Claude Code session must not leak into the child (v1 pty.js:131), nor costly extras (:135).
pub const ENV_REMOVE: [&str; 10] = [
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_CODE_AGENT_PROGRESS_SUMMARIES",
];
const SUBAGENT_MODELS: [&str; 2] = ["haiku", "sonnet"];
pub const APPROVE_TOOL: &str = "mcp__wisp__approve";
/// Plan that runs commands (`HarnessRun::plan_commands`, v1 #794): Claude Code's own plan mode refuses any command
/// with a side effect, so the run goes without prompts and only these file-editing tools are off (v1 `EDIT_TOOLS`).
pub const EDIT_TOOLS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];
/// What the agent hears in Plan that runs commands (v1 runner.js `PLAN_NOTE`), after wisp's system prompt.
pub const PLAN_NOTE: &str = "\n\n# Plan mode\n\nThis task is in Plan mode: don't change the project's files. The file-editing tools are off; don't write files from the shell either (no `sed -i`, `>` into a file, `patch`, `git commit`). Everything else is yours: run commands, tests, builds and the app, read logs, use wisp's tools and the connectors. Don't hand the user a command to run that you could run yourself: run it and report what it showed. End with the plan: what to change, where, and why.\n";

pub(crate) fn permission_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Plan => "plan",
        Mode::Edits => "acceptEdits",
        Mode::Auto => "bypassPermissions",
    }
}

/// v1 common.js `attachmentNote`, with Claude Code's own words for how to read them.
fn attachment_note(files: &[Attachment]) -> String {
    let with_path: Vec<&Attachment> = files.iter().filter(|f| f.path.is_some()).collect();
    if with_path.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = with_path
        .iter()
        .map(|f| format!("- {}  ({}, {})", f.path.as_deref().unwrap_or_default(), f.mime, wisp_harness_kit::protocol::size_words(f.size)))
        .collect();
    format!(
        "\n\n<attachments>\nThe user attached these files. Read images and PDFs with the Read tool; for video/audio/other formats use the shell (e.g. ffmpeg, ffprobe) if needed.\n{}\n</attachments>",
        lines.join("\n")
    )
}

fn dir_of(p: &str) -> String {
    match p.rfind(['/', '\\']) {
        Some(0) => "/".to_owned(),
        Some(i) => p[..i].to_owned(),
        None => ".".to_owned(),
    }
}

/// The wisp binary the offload hook runs (this process: the hook is `wisp hook offload`, no Node in v2).
fn wisp_exe() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.display().to_string())
}

/// Claude Code's `PreToolUse` hook on Bash that sends long output to a file (ADR-0079, `wisp hook offload`): only when
/// the run names a folder for it (`WISP_OFFLOAD_DIR`, set by wisp.threads), never for a lean side run (it only
/// answers).
fn offload_hook(run: &HarnessRun) -> Option<serde_json::Value> {
    run.env.get(wisp_runtime_api::points::OFFLOAD_DIR_ENV).filter(|d| !d.is_empty() && !run.lean)?;
    let exe = wisp_exe()?;
    let command = format!("\"{}\" hook offload", exe.replace('"', "\\\""));
    Some(serde_json::json!({ "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": command, "timeout": 10 }] }] }))
}

/// One --settings: the gate's ask rules, and the hook that sends long Bash output to a file (ADR-0079).
fn settings(run: &HarnessRun) -> Option<String> {
    let mut s = serde_json::Map::new();
    if run.gate {
        let ask: &[&str] = if run.side { &EDIT_TOOLS } else { &GATED };
        s.insert("permissions".into(), serde_json::json!({ "ask": ask }));
    }
    if let Some(hook) = offload_hook(run) {
        s.insert("hooks".into(), hook);
    }
    (!s.is_empty()).then(|| serde_json::Value::Object(s).to_string())
}

/// `--resume`, and for a fork `--fork-session`: a fork from an earlier turn ends its copy where that turn did, and a
/// forked task's copy is its own conversation from now on, saved, or its next turn resumes nothing (a /btw's or a
/// probe's copy is thrown away).
fn resume(run: &HarnessRun, args: &mut Vec<String>) {
    let Some(r) = run.resume_id.as_ref().filter(|r| !r.is_empty()) else { return };
    args.push("--resume".into());
    args.push(r.clone());
    if !run.fork {
        return;
    }
    args.push("--fork-session".into());
    if let Some(at) = run.resume_at.as_ref().filter(|a| !a.is_empty()) {
        args.push("--resume-session-at".into());
        args.push(at.clone());
    }
    if !run.fork_keep {
        args.push("--no-session-persistence".into());
    }
}

/// The launch for `run`, with `bin` the program to start.
pub fn launch(run: &HarnessRun, bin: &str) -> Launch {
    // Plan that runs commands: a task you started (the runtime says), never past a side discussion's gate
    let plan_runs = run.plan_commands && run.mode == Mode::Plan && !run.gate;
    let mut args: Vec<String> = vec!["-p".into(), "--output-format".into(), "stream-json".into()];
    args.push(if run.lean { "--system-prompt" } else { "--append-system-prompt" }.into());
    args.push(if plan_runs { format!("{}{PLAN_NOTE}", run.system_prompt) } else { run.system_prompt.clone() });
    args.push("--permission-mode".into());
    args.push(
        if run.gate {
            "default"
        } else if plan_runs {
            "bypassPermissions"
        } else {
            permission_mode(run.mode)
        }
        .into(),
    );
    if let Some(s) = settings(run) {
        args.push("--settings".into());
        args.push(s);
    }
    if run.lean {
        args.extend(LEAN.iter().map(|s| (*s).to_owned()));
    } else if run.untrusted {
        args.extend(UNTRUSTED.iter().map(|s| (*s).to_owned()));
    }
    let compact = run.prompt.trim().strip_prefix("/compact").is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
    if run.skills == "off" && !run.lean && !compact {
        args.push("--disable-slash-commands".into());
    }
    for (flag, v) in [("--model", &run.model), ("--effort", &run.effort)] {
        if !v.is_empty() {
            args.push(flag.into());
            args.push(v.clone());
        }
    }
    resume(run, &mut args);
    // as a file of the run's own (0600), never inline: a connector's secret env and the run's wisp token on argv are
    // anyone's to read with `ps` (ADR-0116, audit I IS3)
    let mut files = Vec::new();
    if !run.mcp_servers.is_empty() {
        args.push("--mcp-config".into());
        args.push(format!("{{tmp}}/{MCP_CONFIG_FILE}"));
        files.push(LaunchFile { path: MCP_CONFIG_FILE.into(), contents: serde_json::json!({ "mcpServers": run.mcp_servers }).to_string() });
    }
    // a side discussion's writes go to wisp in whatever mode it runs (its main task's, Full auto too)
    if run.mcp_servers.contains_key("wisp") && (run.mode == Mode::Edits || (run.gate && run.side)) {
        args.push("--permission-prompt-tool".into());
        args.push(APPROVE_TOOL.into());
    }
    let free = SIDE_FREE.iter().filter(|_| run.gate && run.side).map(|t| (*t).to_owned());
    let allowed: Vec<String> = run.mcp_servers.keys().map(|n| format!("mcp__{n}")).chain(run.grants.iter().cloned()).chain(free).collect();
    if !allowed.is_empty() {
        args.push("--allowedTools".into());
        args.extend(allowed);
    }
    // its built-in subagents would bypass the user's team; Plan that runs commands has its edit tools off
    let off: Vec<&str> = if run.delegating { vec!["Task", "Agent", "Workflow"] } else { Vec::new() }
        .into_iter()
        .chain(EDIT_TOOLS.into_iter().filter(|_| plan_runs))
        .collect();
    if !off.is_empty() {
        args.push("--disallowedTools".into());
        args.extend(off.into_iter().map(str::to_owned));
    }
    let mut seen = BTreeSet::new();
    let dirs: Vec<String> = run
        .dirs
        .iter()
        .cloned()
        .chain(run.attachments.iter().filter_map(|a| a.path.as_deref().map(dir_of)))
        .filter(|d| seen.insert(d.clone()))
        .collect();
    if !dirs.is_empty() {
        args.push("--add-dir".into());
        args.extend(dirs);
    }
    args.push("--verbose".into()); // ends the variadic flags above
    let mut env = std::collections::BTreeMap::new();
    if SUBAGENT_MODELS.contains(&run.subagent_model.as_str()) {
        env.insert("CLAUDE_CODE_SUBAGENT_MODEL".to_owned(), run.subagent_model.clone());
        env.insert("CLAUDE_CODE_WORKFLOW_SUBAGENT_MODEL".to_owned(), run.subagent_model.clone());
    }
    // without the CLI's nonessential traffic (v1 lean.js: a lean side run, or a task Settings or the task made lean)
    if run.quiet {
        env.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_owned(), "1".to_owned());
    }
    let prompt = format!("{}{}", run.prompt, attachment_note(&run.attachments));
    Launch {
        program: bin.to_owned(),
        args,
        env,
        env_remove: ENV_REMOVE.iter().map(|s| (*s).to_owned()).collect(),
        cwd: run.cwd.clone(),
        stdin: Stdin::Bytes(prompt.into_bytes()),
        files,
        ..Launch::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> HarnessRun {
        HarnessRun { prompt: "fix the login".into(), cwd: "/w".into(), system_prompt: "SYS".into(), ..HarnessRun::default() }
    }

    #[test]
    fn a_plain_run_is_v1s_argv_with_the_prompt_on_stdin() {
        let l = launch(&run(), "claude");
        assert_eq!(l.program, "claude");
        assert_eq!(
            l.args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--append-system-prompt",
                "SYS",
                "--permission-mode",
                "bypassPermissions",
                "--verbose"
            ]
        );
        assert_eq!(l.stdin, Stdin::Bytes(b"fix the login".to_vec()));
        assert_eq!(l.cwd, "/w");
        assert!(l.env_remove.iter().any(|k| k == "CLAUDECODE"));
    }

    #[test]
    fn flags_follow_the_run() {
        let mut r = run();
        r.mode = Mode::Edits;
        r.model = "opus".into();
        r.effort = "high".into();
        r.resume_id = Some("c1".into());
        r.fork = true;
        r.mcp_servers.insert("wisp".into(), serde_json::json!({"command": "x"}));
        r.delegating = true;
        r.dirs = vec!["/a".into()];
        r.attachments =
            vec![Attachment { path: Some("/up/pic.png".into()), mime: "image/png".into(), size: 3000, ..Attachment::default() }];
        let a = launch(&r, "/bin/claude").args.join(" ");
        assert!(a.contains(
            "--permission-mode acceptEdits --model opus --effort high --resume c1 --fork-session --no-session-persistence --mcp-config"
        ));
        assert!(a.contains("--permission-prompt-tool mcp__wisp__approve --allowedTools mcp__wisp --disallowedTools Task Agent Workflow --add-dir /a /up --verbose"));
        assert!(a.contains("--mcp-config {tmp}/wisp-mcp.json --permission-prompt-tool"), "a file of the run's own: {a}");
        // a forked task's first run keeps its copy (audit fork-claude-session-lost: its next turn resumed a session
        // that was never written); a /btw's copy (above) is thrown away
        let kept = launch(&HarnessRun { fork_keep: true, ..r.clone() }, "claude").args.join(" ");
        assert!(kept.contains("--resume c1 --fork-session --mcp-config"), "{kept}");
        assert!(!kept.contains("--no-session-persistence"), "{kept}");
        let Stdin::Bytes(b) = launch(&r, "claude").stdin else { panic!("stdin") };
        assert!(String::from_utf8_lossy(&b).contains("- /up/pic.png  (image/png, 3 KB)"));
        // a 13-byte file is 13 B, not "1 KB" (audit B #19)
        r.attachments = vec![Attachment { path: Some("/up/a.txt".into()), mime: "text/plain".into(), size: 13, ..Attachment::default() }];
        let Stdin::Bytes(b) = launch(&r, "claude").stdin else { panic!("stdin") };
        assert!(String::from_utf8_lossy(&b).contains("- /up/a.txt  (text/plain, 13 B)"));
    }

    /// Audit fork-turn-includes-later: a fork from a turn before the latest ends its copy where that turn did
    /// (`--resume-session-at`, with `--fork-session`); a fork from the latest turn names no point.
    #[test]
    fn a_fork_at_an_earlier_turn_resumes_the_conversation_at_that_turn() {
        let at = HarnessRun { resume_id: Some("c1".into()), fork: true, fork_keep: true, resume_at: Some("msg-7".into()), ..run() };
        let a = launch(&at, "claude").args.join(" ");
        assert!(a.contains("--resume c1 --fork-session --resume-session-at msg-7"), "{a}");
        let latest = launch(&HarnessRun { resume_at: None, ..at.clone() }, "claude").args;
        assert!(!latest.iter().any(|x| x == "--resume-session-at"));
        let plain = launch(&HarnessRun { fork: false, ..at }, "claude").args;
        assert!(!plain.iter().any(|x| x == "--resume-session-at"), "only a fork ends its copy early");
    }

    /// ADR-0079: a run that names an offload folder gets the `PreToolUse` hook on Bash, beside the gate's rules in the
    /// one --settings; a lean run and a run without the folder get none.
    #[test]
    fn a_run_with_an_offload_folder_gets_the_bash_hook() {
        let settings = |r: &HarnessRun| {
            launch(r, "claude")
                .args
                .windows(2)
                .find(|w| w[0] == "--settings")
                .map(|w| serde_json::from_str::<serde_json::Value>(&w[1]).unwrap())
        };
        assert_eq!(settings(&run()), None);
        let mut r = run();
        r.env.insert(wisp_runtime_api::points::OFFLOAD_DIR_ENV.into(), "/home/.wisp/threads/out/t1".into());
        r.gate = true;
        let s = settings(&r).unwrap();
        let hook = &s["hooks"]["PreToolUse"][0];
        assert_eq!(hook["matcher"], "Bash");
        assert!(hook["hooks"][0]["command"].as_str().unwrap().ends_with("\" hook offload"));
        assert!(s["permissions"]["ask"].is_array(), "the gate's rules stay in the same --settings");
        assert_eq!(launch(&r, "claude").args.iter().filter(|a| *a == "--settings").count(), 1);
        r.lean = true;
        r.gate = false;
        assert_eq!(settings(&r), None, "a lean run only answers");
    }

    #[test]
    fn gate_lean_and_skills() {
        let mut r = run();
        r.gate = true;
        r.skills = "off".into();
        let a = launch(&r, "claude").args;
        assert!(a.windows(2).any(|w| w == ["--permission-mode", "default"]));
        assert!(a.iter().any(|x| x == "--disable-slash-commands"));
        r.prompt = "/compact".into();
        assert!(!launch(&r, "claude").args.iter().any(|x| x == "--disable-slash-commands"));
        let mut l = run();
        l.lean = true;
        let a = launch(&l, "claude").args;
        assert_eq!(a[3], "--system-prompt");
        assert!(a.iter().any(|x| x == "--strict-mcp-config"));
    }

    /// Audit user-side-full-access: a side discussion's commands and web tools never reach wisp's approve tool; only its
    /// file writes ask (so a write to a file the main task changed can become a proposal there), in Full auto too. A
    /// /btw's gate still asks about everything (it only answers).
    #[test]
    fn a_side_discussion_asks_wisp_about_its_file_writes_only() {
        let settings = |r: &HarnessRun| {
            let a = launch(r, "claude").args;
            serde_json::from_str::<serde_json::Value>(flag(&a, "--settings")).unwrap()
        };
        let mut side = run();
        side.gate = true;
        side.side = true;
        side.mode = Mode::Auto;
        side.mcp_servers.insert("wisp".into(), serde_json::json!({"command": "x"}));
        let ask = settings(&side)["permissions"]["ask"].clone();
        assert_eq!(ask, serde_json::json!(EDIT_TOOLS), "no Bash in the ask list");
        let a = launch(&side, "claude").args;
        assert_eq!(flag(&a, "--permission-prompt-tool"), APPROVE_TOOL, "its writes reach wisp in Full auto too");
        let allowed = after(&a, "--allowedTools");
        for t in ["Bash", "WebFetch", "WebSearch"] {
            assert!(allowed.iter().any(|x| x == t), "{t} runs without a card: {allowed:?}");
        }
        let btw = HarnessRun { side: false, mode: Mode::Plan, ..side };
        assert_eq!(settings(&btw)["permissions"]["ask"], serde_json::json!(GATED), "a /btw's gate is as it was");
        let a = launch(&btw, "claude").args;
        assert!(!a.iter().any(|x| x == "--permission-prompt-tool") && !after(&a, "--allowedTools").iter().any(|x| x == "Bash"));
    }

    fn flag<'a>(a: &'a [String], f: &str) -> &'a str {
        a.iter().position(|x| x == f).and_then(|i| a.get(i + 1)).map_or("", String::as_str)
    }

    fn after(a: &[String], f: &str) -> Vec<String> {
        let Some(i) = a.iter().position(|x| x == f) else { return Vec::new() };
        a[i + 1..].iter().take_while(|x| !x.starts_with("--")).cloned().collect()
    }

    /// v1 #794 test/plan-mode.test.js: Plan with `plan_commands` runs without prompts, its edit tools off and the Plan
    /// note told; without it, Claude Code's own read-only plan mode; a side discussion's gate wins; only Plan.
    #[test]
    fn plan_that_runs_commands_has_no_prompts_its_edit_tools_off_and_the_plan_note_v1_794() {
        let strict = launch(&HarnessRun { mode: Mode::Plan, ..run() }, "claude").args;
        assert_eq!(flag(&strict, "--permission-mode"), "plan");
        assert!(!strict.iter().any(|x| x == "--disallowedTools"));
        assert!(!flag(&strict, "--append-system-prompt").contains("# Plan mode"));

        let open = launch(&HarnessRun { mode: Mode::Plan, plan_commands: true, delegating: true, ..run() }, "claude").args;
        assert_eq!(flag(&open, "--permission-mode"), "bypassPermissions", "Claude Code's plan mode refuses commands with side effects");
        assert_eq!(after(&open, "--disallowedTools"), ["Task", "Agent", "Workflow", "Edit", "MultiEdit", "Write", "NotebookEdit"]);
        let sys = flag(&open, "--append-system-prompt");
        assert!(sys.starts_with("SYS\n\n# Plan mode\n\n"), "{sys}");
        assert!(sys.contains("don't change the project's files") && sys.contains("run commands, tests, builds and the app"));

        let alone = launch(&HarnessRun { mode: Mode::Plan, plan_commands: true, ..run() }, "claude").args;
        assert_eq!(after(&alone, "--disallowedTools"), EDIT_TOOLS);

        let gated = launch(&HarnessRun { mode: Mode::Plan, plan_commands: true, gate: true, ..run() }, "claude").args;
        assert_eq!(flag(&gated, "--permission-mode"), "default", "a side discussion's gate wins");
        assert!(!gated.iter().any(|x| x == "--disallowedTools"));

        let edits = launch(&HarnessRun { mode: Mode::Edits, plan_commands: true, ..run() }, "claude").args;
        assert_eq!(flag(&edits, "--permission-mode"), "acceptEdits", "only Plan");
        assert!(!edits.iter().any(|x| x == "--disallowedTools"));
        assert!(!flag(&edits, "--append-system-prompt").contains("# Plan mode"));
    }

    /// v1 lean.js `LEAN_ENV`: a quiet run goes without Claude Code's nonessential traffic.
    #[test]
    fn a_quiet_run_turns_the_nonessential_traffic_off() {
        assert!(!launch(&run(), "claude").env.contains_key("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"));
        let q = HarnessRun { quiet: true, ..run() };
        assert_eq!(launch(&q, "claude").env.get("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC").map(String::as_str), Some("1"));
    }

    /// Audit I IS3: a connector's secret env and the run's wisp token were on Claude's argv (`--mcp-config '{…}'`), where
    /// any local user reads them with `ps`. They go in the run's own MCP config file; argv names only its path.
    #[test]
    fn mcp_servers_go_in_a_file_never_on_argv_audit_i_is3() {
        let mut r = run();
        r.mcp_servers.insert(
            "wisp".into(),
            serde_json::json!({ "type": "http", "url": "http://127.0.0.1:1/api/mcp", "headers": { "x-wisp-token": "tok-SECRET-1" } }),
        );
        r.mcp_servers.insert("docs".into(), serde_json::json!({ "command": "docs-mcp", "env": { "DOCS_TOKEN": "env-SECRET-2" } }));
        let l = launch(&r, "claude");
        let argv = l.args.join(" ");
        assert!(!argv.contains("SECRET") && !argv.contains("x-wisp-token"), "{argv}");
        let at = l.args.iter().position(|a| a == "--mcp-config").unwrap();
        assert_eq!(l.args[at + 1], "{tmp}/wisp-mcp.json");
        assert_eq!(l.files.len(), 1);
        let cfg: serde_json::Value = serde_json::from_str(&l.files[0].contents).unwrap();
        assert_eq!(cfg["mcpServers"]["wisp"]["headers"]["x-wisp-token"], "tok-SECRET-1");
        assert_eq!(cfg["mcpServers"]["docs"]["env"]["DOCS_TOKEN"], "env-SECRET-2");
        assert!(l.env.values().all(|v| !v.contains("SECRET")));
    }
}
