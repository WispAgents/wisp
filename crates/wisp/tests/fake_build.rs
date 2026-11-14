//! The fake harness build in `common` is safe when several test processes start it at once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::process::{Command, Stdio};

const CHILD: &str = "WISP_FAKE_BUILD_OUT";

/// One of the racing processes: builds into the shared `out`, then runs the agent fake.
#[test]
fn fake_build_child() {
    let Some(out) = std::env::var_os(CHILD) else { return };
    let (agent, gh) = common::fake_bins_into(std::path::Path::new(&out));
    assert!(gh.is_file(), "{}", gh.display());
    let o = Command::new(&agent).arg("--version").stdin(Stdio::null()).output().unwrap();
    assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("9.9.9"), "{} --version: {o:?}", agent.display());
}

#[test]
fn several_processes_build_the_fakes_at_once() {
    let out = std::env::temp_dir().join(format!("wisp-fake-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let me = std::env::current_exe().unwrap();
    let kids: Vec<_> = (0..6)
        .map(|_| {
            Command::new(&me)
                .args(["--exact", "fake_build_child", "--nocapture", "--test-threads=1"])
                .env(CHILD, &out)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for k in kids {
        let o = k.wait_with_output().unwrap();
        assert!(
            o.status.success(),
            "a racing build failed:\n{}\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(String::from_utf8_lossy(&o.stdout).contains("1 passed"), "the child ran no test: {}", String::from_utf8_lossy(&o.stdout));
    }
    // no temp file left behind, the two fakes and the lock in place
    let mut names: Vec<_> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    let x = std::env::consts::EXE_SUFFIX;
    assert_eq!(names, [".lock".to_owned(), format!("wisp-fake-agent{x}"), format!("wisp-fake-gh{x}")]);
    std::fs::remove_dir_all(&out).unwrap();
}
