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
            if let Some(out) = answer(&input, dir.as_deref(), &wisp, min) {
                let _ = writeln!(std::io::stdout(), "{out}");
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

/// The hook's answer to Claude Code's `PreToolUse` input, or None to let the command run as it is.
fn answer(input: &str, dir: Option<&str>, wisp: &str, min: usize) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    offload::hook_answer(&v, dir, wisp, min).map(|a| a.to_string())
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
        let out = answer(bash, Some("/tmp/out/t1"), "/bin/wisp", 8000).unwrap();
        assert!(out.contains("updatedInput") && out.contains("/tmp/out/t1"));
        assert_eq!(answer(bash, None, "/bin/wisp", 8000), None, "no folder: no rewrite");
        assert_eq!(answer("not json", Some("/tmp/x"), "/bin/wisp", 8000), None);
        let bg = r#"{"tool_name":"Bash","tool_input":{"command":"npm run dev","run_in_background":true}}"#;
        assert_eq!(answer(bg, Some("/tmp/x"), "/bin/wisp", 8000), None);
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
