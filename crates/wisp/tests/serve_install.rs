//! `accounts.install` on a computer without Node (owner, 2026-11-25: 68 of 72 alpha installs started with no harness
//! and none of them ran a task): with no npm, wisp downloads opencode's own platform package from the npm registry
//! (here a fake one), checks its sha512, unpacks the binary into its tools folder and finds it there. Every outcome
//! reaches `PostHog`'s `harness_install` with how and, on a failure, why (v1 only counted npm's successes).
//! Unix only: the fake binary is a shell script (Windows' would be an `.exe`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::doc_markdown, clippy::type_complexity)]
#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use common::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha512};

/// A tiny HTTP server: every request's path and JSON body is kept; `answer(path)` gives the status and body.
fn fake_http(answer: impl Fn(&str) -> (u16, Vec<u8>) + Send + 'static) -> (u16, Arc<Mutex<Vec<(String, Value)>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap_or(0);
            let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
            let mut len = 0;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap_or(());
            g.lock().unwrap().push((path.clone(), serde_json::from_slice(&body).unwrap_or(Value::Null)));
            let (status, out) = answer(&path);
            let head = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", out.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&out);
        }
    });
    (port, got)
}

/// opencode's platform tarball as npm packs it (`package/package.json`, `package/bin/opencode`), with a script
/// standing in for the binary: its version, one free model, no providers signed in.
fn opencode_tgz(dir: &Path) -> Vec<u8> {
    let pkg = dir.join("tgz/package");
    std::fs::create_dir_all(pkg.join("bin")).unwrap();
    std::fs::write(pkg.join("package.json"), r#"{"name":"opencode-fake","version":"1.19.6"}"#).unwrap();
    let sh = "#!/bin/sh\ncase \"$1\" in\n  --version) echo 1.19.6 ;;\n  models) echo opencode/big-pickle ;;\nesac\nexit 0\n";
    script(&pkg.join("bin/opencode"), sh, "");
    let out = std::process::Command::new("tar").args(["czf", "../opencode.tgz", "package"]).current_dir(dir.join("tgz")).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    std::fs::read(dir.join("opencode.tgz")).unwrap()
}

fn sri(b: &[u8]) -> String {
    format!("sha512-{}", base64::engine::general_purpose::STANDARD.encode(Sha512::digest(b)))
}

fn events(got: &Mutex<Vec<(String, Value)>>, name: &str) -> Vec<Value> {
    got.lock()
        .unwrap()
        .iter()
        .flat_map(|(_, b)| b["batch"].as_array().cloned().unwrap_or_default())
        .filter(|e| e["event"] == name)
        .map(|e| e["properties"].clone())
        .collect()
}

async fn until_events(api: &Api, got: &Mutex<Vec<(String, Value)>>, name: &str, n: usize) -> Vec<Value> {
    let at = Instant::now();
    loop {
        api.cmd("telemetry.flush", Value::Null).await;
        let e = events(got, name);
        if e.len() >= n || at.elapsed() > Duration::from_secs(10) {
            return e;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn opencode(agents: &Value) -> Value {
    agents.as_array().unwrap().iter().find(|a| a["id"] == "opencode").cloned().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn with_no_npm_opencode_comes_from_the_registry_checked_and_every_attempt_is_counted() {
    let dir = temp("serve-install");
    let tgz = opencode_tgz(&dir);
    let corrupt = Arc::new(AtomicBool::new(true));
    let (os, arch) = (
        match std::env::consts::OS {
            "macos" => "darwin",
            o => o,
        },
        match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x64",
            a => a,
        },
    );
    let package = format!("opencode-{os}-{arch}");
    let reg_port = Arc::new(Mutex::new(0u16));
    let (bad, rp, pkg, body) = (corrupt.clone(), reg_port.clone(), package.clone(), tgz.clone());
    let (port_r, reg) = fake_http(move |path| {
        let port = *rp.lock().unwrap();
        if path == format!("/{pkg}/latest") {
            // the first answer's checksum is someone else's: nothing may be installed from it
            let integrity = if bad.load(Ordering::SeqCst) { sri(b"not this") } else { sri(&body) };
            let tarball = format!("http://127.0.0.1:{port}/{pkg}/-/{pkg}-1.19.6.tgz");
            (
                200,
                json!({ "name": pkg, "version": "1.19.6", "dist": { "tarball": tarball, "integrity": integrity } })
                    .to_string()
                    .into_bytes(),
            )
        } else if path == format!("/{pkg}/-/{pkg}-1.19.6.tgz") {
            (200, body.clone())
        } else {
            (404, b"{}".to_vec())
        }
    });
    *reg_port.lock().unwrap() = port_r;
    let (ph, got) = fake_http(|_| (200, b"{}".to_vec()));
    let (registry, host) = (format!("http://127.0.0.1:{port_r}"), format!("http://127.0.0.1:{ph}"));
    let no_npm = dir.join("no-such-npm");
    let (serve, port) = start_with(
        &dir,
        &[
            // nothing installed: no opencode override, a PATH without one, no npm
            ("WISP_OPENCODE_BIN", ""),
            ("PATH", "/usr/bin:/bin"),
            ("WISP_NPM", no_npm.to_str().unwrap()),
            ("WISP_NPM_REGISTRY", registry.as_str()),
            ("WISP_POSTHOG_HOST", host.as_str()),
            ("WISP_POSTHOG_KEY", "phc_test"),
            ("CI", ""),
            ("DO_NOT_TRACK", ""),
            ("WISP_TELEMETRY", "1"),
        ],
    );
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let log = || std::fs::read_to_string(dir.join("serve.log")).unwrap_or_default();
    assert_eq!(opencode(&api.cmd("runtime.agents", json!({})).await)["available"], false, "{}", log());

    // a checksum that doesn't match: refused, nothing written, counted with its reason
    let r = api.try_cmd("accounts.install", json!({ "agent": "opencode" })).await;
    assert_eq!(r["ok"], false, "{r}");
    assert!(r.to_string().contains("checksum"), "{r}");
    let bin = dir.join("home/tools/opencode/bin/opencode");
    assert!(!bin.exists(), "nothing unpacked from a download that doesn't match");
    let e = until_events(&api, &got, "harness_install", 1).await;
    assert_eq!(
        (&e[0]["agent"], &e[0]["ok"], &e[0]["how"], &e[0]["reason"]),
        (&json!("opencode"), &json!(false), &json!("download"), &json!("download_failed")),
        "{e:?}"
    );

    // the real one: downloaded, unpacked runnable where detect looks, found, counted
    corrupt.store(false, Ordering::SeqCst);
    let agents = api.cmd("accounts.install", json!({ "agent": "opencode" })).await;
    let oc = opencode(&agents);
    assert_eq!((&oc["available"], &oc["version"]), (&json!(true), &json!("1.19.6")), "{agents} {}", log());
    assert!(bin.exists());
    let e = until_events(&api, &got, "harness_install", 2).await;
    assert_eq!((&e[1]["ok"], &e[1]["how"], &e[1]["reason"]), (&json!(true), &json!("download"), &Value::Null), "{e:?}");
    assert!(e[1]["ms"].as_u64().is_some(), "{e:?}");
    let paths: Vec<String> = reg.lock().unwrap().iter().map(|(p, _)| p.clone()).collect();
    assert!(paths.contains(&format!("/{package}/latest")), "{paths:?}");
    drop(serve);
}
