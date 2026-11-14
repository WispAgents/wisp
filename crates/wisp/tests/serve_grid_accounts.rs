//! Several grids and accounts on one wisp, end to end (ADR-0061, F-mach-21): two local grids (`wisp-grid dev`, in
//! this process), signed in to both and twice to one (two people), each listed with its cloud box and computers in
//! Machines under the grid they came from; `wisp cloud accounts | switch | remove`; the tokens only in secrets.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines, clippy::many_single_char_names)]

mod common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Api, start_with, temp};
use serde_json::{Value, json};

struct Grid {
    port: u16,
    url: String,
    stop: wisp_kernel::CancellationToken,
    _g: std::sync::Arc<wisp_grid::edge::Grid>,
}

impl Drop for Grid {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

async fn grid(dir: &Path) -> Grid {
    std::fs::create_dir_all(dir).unwrap();
    let (g, port, stop) = wisp_grid::edge::dev(
        dir,
        0,
        PathBuf::from(env!("CARGO_BIN_EXE_wisp")),
        vec![("WISP_WEB_DIR".into(), dir.join("no-web").display().to_string())],
        600_000,
    )
    .await
    .unwrap();
    Grid { port, url: format!("http://wisp.localhost:{port}"), stop, _g: g }
}

fn browser(port: u16) -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve("wisp.localhost", SocketAddr::from(([127, 0, 0, 1], port)))
        .build()
        .unwrap()
}

/// `handle`, signed in to the grid in their browser, approves `code`.
async fn approve(g: &Grid, handle: &str, code: &str) {
    let b = browser(g.port);
    let r = b.get(format!("{}/auth/dev?handle={handle}", g.url)).send().await.unwrap();
    let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_owned();
    let page = b.get(format!("{}/activate?code={code}", g.url)).header("cookie", &cookie).send().await.unwrap().text().await.unwrap();
    let csrf = page.split("name=\"csrf\" value=\"").nth(1).unwrap().split('"').next().unwrap().to_owned();
    let r = b
        .post(format!("{}/activate", g.url))
        .header("cookie", &cookie)
        .json(&json!({ "csrf": csrf, "code": code, "answer": "allow" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
}

async fn sign_in(api: &Api, g: &Grid, handle: &str) {
    let f = api.cmd("grid-client.signin", json!({ "grid": g.url })).await;
    assert_eq!(f["state"], "waiting", "{f}");
    approve(g, handle, f["userCode"].as_str().unwrap()).await;
    let at = Instant::now();
    loop {
        let s = api.cmd("grid-client.signinStatus", json!({ "id": f["id"] })).await;
        if s["state"] != "waiting" {
            assert_eq!((s["state"].as_str(), s["handle"].as_str()), (Some("done"), Some(handle)), "{s}");
            return;
        }
        assert!(at.elapsed() < Duration::from_secs(30), "{s}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Another computer of `handle`'s on the grid (signed in there by itself, as another wisp would be).
async fn other_computer(g: &Grid, handle: &str, name: &str) {
    let b = browser(g.port);
    let c: Value = b.post(format!("{}/api/device/code", g.url)).json(&json!({ "name": "t" })).send().await.unwrap().json().await.unwrap();
    approve(g, handle, c["user_code"].as_str().unwrap()).await;
    tokio::time::sleep(Duration::from_millis(5_100)).await;
    let t: Value = b
        .post(format!("{}/api/device/token", g.url))
        .json(&json!({ "device_code": c["device_code"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = t["token"].as_str().unwrap_or_else(|| panic!("{t}"));
    let r = b.post(format!("{}/api/computers", g.url)).bearer_auth(token).json(&json!({ "name": name })).send().await.unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());
}

fn wisp(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The grid peers by (grid, handle, name, transport), sorted.
async fn grid_peers(a: &Api) -> Vec<(String, String, String, String)> {
    let all = a.cmd("machines.list", json!({})).await;
    let mut out: Vec<_> = all
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["grid"].is_string())
        .map(|p| {
            let s = |k: &str| p[k].as_str().unwrap_or_default().to_owned();
            (s("grid"), s("handle"), s("name"), s("transport"))
        })
        .collect();
    out.sort();
    out
}

fn keys(v: &Value) -> Vec<(String, bool)> {
    v.as_array().unwrap().iter().map(|a| (a["key"].as_str().unwrap().to_owned(), a["current"] == true)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn two_grids_and_two_people_on_one_each_listed_switched_and_signed_out_of_alone() {
    let root = temp("serve-grid-accounts");
    let (g1, g2) = (grid(&root.join("g1")).await, grid(&root.join("g2")).await);
    let d = root.join("a");
    std::fs::create_dir_all(&d).unwrap();
    let (_serve, port) = start_with(&d, &[]);
    let a = Api { http: reqwest::Client::new(), port, n: 0.into() };

    other_computer(&g2, "bo", "studio").await;
    sign_in(&a, &g1, "ada").await;
    sign_in(&a, &g2, "bo").await;
    sign_in(&a, &g1, "cy").await;
    let (k1a, k2b, k1c) = (format!("{}#ada", g1.url), format!("{}#bo", g2.url), format!("{}#cy", g1.url));

    // each_account_is_listed: in sign-in order, the newest current
    let st = a.cmd("grid-client.status", json!({})).await;
    assert_eq!(keys(&st["accounts"]), [(k1a.clone(), false), (k2b.clone(), false), (k1c.clone(), true)], "{st}");
    assert_eq!(st["handle"], "cy");

    // each account's cloud box and computers in Machines, with the grid they came from
    let (h1, h2) = (g1.url.clone(), g2.url.clone());
    let want = |list: &[(&str, &str, &str, &str)]| -> Vec<(String, String, String, String)> {
        let mut v: Vec<_> = list.iter().map(|(g, h, n, t)| ((*g).to_owned(), (*h).to_owned(), (*n).to_owned(), (*t).to_owned())).collect();
        v.sort();
        v
    };
    let at = Instant::now();
    let three = want(&[
        (&h1, "ada", "Cloud", "grid"),
        (&h2, "bo", "Cloud (bo)", "grid"),
        (&h2, "bo", "studio", "grid-tunnel"),
        (&h1, "cy", "Cloud (cy)", "grid"),
    ]);
    while grid_peers(&a).await != three && at.elapsed() < Duration::from_secs(20) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(grid_peers(&a).await, three);

    // the CLI lists them, the current marked, and switches by handle
    let (code, out, err) = wisp(&d, &["cloud", "accounts"]);
    assert_eq!(code, 0, "{out}{err}");
    let host1 = g1.url.trim_start_matches("http://");
    assert!(out.contains(&format!("● cy@{host1}")) && out.contains(&format!("ada@{host1}")) && out.contains("bo@"), "{out}");
    let (code, out, err) = wisp(&d, &["cloud", "switch", "bo"]);
    assert_eq!(code, 0, "{out}{err}");
    let st = a.cmd("grid-client.status", json!({})).await;
    assert_eq!((st["handle"].as_str(), st["grid"].as_str()), (Some("bo"), Some(g2.url.as_str())));

    // signing out of ada leaves cy's box, though it's on the same grid; bo stays current
    let (code, out, err) = wisp(&d, &["cloud", "remove", &format!("ada@{host1}")]);
    assert_eq!(code, 0, "{out}{err}");
    let st = a.cmd("grid-client.status", json!({})).await;
    assert_eq!(keys(&st["accounts"]), [(k2b.clone(), true), (k1c.clone(), false)]);
    assert_eq!(
        grid_peers(&a).await,
        want(&[(&h2, "bo", "Cloud (bo)", "grid"), (&h2, "bo", "studio", "grid-tunnel"), (&h1, "cy", "Cloud (cy)", "grid")])
    );

    // signing out of the current one hands over to the first left
    let st = a.cmd("grid-client.accountRemove", json!({ "key": k2b })).await;
    assert_eq!((keys(&st["accounts"]), st["handle"].as_str()), (vec![(k1c.clone(), true)], Some("cy")));
    assert_eq!(grid_peers(&a).await, want(&[(&h1, "cy", "Cloud (cy)", "grid")]));

    // the_tokens_stay_in_secrets: keyed per grid and account, never in the store
    let secrets = std::fs::read_to_string(d.join("home/secrets.json")).unwrap_or_default();
    assert!(secrets.contains(&format!("\"token/{k1c}\"")) && !secrets.contains(&format!("\"token/{k1a}\"")), "{secrets}");
    for f in std::fs::read_dir(d.join("home")).unwrap() {
        let p = f.unwrap().path();
        if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("wisp.db")) {
            assert!(!std::fs::read(&p).unwrap().windows(4).any(|w| w == b"wgd_"), "a token in {}", p.display());
        }
    }
}
