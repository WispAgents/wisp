//! DESK2-05: closing the desktop app ends everything wisp started (the shell's `KILL_ON_JOB_CLOSE` job), except the
//! one start that must outlive it: the update's relaunch helper. A start leaves that job only when it asks
//! (`SpawnOpts::leave_every_job`, `wisp_proc::outliving_flags`), so this pins who asks: the hand-off, and nobody else.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]

use std::path::{Path, PathBuf};

const ASK: &str = "leave_every_job: true";
const ONLY: &str = "plugins/update/host/src/hand_off.rs";

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if p.is_dir() {
            if !matches!(name.as_ref(), "target" | "node_modules" | "gen" | ".git") {
                walk(&p, out);
            }
        } else if name.ends_with(".rs") {
            out.push(p);
        }
    }
}

#[test]
fn only_the_updates_relaunch_helper_asks_to_leave_every_job() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for top in ["crates", "plugins", "apps", "desktop"] {
        walk(&root.join(top), &mut files);
    }
    assert!(files.len() > 100, "the workspace wasn't found from {}", root.display());
    let mut asking: Vec<String> = files
        .iter()
        .filter(|f| std::fs::read_to_string(f).is_ok_and(|t| t.contains(ASK)))
        .map(|f| f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
        // wisp-proc's own tests start helpers both ways
        .filter(|f| !f.starts_with("crates/wisp-proc/tests/"))
        .collect();
    asking.sort();
    assert_eq!(
        asking,
        [ONLY],
        "a start other than the update's relaunch helper leaves the desktop app's job: closing the window wouldn't stop it"
    );
}
