//! Long command output into a file (v1 server/offload-hook.mjs): long output is most of what fills a long thread, and
//! most of it is read once. The command runs as written, its output goes to a file in the task's folder, and the
//! agent sees the start, the lines that look like errors, the end and the file's path (it can grep or `sed -n` the
//! rest). Short output passes as it was. Claude Code shows only the first 2 KB of a long output, which is where a
//! test run's failures never are.
//!
//! The rewrite: the command in a `{ … }` group, so a `cd` in it still carries to the next command; an EXIT trap shows
//! the output through `wisp hook offload-show`, so an `exit` in it still shows it, with the command's own status. Not
//! for background commands (`run_in_background`, a trailing `&`, `nohup`, `disown`): they print as they go.

use std::fmt::Write as _;

use serde_json::{Value, json};

pub const MARK: &str = "# wisp: long output goes to a file";
/// From this many bytes output is cut (the run's `WISP_OFFLOAD_MIN`, never under `MIN_FLOOR`).
pub const MIN_DEFAULT: usize = 8000;
pub const MIN_FLOOR: usize = 2000;
const HEAD: usize = 1500;
const TAIL: usize = 3500;
const ERRORS_MAX: usize = 25;
const LINE_MAX: usize = 240;

/// The size from which output is cut, from the run's env.
pub fn min_of(env: Option<&str>) -> usize {
    env.and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(MIN_DEFAULT).max(MIN_FLOOR)
}

fn sh(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Whether a command runs in the background (it prints as it goes: never wrapped).
pub fn background(command: &str) -> bool {
    let code: String = command.lines().map(|l| l.split(" #").next().unwrap_or(l)).collect::<Vec<_>>().join("\n");
    let t = code.trim_end();
    (t.ends_with('&') && !t.ends_with("&&")) || t.split_whitespace().any(|w| w == "nohup" || w == "disown")
}

/// The command as it runs, or None: it runs untouched (empty, already wrapped, background, no folder).
pub fn wrap(command: &str, dir: &str, wisp: &str, min: usize) -> Option<String> {
    if command.trim().is_empty() || dir.trim().is_empty() || wisp.trim().is_empty() || command.contains(MARK) || background(command) {
        return None;
    }
    let d = dir.replace('\\', "/");
    Some(
        [
            MARK.to_owned(),
            format!("__wisp_d={}; __wisp_w={}; mkdir -p \"$__wisp_d\" 2>/dev/null; __wisp_f=\"$__wisp_d/out-$(date +%s)-$$-${{RANDOM:-0}}.txt\"", sh(&d), sh(wisp)),
            // fds 8 and 9 keep where output went: an exit inside the group runs the trap still redirected
            "exec 8>&1 9>&2".to_owned(),
            format!("trap '__wisp_s=$?; {{ \"$__wisp_w\" hook offload-show \"$__wisp_f\" {min} || cat \"$__wisp_f\"; }} >&8 2>&9; exit ${{__wisp_e:-$__wisp_s}}' EXIT"),
            "{".to_owned(),
            command.to_owned(),
            "} > \"$__wisp_f\" 2>&1 < /dev/null; __wisp_e=$?".to_owned(),
        ]
        .join("\n"),
    )
}

/// The file a harness's compaction hook adds a line to, in the run's offload folder (`wisp hook compacted`): wisp.threads
/// counts and removes it at the turn's end, for a harness whose stream doesn't say it compacted (Codex, ADR-0097).
pub const COMPACTIONS_FILE: &str = "compactions";

/// Which CLI asks: Claude Code takes `updatedInput` alone (a `permissionDecision` would skip its own ask); Codex needs
/// `permissionDecision: "allow"` beside it (its docs: the rewrite is an allow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flavor {
    #[default]
    Claude,
    Codex,
}

/// Claude Code's `PreToolUse` hook: its input (stdin) → the `updatedInput` to answer with, or None (run as it is).
pub fn hook_answer(input: &Value, dir: Option<&str>, wisp: &str, min: usize) -> Option<Value> {
    hook_answer_as(Flavor::Claude, input, dir, wisp, min)
}

/// [`hook_answer`] in `flavor`'s shape (both send `tool_name: "Bash"` and `tool_input.command`).
pub fn hook_answer_as(flavor: Flavor, input: &Value, dir: Option<&str>, wisp: &str, min: usize) -> Option<Value> {
    if input["tool_name"] != "Bash" || input["tool_input"]["run_in_background"] == true {
        return None;
    }
    let ti = input.get("tool_input")?.as_object()?;
    let command = wrap(ti.get("command")?.as_str()?, dir?, wisp, min)?;
    let mut updated = ti.clone();
    updated.insert("command".into(), Value::String(command));
    Some(match flavor {
        Flavor::Claude => json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "updatedInput": updated } }),
        Flavor::Codex => {
            json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "allow", "updatedInput": updated } })
        }
    })
}

/// The longest start of `b` of at most `n` bytes that ends between characters.
fn head(b: &str, n: usize) -> &str {
    let mut end = n.min(b.len());
    while !b.is_char_boundary(end) {
        end -= 1;
    }
    &b[..end]
}

fn tail(b: &str, n: usize) -> &str {
    let mut start = b.len().saturating_sub(n);
    while !b.is_char_boundary(start) {
        start += 1;
    }
    &b[start..]
}

const ERROR_WORDS: [&str; 7] = ["error", "fail", "exception", "panic", "traceback", "fatal", "assert"];

/// A line that looks like an error, but not a count of none ("0 failed", "errors: 0", "0 errors").
fn error_line(line: &str) -> bool {
    let l = line.to_lowercase();
    if !ERROR_WORDS.iter().any(|w| l.contains(w)) {
        return false;
    }
    let words: Vec<&str> = l.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let zero_near = |i: usize| (i > 0 && words[i - 1] == "0") || words.get(i + 1) == Some(&"0");
    let hits: Vec<usize> = words.iter().enumerate().filter(|(_, w)| ERROR_WORDS.iter().any(|e| w.starts_with(e))).map(|(i, _)| i).collect();
    !hits.iter().all(|i| zero_near(*i))
}

/// What the agent sees of output that went to `path`: None when it's short (shown whole, the file removed).
pub fn show(out: &[u8], path: &str, min: usize) -> Option<String> {
    if out.len() <= min {
        return None;
    }
    let text = String::from_utf8_lossy(out);
    let lines = text.lines().count();
    let mut s = String::from(head(&text, HEAD));
    let _ = write!(
        s,
        "\n\n[wisp: {} bytes, {lines} lines. The middle is cut here; the whole output is in {path} (grep or sed -n it rather than cat it)]\n",
        out.len()
    );
    let errs: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| error_line(l))
        .take(ERRORS_MAX)
        .map(|(i, l)| format!("{}:{}", i + 1, l.chars().take(LINE_MAX).collect::<String>()))
        .collect();
    if !errs.is_empty() {
        let _ = write!(s, "\n[lines that look like errors, with their line numbers]\n{}\n", errs.join("\n"));
    }
    s.push_str("\n[the end]\n");
    s.push_str(tail(&text, TAIL));
    Some(s)
}
