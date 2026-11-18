//! `wisp hook offload` and `wisp hook offload-show <file> <min>` (ADR-0079): Claude Code's `PreToolUse` hook on Bash
//! that sends long output to a file under the task's folder, and the step that prints what the agent sees of it.
//! Both run before the async runtime starts and never fail loudly: a hook that errors would hold the agent's command.
use std::io::{Read as _, Write as _};

use wisp_runtime_api::points::{OFFLOAD_DIR_ENV, OFFLOAD_MIN_ENV};
use wisp_threads_core::offload;

/// Runs `wisp hook offload…` when `argv` is one; false for anything else.
pub fn fast_path(argv: &[String]) -> bool {
    match (argv.first().map(String::as_str), argv.get(1).map(String::as_str)) {
        (Some("hook"), Some("offload")) => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            let dir = std::env::var(OFFLOAD_DIR_ENV).ok();
            let wisp = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            let min = offload::min_of(std::env::var(OFFLOAD_MIN_ENV).ok().as_deref());
            let flavor = if argv.iter().any(|a| a == "--codex") { offload::Flavor::Codex } else { offload::Flavor::Claude };
            if let Some(out) = answer(flavor, &input, dir.as_deref(), &wisp, min) {
                let _ = writeln!(std::io::stdout(), "{out}");
            }
            true
        }
        (Some("hook"), Some("compacted")) => {
            // Codex's PostCompact hook: its input is read and dropped (it waits on our stdin closing)
            let _ = std::io::stdin().read_to_string(&mut String::new());
            if let Some(dir) = std::env::var(OFFLOAD_DIR_ENV).ok().filter(|d| !d.is_empty()) {
                compacted(std::path::Path::new(&dir));
            }
            true
        }
        (Some("hook"), Some("offload-show")) => {
            let path = argv.get(2).map_or("", String::as_str);
            let min = offload::min_of(argv.get(3).map(String::as_str));
            let _ = std::io::stdout().write_all(&shown(path, min));
            true
        }
        _ => false,
    }
}

/// The hook's answer to a `PreToolUse` input, or None to let the command run as it is.
fn answer(flavor: offload::Flavor, input: &str, dir: Option<&str>, wisp: &str, min: usize) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    offload::hook_answer_as(flavor, &v, dir, wisp, min).map(|a| a.to_string())
}

/// One more compaction this turn: a line in the folder's `compactions` (wisp.threads counts and removes it).
fn compacted(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(offload::COMPACTIONS_FILE)) {
        let _ = writeln!(f, "compacted");
    }
}

/// What the agent sees of the output in `path`: all of it when short (and the file goes), else its start, its
/// error-looking lines, its end and where the rest is.
fn shown(path: &str, min: usize) -> Vec<u8> {
    let Ok(out) = std::fs::read(path) else { return Vec::new() };
    if let Some(s) = offload::show(&out, path, min) {
        s.into_bytes()
    } else {
        let _ = std::fs::remove_file(path);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hook_rewrites_a_foreground_bash_command_and_leaves_the_rest() {
        let bash = r#"{"tool_name":"Bash","tool_input":{"command":"cargo test"}}"#;
        let out = answer(offload::Flavor::Claude, bash, Some("/tmp/out/t1"), "/bin/wisp", 8000).unwrap();
        assert!(out.contains("updatedInput") && out.contains("/tmp/out/t1"));
        assert_eq!(answer(offload::Flavor::Claude, bash, None, "/bin/wisp", 8000), None, "no folder: no rewrite");
        assert_eq!(answer(offload::Flavor::Claude, "not json", Some("/tmp/x"), "/bin/wisp", 8000), None);
        let bg = r#"{"tool_name":"Bash","tool_input":{"command":"npm run dev","run_in_background":true}}"#;
        assert_eq!(answer(offload::Flavor::Claude, bg, Some("/tmp/x"), "/bin/wisp", 8000), None);
        // Codex's answer is an allow with the rewrite; Claude's leaves the decision to Claude Code
        let codex: serde_json::Value =
            serde_json::from_str(&answer(offload::Flavor::Codex, bash, Some("/tmp/out/t1"), "/bin/wisp", 8000).unwrap()).unwrap();
        assert_eq!(codex["hookSpecificOutput"]["permissionDecision"], "allow");
        assert!(codex["hookSpecificOutput"]["updatedInput"]["command"].as_str().unwrap().contains("/tmp/out/t1"));
        assert!(!out.contains("permissionDecision"));
    }

    #[test]
    fn each_compaction_adds_a_line_wisp_threads_counts() {
        let dir = std::env::temp_dir().join(format!("wisp-compacted-{}", std::process::id())).join("t1");
        compacted(&dir);
        compacted(&dir);
        assert_eq!(std::fs::read_to_string(dir.join(offload::COMPACTIONS_FILE)).unwrap().lines().count(), 2);
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn short_output_is_shown_whole_and_its_file_removed_long_output_stays() {
        let dir = std::env::temp_dir().join(format!("wisp-offload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let short = dir.join("short.txt");
        std::fs::write(&short, "ok\n").unwrap();
        assert_eq!(shown(&short.display().to_string(), 2000), b"ok\n");
        assert!(!short.exists());
        let long = dir.join("long.txt");
        std::fs::write(&long, "line\n".repeat(2000)).unwrap();
        let s = String::from_utf8(shown(&long.display().to_string(), 2000)).unwrap();
        assert!(s.contains(&long.display().to_string()) && s.len() < 10_000);
        assert!(long.exists(), "the agent reads the rest from the file");
        assert_eq!(shown("/no/such/file", 2000), Vec::<u8>::new());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
