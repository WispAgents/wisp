//! A stand-in for the claude / codex / … CLIs (port of the core of v1 test/fixtures/fake-agent.mjs).
//! The CLI it plays comes from its file name (link it as `claude`, `codex`, …). It speaks each one's
//! stream format, edits `math.js` for real (`a - b` → `a + b`) and ends with a wisp block.
//!
//!   `FAKE_RECORD`      directory; each run appends {argv, stdin, cwd, pid, live, codexHome, subagentModel} to
//!                      `<name>.jsonl`; `live` is a file it holds locked until it ends (see `hold_live`)
//!   `FAKE_HANG=1`      stop after the first events and wait to be killed
//!   `FAKE_GRANDCHILD`  a file: a run starts a copy of this binary that only sleeps, then appends `<own pid> <its pid>\n`
//!                      once both hold `<file>.<pid>.live` locked (see `hold_live`)
//!   `FAKE_FAIL=1`      exit 1 with an error on stderr, no result
//!   `FAKE_LIMIT_ONCE`  a file: the first run with a prompt hits a usage limit (stderr, exit 1); later runs work
//!   `FAKE_DELAY` / `FAKE_LINGER`  ms to wait before answering / after the result
//!   `FAKE_GATE`        directory; a run whose prompt is `gate <name> …` (or has a `\n\ngate <name>` paragraph) waits
//!                      until the file `<dir>/<name>` exists: the test holds the turn running until it creates it
//!   `FAKE_GATE_ALL`    a gate name: every run (no prompt prefix needed, e.g. one a Slack message started) waits on it
//!   `FAKE_SAY`         `needle=>words;;…`: a run whose prompt holds a needle ends its answer with the words
//!   `FAKE_REPLAY`      claude: print this .jsonl as its stream and exit (a recorded run)
//!   `FAKE_STREAM`      claude: events a second; a run streams assistant text for `FAKE_STREAM_MS` (5000) after its
//!                      init, each line `FAKE_STREAM_BYTES` (400) long, its text starting `~stamp:<pid>:<n>:<unix µs>~`
//!                      (when it was written: spikes/server-budgets times each one to the clients' patches), paced
//!                      against its start so a slow write never makes the stream fall behind, then ends as usual
//!   `FAKE_STREAM_MIX=agent`  with `FAKE_STREAM`: the lines are a real agent's steps instead of text, `MIX` in turn
//!                      (thinking, Bash calls, their results, answers), each sized so the step wisp keeps is its bytes
//!   `FAKE_CODEX_USAGE` a file: codex app-server's `account/rateLimits/read` answers its JSON as `rateLimits`
//!   `FAKE_CLI_VERSION` what --version says (9.9.9); `FAKE_AUTH_PROFILE`, `FAKE_AUTH_FAIL` for `auth status --text`
//!   TOPIC[…] in the prompt: the hero names it
//!   named `exec-plugin`: an exec plugin (JSON-RPC on stdio) that answers `initialize`, starts `FAKE_GRANDCHILD`'s
//!   sleeper then, and ignores its input closing (it never exits on its own); `FAKE_EXEC_CRASH=1`: exits 1 right after
//!   starting the sleeper (a crash that leaves its child behind)
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
use std::io::{Read, Write};
use std::path::Path;

use serde_json::{Value, json};

/// A run: it was given a prompt. A harness's detect probe (`models`, `--help`, `auth list`) has none, runs in
/// the server's folder, and must never edit a file there (it left math.js and notes.txt in crates/wisp).
static RUN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn edit(path: &Path, text: &str) {
    if RUN.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = std::fs::write(path, text);
    }
}

fn out(v: &Value) {
    println!("{v}");
    let _ = std::io::stdout().flush();
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn wait_ms(k: &str) {
    if let Some(ms) = env(k).and_then(|v| v.parse::<u64>().ok()) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// `FAKE_GATE`: a gated run waits for its gate file (the test's release), checked every 20 ms: a condition, not a
/// duration, so the turn runs exactly as long as the test needs however loaded the machine is.
fn wait_gate(prompt: &str) {
    let Some(dir) = env("FAKE_GATE") else { return };
    let gated = prompt.strip_prefix("gate ").or_else(|| prompt.split("\n\ngate ").nth(1));
    let all = env("FAKE_GATE_ALL");
    if let Some(name) = all.as_deref().or_else(|| gated.and_then(|r| r.split_whitespace().next())) {
        let file = Path::new(&dir).join(name);
        while !file.exists() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

fn hang() -> ! {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn final_text(name: &str, prompt: &str) -> String {
    let mut hero = json!({ "type": "hero", "title": format!("Fake {name} finished"), "summary": "No question asked.", "status": "done" });
    if let Some(t) = prompt.split_once("TOPIC[").and_then(|(_, r)| r.split_once(']')).map(|(t, _)| t) {
        hero["topic"] = Value::from(t);
    }
    let mut blocks = vec![hero];
    if let Ok(path) = std::env::var("FAKE_CREW_BLOCK_FILE")
        && let Ok(raw) = std::fs::read_to_string(&path)
        && let Ok(v) = serde_json::from_str::<Value>(&raw)
        && v.is_object()
    {
        blocks.push(v);
    }
    format!("All done.\n{}```wisp\n{}\n```", said(prompt), json!({ "blocks": blocks }))
}

/// `FAKE_SAY`: `needle=>words` pairs split by `;;`; a run whose prompt holds a needle ends its answer with its words (before
/// its wisp block), a line each (a factory's reviewer says APPROVE, ADR-0114).
fn said(prompt: &str) -> String {
    env("FAKE_SAY")
        .unwrap_or_default()
        .split(";;")
        .filter_map(|p| p.split_once("=>"))
        .filter(|(needle, _)| !needle.is_empty() && prompt.contains(needle))
        .fold(String::new(), |mut out, (_, words)| {
            out.push_str(words);
            out.push('\n');
            out
        })
}

/// `--version`, sign-in checks and sign-out: answered without a run.
fn meta(name: &str, argv: &[String]) -> Option<i32> {
    let joined = argv.join(" ");
    if matches!(argv.first().map(String::as_str), Some("--version" | "-v")) {
        let v = env("FAKE_CLI_VERSION").unwrap_or_else(|| "9.9.9".into());
        let line = match name {
            "claude" => format!("{v} (Fake Claude Code)"),
            "codex" => format!("fake-codex {v}"),
            _ => format!("fake-{name} 9.9.9"),
        };
        println!("{line}");
        return Some(0);
    }
    if name == "claude" && joined == "setup-token" {
        // Claude Code 2.1: the link as a terminal hyperlink (OSC 8), then, once the browser's localhost callback
        // finished it (FAKE_LOGIN_MS later), the long-lived token, wrapped as a terminal prints it; no code pasted
        let url = "https://claude.ai/oauth/authorize?code=true&client_id=fake&state=s";
        print!(
            "Opening browser to sign in…\r\n\r\nBrowser didn't open? Use the URL below\r\n\r\n\u{1b}]8;id=f1;{url}\u{7}{url}\u{1b}]8;;\u{7}\r\n\r\nPaste code here if prompted > "
        );
        let _ = std::io::stdout().flush();
        wait_ms("FAKE_LOGIN_MS");
        print!(
            "\r\n✓ Long-lived authentication token created successfully!\r\n\r\nYour OAuth token (valid for 1 year):\r\n\r\nsk-ant-oat01-fake-token-for-every-sandbox\r\n-0123456789\r\n\r\nStore this token securely.\r\n"
        );
        let _ = std::io::stdout().flush();
        return Some(0);
    }
    if name == "codex" && joined == "login --device-auth" {
        // v1 agent-login: a link and a one-time code, then signed in
        println!("Open https://auth.example.dev/codex/device\nand enter this one-time code ABCD-12345");
        let _ = std::io::stdout().flush();
        wait_ms("FAKE_LOGIN_MS");
        if let Some(home) = env("CODEX_HOME") {
            let _ = std::fs::create_dir_all(&home);
            let _ = std::fs::write(Path::new(&home).join("auth.json"), "{}");
        }
        return Some(0);
    }
    if name == "codex" && joined == "login status" {
        let signed = env("CODEX_HOME").is_some_and(|h| Path::new(&h).join("auth.json").is_file());
        println!("{}", if signed { "Logged in using ChatGPT" } else { "Not logged in" });
        return Some(i32::from(!signed));
    }
    if name == "codex" && joined == "app-server" {
        // the usage read Codex's own status line does (v1 live-limits.js)
        let mut input = String::new();
        let _ = std::io::stdin().read_to_string(&mut input);
        for line in input.lines() {
            let Ok(m) = serde_json::from_str::<Value>(line) else { continue };
            match m["id"].as_i64() {
                Some(1) => out(&json!({ "id": 1, "result": {} })),
                // FAKE_CODEX_USAGE: a file whose JSON is the answer's `rateLimits` (a test moves the windows)
                Some(2) => {
                    let read = env("FAKE_CODEX_USAGE")
                        .and_then(|f| std::fs::read_to_string(f).ok())
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
                    let limits = read.unwrap_or_else(
                        || json!({ "primary": { "usedPercent": 42, "windowDurationMins": 300, "resetsAt": 1_900_000_000 } }),
                    );
                    out(&json!({ "id": 2, "result": { "rateLimits": limits } }));
                }
                _ => {}
            }
        }
        return Some(0);
    }
    if name == "codex" && joined == "debug models" {
        // no catalog: detect falls back to $CODEX_HOME/models_cache.json
        return Some(1);
    }
    if name == "claude" && joined == "auth status --text" {
        if env("FAKE_AUTH_FAIL").is_some() {
            eprintln!("Error: could not read the sign-in");
            return Some(1);
        }
        println!("Login method: Claude Max Account");
        if let Some(p) = env("FAKE_AUTH_PROFILE") {
            println!("Profile: {p}");
        }
        return Some(0);
    }
    ["auth status", "auth logout", "login status", "logout", "status"].contains(&joined.as_str()).then_some(0)
}

/// Holds an exclusive lock on `path` (made if missing) until this process ends, however it ends (the OS drops it with
/// the process; the handle is never inherited). A test tells whether this process still runs by that lock, never by
/// its pid alone: Windows hands a freed pid to the next process at once (`serve_orphans`' hung probes on Windows CI: a
/// probe ended at its time limit, the next process started got its pid, and the probe looked alive 25 s on).
fn hold_live(path: &Path) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // no lock would read as ended at once (a false pass): fail loudly instead
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).expect("the live file");
    f.lock().expect("the live file's lock");
    // held for the process's life: closing it would let the lock go
    std::mem::forget(f);
}

/// `FAKE_GRANDCHILD`'s lock file for process `pid`.
fn live_of(file: &str, pid: u32) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{file}.{pid}.live"))
}

/// The MCP config the run was given (ADR-0116): read from the file `--mcp-config <path>` (Claude) or
/// `--additional-mcp-config @<path>` (Copilot) names, while it's there (the runtime removes it when the run ends), or the
/// JSON itself when it was given inline.
fn mcp_config(argv: &[String]) -> Value {
    let Some(i) = argv.iter().position(|a| a == "--mcp-config" || a == "--additional-mcp-config") else { return Value::Null };
    let Some(v) = argv.get(i + 1) else { return Value::Null };
    let text =
        if v.trim_start().starts_with('{') { v.clone() } else { std::fs::read_to_string(v.trim_start_matches('@')).unwrap_or_default() };
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

/// What Codex's MCP servers read from its environment (ADR-0116): the variables its argv names (`env_vars=[…]`,
/// `env_http_headers={…="VAR"}`), with their values.
fn mcp_env(argv: &[String]) -> Value {
    let mut out = serde_json::Map::new();
    for a in argv.iter().filter(|a| a.contains(".env_vars=") || a.contains(".env_http_headers=")) {
        for name in a.split('"').skip(1).step_by(2) {
            if let Some(v) = env(name) {
                out.insert(name.to_owned(), Value::String(v));
            }
        }
    }
    Value::Object(out)
}

fn record(name: &str, argv: &[String], prompt: &str) {
    let Some(dir) = env("FAKE_RECORD") else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let live = Path::new(&dir).join("live").join(format!("{name}.{}.{nanos}", std::process::id()));
    // taken before the record is written: a test that reads the record finds the lock held while this runs
    hold_live(&live);
    let line = json!({ "argv": argv, "stdin": prompt, "cwd": std::env::current_dir().ok(), "pid": std::process::id(), "live": live, "codexHome": env("CODEX_HOME"), "subagentModel": env("CLAUDE_CODE_SUBAGENT_MODEL"), "mcpConfig": mcp_config(argv), "mcpEnv": mcp_env(argv) });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(Path::new(&dir).join(format!("{name}.jsonl"))) {
        // one write per line: `writeln!` may split a long argv line, and a reader would see half of it
        let _ = f.write_all(format!("{line}\n").as_bytes());
    }
}

fn claude(prompt: &str, target: &Path, before: &str, after: &str) {
    let s = "claude-session-1";
    if let Some(f) = env("FAKE_REPLAY") {
        print!("{}", std::fs::read_to_string(f).unwrap_or_default());
        return;
    }
    let t = target.display().to_string();
    out(
        &json!({ "type": "system", "subtype": "init", "session_id": s, "claude_code_version": env("FAKE_CLI_VERSION").unwrap_or_else(|| "9.9.9".into()), "apiKeySource": "none" }),
    );
    if RUN.load(std::sync::atomic::Ordering::Relaxed) {
        stream(s);
    }
    out(
        &json!({ "type": "assistant", "session_id": s, "message": { "content": [{ "type": "tool_use", "id": "t1", "name": "Read", "input": { "file_path": t } }] } }),
    );
    if env("FAKE_HANG").is_some() {
        hang();
    }
    out(
        &json!({ "type": "user", "session_id": s, "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": [{ "type": "text", "text": before }] }] } }),
    );
    out(
        &json!({ "type": "assistant", "session_id": s, "message": { "content": [{ "type": "tool_use", "id": "t2", "name": "Edit", "input": { "file_path": t, "old_string": "a - b", "new_string": "a + b" } }] } }),
    );
    // as a real Edit: no change, no write
    if after != before {
        edit(target, after);
    }
    out(
        &json!({ "type": "result", "subtype": "success", "is_error": false, "result": final_text("claude", prompt), "session_id": s, "duration_ms": 5, "num_turns": 1, "total_cost_usd": 0.01, "usage": { "input_tokens": 10, "output_tokens": 5 } }),
    );
}

/// `FAKE_STREAM_MIX=agent`: one cycle of a real agent's steps, `(kind, bytes of the step wisp keeps)`. The bytes are the
/// 20 ventiles' midpoints (2.5 %, 7.5 %, … 97.5 %) of 275 374 steps that 3 872 real Claude Code turns kept once wisp's
/// own parser read them (`spikes/server-budgets`' `steps` bin, its README *The stream's shape*), the kinds in their
/// share (thinking 21 %, calls 35 %, results 34 %, text 9 %), each result after its call.
const MIX: [(&str, usize); 20] = [
    ("thinking", 19),
    ("tool", 86),
    ("tool_output", 180),
    ("tool", 222),
    ("tool_output", 563),
    ("text", 146),
    ("thinking", 19),
    ("tool", 487),
    ("tool_output", 1988),
    ("tool", 114),
    ("tool_output", 264),
    ("thinking", 19),
    ("tool", 1350),
    ("tool_output", 413),
    ("tool", 327),
    ("tool_output", 1863),
    ("thinking", 19),
    ("tool", 681),
    ("tool_output", 922),
    ("text", 2925),
];

/// What wisp's step adds around the text the CLI sent, by kind (`{"kind":"tool","label":…}`), so a line's text is
/// padded to the step's size: measured with the `steps` bin on these very lines.
fn mix_overhead(kind: &str) -> usize {
    match kind {
        "thinking" => 29,
        "tool" => 74,
        "tool_output" => 61,
        _ => 35,
    }
}

/// Line n of a `FAKE_STREAM_MIX=agent` stream, its text (stamp first) padded to `MIX[n % 20]`'s bytes.
fn mix_line(session: &str, n: u64, stamp: &str) -> Value {
    let i = usize::try_from(n % 20).unwrap_or(0);
    let (kind, bytes) = MIX[i];
    let mut text = stamp.to_owned();
    text.extend(std::iter::repeat_n('x', bytes.saturating_sub(mix_overhead(kind) + text.len())));
    let assistant = |part: Value| json!({ "type": "assistant", "session_id": session, "message": { "content": [part] } });
    match kind {
        "thinking" => assistant(json!({ "type": "thinking", "thinking": text, "signature": "fake" })),
        "tool" => assistant(
            json!({ "type": "tool_use", "id": format!("mix{n}"), "name": "Bash", "input": { "command": text, "description": "Run it" } }),
        ),
        // its call is the line before (every result in MIX follows its call)
        "tool_output" => {
            json!({ "type": "user", "session_id": session, "message": { "content": [{ "type": "tool_result", "tool_use_id": format!("mix{}", n.saturating_sub(1)), "content": text }] } })
        }
        _ => assistant(json!({ "type": "text", "text": text })),
    }
}

/// `FAKE_STREAM`: `rate` assistant text lines a second for `FAKE_STREAM_MS`, each stamped with when it was written.
/// The n-th line is due at start + n / rate: a late one goes at once and the next keeps the pace.
#[allow(clippy::cast_precision_loss)] // n stays far below 2^52
fn stream(session: &str) {
    let Some(rate) = env("FAKE_STREAM").and_then(|v| v.parse::<f64>().ok()).filter(|r| *r > 0.0) else { return };
    let ms = env("FAKE_STREAM_MS").and_then(|v| v.parse::<u64>().ok()).unwrap_or(5000);
    let bytes = env("FAKE_STREAM_BYTES").and_then(|v| v.parse::<usize>().ok()).unwrap_or(400);
    let mix = env("FAKE_STREAM_MIX").is_some_and(|m| m == "agent");
    let start = std::time::Instant::now();
    let until = std::time::Duration::from_millis(ms);
    let pid = std::process::id();
    for n in 0u64.. {
        let due = std::time::Duration::from_secs_f64(n as f64 / rate);
        if due >= until {
            break;
        }
        if let Some(wait) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
        let us = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros());
        let mut text = format!("~stamp:{pid}:{n}:{us}~ ");
        if mix {
            out(&mix_line(session, n, &text));
            continue;
        }
        let line = |text: &str| json!({ "type": "assistant", "session_id": session, "message": { "content": [{ "type": "text", "text": text }] } });
        // padded to the line length asked for (the JSON around the text is counted)
        let pad = bytes.saturating_sub(line(&text).to_string().len());
        text.extend(std::iter::repeat_n('x', pad));
        out(&line(&text));
    }
}

fn codex(prompt: &str, target: &Path, after: &str) {
    out(&json!({ "type": "thread.started", "thread_id": "codex-session-1" }));
    out(&json!({ "type": "turn.started" }));
    out(
        &json!({ "type": "item.started", "item": { "id": "i0", "type": "command_execution", "command": "/bin/bash -lc 'npm test'", "status": "in_progress" } }),
    );
    if env("FAKE_HANG").is_some() {
        hang();
    }
    out(
        &json!({ "type": "item.completed", "item": { "id": "i0", "type": "command_execution", "command": "npm test", "exit_code": 1, "aggregated_output": "1 failing", "status": "failed" } }),
    );
    edit(target, after);
    let cwd = target.parent().unwrap_or(Path::new("."));
    edit(&cwd.join("notes.txt"), "new file\n");
    out(
        &json!({ "type": "item.completed", "item": { "id": "i1", "type": "file_change", "changes": [{ "path": target, "kind": "update" }, { "path": "notes.txt", "kind": "add" }], "status": "completed" } }),
    );
    out(&json!({ "type": "item.completed", "item": { "id": "i2", "type": "agent_message", "text": final_text("codex", prompt) } }));
    out(&json!({ "type": "turn.completed", "usage": { "input_tokens": 10, "cached_input_tokens": 0, "output_tokens": 5 } }));
}

/// Each other CLI's recorded turn (the G6 fixtures), with this run's folder and answer.
fn recorded(name: &str) -> Option<&'static str> {
    Some(match name {
        "opencode" => include_str!("../../../plugins/opencode/core/tests/fixtures/turn.jsonl"),
        "cursor-agent" => include_str!("../../../plugins/cursor/core/tests/fixtures/turn.jsonl"),
        "copilot" => include_str!("../../../plugins/copilot/core/tests/fixtures/turn.jsonl"),
        "grok" => include_str!("../../../plugins/grok/core/tests/fixtures/turn.jsonl"),
        "agy" => include_str!("../../../plugins/antigravity/core/tests/fixtures/turn.jsonl"),
        _ => return None,
    })
}

/// Strings in a recorded line: the folder is this run's, the answer names this prompt's topic.
fn fill(v: &mut Value, cwd: &str, answer: &str) {
    match v {
        Value::String(s) if s.starts_with("All done.\n```wisp") => answer.clone_into(s),
        Value::String(s) => *s = s.replace("@CWD@", cwd),
        Value::Array(a) => a.iter_mut().for_each(|x| fill(x, cwd, answer)),
        Value::Object(o) => o.values_mut().for_each(|x| fill(x, cwd, answer)),
        _ => {}
    }
}

fn replay_turn(name: &str, stream: &str, prompt: &str, target: &Path, after: &str) {
    let cwd = target.parent().unwrap_or(Path::new("."));
    edit(target, after);
    edit(&cwd.join("notes.txt"), "new file\n");
    let short = name.split('-').next().unwrap_or(name);
    let answer = final_text(short, prompt);
    for (i, line) in stream.lines().enumerate() {
        if i == 2 && env("FAKE_HANG").is_some() {
            hang();
        }
        let Ok(mut v) = serde_json::from_str::<Value>(line) else { continue };
        fill(&mut v, &cwd.display().to_string(), &answer);
        out(&v);
    }
}

/// `FAKE_GRANDCHILD`: a process this run starts and never waits for (an agent's tool, a dev server), in the run's own
/// process group, holding this same executable: wisp must end it with the run, and with the server however it stops.
fn grandchild(file: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    hold_live(&live_of(file, std::process::id()));
    let mut child = std::process::Command::new(exe)
        .env("FAKE_SLEEPER", file)
        .env_remove("FAKE_GRANDCHILD")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the grandchild starts");
    // its one byte says it holds its lock: only then is the line written (a test that reads it finds both held), and
    // before this run goes on (an exec plugin that crashes right after must leave the line behind)
    // no byte (it died first): no line, so the test fails with "never started its grandchild" instead of passing
    let said = child.stdout.take().is_some_and(|mut out| matches!(out.read(&mut [0u8; 1]), Ok(1)));
    let pid = child.id();
    // never waited for: it outlives this run until wisp ends it
    std::mem::forget(child);
    if !said {
        return;
    }
    let line = format!("{} {pid}\n", std::process::id());
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file).expect("the pid file");
    let _ = f.write_all(line.as_bytes());
}

/// The grandchild: takes its lock, says so on stdout (one byte, then the pipe closes), and sleeps until it is killed.
fn sleeper(file: &str) -> ! {
    hold_live(&live_of(file, std::process::id()));
    let _ = std::io::stdout().write_all(b"\n");
    let _ = std::io::stdout().flush();
    hang();
}

/// `exec-plugin`: answers each request with `null` (so `initialize` finishes), starts the grandchild once set up, and
/// once its input closes (wisp gone) keeps running: only wisp's containment can end it.
fn exec_plugin() -> ! {
    use std::io::BufRead as _;
    let mut started = false;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(m) = serde_json::from_str::<Value>(&line) else { continue };
        if m.get("method").is_none() {
            continue;
        }
        if let Some(id) = m.get("id") {
            out(&json!({ "jsonrpc": "2.0", "id": id, "result": null }));
        }
        if !started && m["method"] == "initialize" {
            started = true;
            if let Some(f) = env("FAKE_GRANDCHILD") {
                grandchild(&f);
            }
            if env("FAKE_EXEC_CRASH").is_some() {
                std::process::exit(1);
            }
        }
    }
    hang();
}

fn main() {
    if let Some(file) = env("FAKE_SLEEPER") {
        sleeper(&file);
    }
    let mut args = std::env::args();
    let me = args.next().unwrap_or_default();
    let name = Path::new(&me).file_stem().and_then(|s| s.to_str()).unwrap_or("agent").to_owned();
    if name == "exec-plugin" {
        exec_plugin();
    }
    let argv: Vec<String> = args.collect();
    if let Some(code) = meta(&name, &argv) {
        std::process::exit(code);
    }
    let mut prompt = String::new();
    let _ = std::io::stdin().read_to_string(&mut prompt);
    // agy takes it as a stream-json `user` event, grok as a file
    if name == "agy" {
        prompt = serde_json::from_str::<Value>(prompt.lines().next().unwrap_or(""))
            .ok()
            .and_then(|v| v["message"]["content"].as_str().map(str::to_owned))
            .unwrap_or_default();
    }
    if let Some(i) = argv.iter().position(|a| a == "--prompt-file") {
        prompt = std::fs::read_to_string(argv.get(i + 1).map_or("", String::as_str)).unwrap_or_default();
    }
    record(&name, &argv, &prompt);
    let run = !prompt.trim().is_empty();
    if run && let Some(f) = env("FAKE_GRANDCHILD") {
        grandchild(&f);
    }
    RUN.store(run, std::sync::atomic::Ordering::Relaxed);
    // only a run (it has a prompt) hits it: a harness's detect probe (`opencode models`, `auth list`) that
    // lands late must not use the limit up (serve_kinds' best-of-n flaked on that under CI load)
    if let Some(f) = env("FAKE_LIMIT_ONCE")
        && !prompt.trim().is_empty()
        && !Path::new(&f).exists()
    {
        let _ = std::fs::write(&f, "1");
        eprintln!("{}", env("FAKE_LIMIT_TEXT").unwrap_or_else(|| "Claude AI usage limit reached|1767225600".into()));
        std::process::exit(1);
    }
    if env("FAKE_FAIL").is_some() {
        eprintln!("Error: the fake {name} failed on purpose");
        std::process::exit(1);
    }
    // a run's delay only: a detect probe (`opencode models`, `cursor-agent --list-models`, `copilot help config`)
    // answers at once, or every `runtime.agents` paid FAKE_DELAY per probe (serve_mcp_tools' sweep took 274 s)
    if run {
        wait_ms("FAKE_DELAY");
        wait_gate(&prompt);
    }
    let target = std::env::current_dir().unwrap_or_default().join("math.js");
    let before = std::fs::read_to_string(&target).unwrap_or_default();
    let after = before.replace("a - b", "a + b");
    match name.as_str() {
        "claude" => claude(&prompt, &target, &before, &after),
        "codex" => codex(&prompt, &target, &after),
        n if recorded(n).is_some() => replay_turn(n, recorded(n).unwrap_or_default(), &prompt, &target, &after),
        _ => {
            edit(&target, &after);
            println!("{}", final_text(&name, &prompt));
        }
    }
    wait_ms("FAKE_LINGER");
}
