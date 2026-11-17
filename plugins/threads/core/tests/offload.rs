//! The Bash output offload (v1 test/threads.test.js "offload"), and a real bash run of the rewrite where bash is.

use std::fmt::Write as _;

use serde_json::json;
use wisp_threads_core::offload::{MARK, background, hook_answer, min_of, show, wrap};

#[test]
fn background_commands_empty_ones_and_wrapped_ones_run_as_they_are() {
    for c in ["npm run dev &", "nohup node srv.js", "node srv.js & disown", ""] {
        assert_eq!(wrap(c, "/tmp/x", "/bin/wisp", 8000), None, "{c:?}");
    }
    assert!(!background("make && make test"), "&& is no background");
    assert_eq!(wrap("ls", "", "/bin/wisp", 8000), None, "no folder: untouched");
    let w = wrap("ls", "/tmp/x", "/bin/wisp", 8000).unwrap();
    assert!(w.starts_with(MARK));
    assert_eq!(wrap(&w, "/tmp/x", "/bin/wisp", 8000), None, "never twice");
    assert!(w.contains("__wisp_w='/bin/wisp'") && w.contains("hook offload-show \"$__wisp_f\" 8000 || cat"));
    assert!(wrap("echo 'it''s'", "/tmp/o'k", "/bin/wisp", 8000).unwrap().contains(r"'/tmp/o'\''k'"), "quotes in the folder");
    assert_eq!((min_of(None), min_of(Some("100")), min_of(Some("20000"))), (8000, 2000, 20000));
}

#[test]
fn the_hook_answers_only_for_bash_in_the_foreground() {
    let input = json!({ "tool_name": "Bash", "tool_input": { "command": "cargo test", "description": "tests" } });
    let out = hook_answer(&input, Some("/t"), "/bin/wisp", 8000).unwrap();
    let u = &out["hookSpecificOutput"]["updatedInput"];
    assert_eq!((out["hookSpecificOutput"]["hookEventName"].as_str(), u["description"].as_str()), (Some("PreToolUse"), Some("tests")));
    assert!(u["command"].as_str().unwrap().starts_with(MARK));
    let bg = json!({ "tool_name": "Bash", "tool_input": { "command": "npm run dev", "run_in_background": true } });
    assert_eq!(hook_answer(&bg, Some("/t"), "/bin/wisp", 8000), None);
    assert_eq!(hook_answer(&json!({ "tool_name": "Read", "tool_input": {} }), Some("/t"), "/bin/wisp", 8000), None);
    assert_eq!(hook_answer(&input, None, "/bin/wisp", 8000), None, "no folder for this run: off");
}

#[test]
fn long_output_shows_its_start_its_errors_and_its_end() {
    assert_eq!(show(b"short", "/t/out.txt", 8000), None, "short: shown whole");
    let mut out = String::new();
    for i in 0..2000 {
        writeln!(out, "line {i} ok é").unwrap();
        if i == 900 {
            out.push_str("thread 'login' panicked at src/a.rs:12\n");
        }
        if i == 950 {
            out.push_str("test result: ok. 10 passed; 0 failed\n");
        }
    }
    out.push_str("error: 1 test failed\n");
    let s = show(out.as_bytes(), "/t/out.txt", 8000).unwrap();
    assert!(s.starts_with("line 0 ok"));
    assert!(s.contains("whole output is in /t/out.txt"));
    assert!(s.contains("902:thread 'login' panicked"), "{}", &s[..s.len().min(3000)]);
    assert!(
        !s.contains("0 failed\n[") && !s.lines().any(|l| l.ends_with("10 passed; 0 failed") && l.contains(':') && l.starts_with("95")),
        "a count of none isn't an error"
    );
    assert!(s.ends_with("error: 1 test failed\n"));
    assert!(s.len() < 8000);
}

/// The rewrite really runs: output to a file, a `cd` still carries, the command's own status, `exit` inside it.
#[test]
fn the_rewrite_runs_in_bash_with_the_commands_own_status() {
    let Ok(bash) = which("bash") else { return };
    let dir = std::env::temp_dir().join(format!("wisp-offload-test-{}", std::process::id()));
    // a stand-in for `wisp hook offload-show`: prints the file whole
    let shim = dir.with_extension("sh");
    std::fs::write(&shim, "#!/bin/sh\ncat \"$3\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = |cmd: &str| {
        let w = wrap(cmd, &dir.display().to_string(), &shim.display().to_string(), 8000).unwrap();
        std::process::Command::new(&bash).arg("-c").arg(w).output().unwrap()
    };
    let r = run("echo hello; echo oops >&2");
    assert_eq!(String::from_utf8_lossy(&r.stdout), "hello\noops\n");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(run("echo x; exit 3").status.code(), Some(3), "an exit inside keeps its status");
    assert_eq!(run("false").status.code(), Some(1));
    assert!(std::fs::read_dir(&dir).unwrap().count() >= 1, "the output went to a file in the task's folder");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&shim);
}

fn which(bin: &str) -> Result<std::path::PathBuf, ()> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(bin)).find(|f| f.is_file())).ok_or(())
}
