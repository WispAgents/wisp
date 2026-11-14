//! Gate G6 for opencode: every recorded `opencode run --format json` run parses as v1's adapter reported it, and
//! builds the argv v1 built for the same options (the fixtures' `meta.argv`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use serde_json::Value;
use wisp_harness_kit::{NoDisk, RunPaths, golden};
use wisp_opencode_core::{OpencodeArgs, OpencodeOptions, OpencodeParser, args};

const CWD: &str = "/tmp/wisp-g6-run";

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn names() -> Vec<String> {
    let mut n: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".jsonl")).map(str::to_owned))
        .collect();
    n.sort();
    n
}

fn read(name: &str, ext: &str) -> String {
    std::fs::read_to_string(dir().join(format!("{name}.{ext}"))).unwrap_or_default()
}

fn meta(name: &str) -> Value {
    serde_json::from_str(&read(name, "meta.json")).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

#[test]
fn g6_every_recorded_opencode_run_parses_as_v1_did() {
    let mut failed = Vec::new();
    let all = names();
    assert_eq!(all.len(), 15, "the 15 recorded runs");
    for name in &all {
        let m = meta(name);
        let o = &m["opts"];
        let mut p = OpencodeParser::new(OpencodeOptions {
            paths: RunPaths::same(CWD),
            model: s(&o["model"]).to_owned(),
            ..OpencodeOptions::default()
        });
        if let Err(e) = golden::check(&mut p, CWD, &m, &read(name, "jsonl"), &read(name, "v1.json"), &mut NoDisk) {
            failed.push(format!("{name}: {e}"));
        }
    }
    eprintln!("G6 opencode: {}/{} match v1", all.len() - failed.len(), all.len());
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn every_fixture_builds_v1s_argv() {
    for name in names() {
        let m = meta(&name);
        let o = &m["opts"];
        let files: Vec<String> =
            o["attachments"].as_array().map(|a| a.iter().map(|x| s(&x["path"]).to_owned()).collect()).unwrap_or_default();
        let a = OpencodeArgs {
            mode: if o["mode"].is_string() { s(&o["mode"]) } else { "edits" },
            model: s(&o["model"]),
            effort: s(&o["effort"]),
            resume_id: o["resumeId"].as_str(),
            fork: o["fork"].as_bool().unwrap_or(false),
            attachments: &files,
            v2: false,
        };
        let want: Vec<String> = m["argv"].as_array().unwrap().iter().map(|x| s(x).to_owned()).collect();
        assert_eq!(args(&a), want, "{name}");
    }
}

/// Windows CI (round 3): a run in a `C:\…` folder names the same project-relative files as one in `/tmp/…`, with
/// `/` (it named `C:\…\proj/math.js` and lost `notes.txt`, so a turn counted 1 changed file, or none).
#[test]
fn a_windows_folder_names_the_same_files_as_a_posix_one() {
    const WIN: &str = r"C:\Users\RUNNER~1\AppData\Local\Temp\wisp-g6-run";
    let run = |cwd: &str, stdout: &str| -> Vec<String> {
        let mut p = OpencodeParser::new(OpencodeOptions { paths: RunPaths::same(cwd), ..OpencodeOptions::default() });
        let at = if cwd == WIN { "C:/Users/RUNNER~1/AppData/Local/Temp/wisp-g6-run" } else { CWD };
        golden::files(&mut p, cwd, stdout).into_iter().map(|f| f.replace(at, "@CWD@")).collect()
    };
    let mut math = 0;
    for name in names() {
        let stdout = read(&name, "jsonl");
        let (posix, win) = (run(CWD, &stdout), run(WIN, &stdout));
        assert_eq!(win, posix, "{name}");
        math += usize::from(posix.iter().any(|f| f == "math.js"));
    }
    assert!(math > 0, "some run names math.js");
}
