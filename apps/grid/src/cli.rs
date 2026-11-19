//! `wisp-grid …` (v1 grid/cli.js, the parts ported so far):
//!
//! - `wisp-grid dev [--port N] [--data DIR] [--wisp PATH] [--idle-ms N] [--real-harnesses]`: a grid on this machine
//!   for development, with `auth.dev` on (sign in by handle at `/auth/dev`) and boxes as `wisp serve`
//!   processes. Prints its URL, runs until Ctrl-C or SIGTERM. Its boxes get every `WISP_*_BIN` set, and no real
//!   harness CLI unless `--real-harnesses`.
//!
//! - `wisp-grid serve [--data DIR]`: a real grid from `<data>/grid.json` (or `GRID_DATA`), behind Caddy: sign-in
//!   with its providers (GitHub, Google, OIDC), boxes as containers through boxd (`hostd.driver: "container"`,
//!   `hostd.container.boxd`: boxd's socket) or as processes here (`"process"`). Runs until SIGTERM or Ctrl-C.
//!
//! - `wisp-grid waitlist [--data DIR]`: who's waiting for a seat, and who wants more machines (Add a cloud machine).
//!
//! `dev` and `serve` stop every box before they exit, on Ctrl-C or SIGTERM: no box outlives the grid.
use std::path::PathBuf;

/// The flag's value: `--port 8080` or `--port=8080`.
fn flag(args: &[String], name: &str) -> Option<String> {
    let eq = format!("{name}=");
    args.iter().enumerate().find_map(|(i, a)| if a == name { args.get(i + 1).cloned() } else { a.strip_prefix(&eq).map(str::to_owned) })
}

fn wisp_beside() -> PathBuf {
    let name = if cfg!(windows) { "wisp.exe" } else { "wisp" };
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

pub async fn main(args: Vec<String>) -> i32 {
    match args.first().map(String::as_str) {
        Some("dev") => dev(&args).await,
        Some("serve") => match serve(flag(&args, "--data").map(PathBuf::from)).await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("  ✗ wisp-grid serve: {e}");
                1
            }
        },
        Some("waitlist") => match waitlist(flag(&args, "--data").map(PathBuf::from)) {
            Ok(text) => {
                use std::io::Write as _;
                let _ = std::io::stdout().write_all(text.as_bytes());
                0
            }
            Err(e) => {
                eprintln!("  ✗ wisp-grid waitlist: {e}");
                1
            }
        },
        _ => {
            eprintln!(
                "wisp-grid dev [--port N] [--data DIR] [--wisp PATH] [--idle-ms N] [--real-harnesses]\nwisp-grid serve [--data DIR]\nwisp-grid waitlist [--data DIR]"
            );
            2
        }
    }
}

/// Until the grid is told to stop: Ctrl-C, or SIGTERM (systemd, `kill`, a supervisor). Either way the grid stops its
/// boxes before it goes (v1 grid/cli.js), so none outlives it. If the signal can't be watched, Ctrl-C alone.
pub async fn stopped() {
    #[cfg(unix)]
    if let Ok(mut term) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}

const DEV_USAGE: &str = "wisp-grid dev [--port N] [--data DIR] [--wisp PATH] [--idle-ms N] [--real-harnesses]
  a grid on this machine for development (sign in by handle at /auth/dev), until Ctrl-C
  --port N          its port (8080)
  --data DIR        its data (~/.wisp-grid-dev)
  --wisp PATH       the wisp binary its boxes run (the one beside wisp-grid)
  --idle-ms N       a box sleeps after this long idle (120000)
  --real-harnesses  boxes run the harness CLIs on your PATH; without it, only the WISP_<HARNESS>_BIN you set";

/// The flags `dev` takes, each with a value but the last.
const DEV_FLAGS: [&str; 5] = ["--port", "--data", "--wisp", "--idle-ms", "--real-harnesses"];

/// Setup audit B7: `wisp-grid dev --help` started a grid (on :8080, in ~/.wisp-grid-dev). Err((code, text)): print the
/// usage and exit, never a grid (0 for --help, 2 for a word that isn't a flag). Ok: run it, after saying the flags it
/// ignores (an older script's `--fake-agents`: a warning, as before, never a stop).
fn dev_words(args: &[String]) -> Result<Vec<String>, (i32, String)> {
    let mut ignored = Vec::new();
    let mut i = 1;
    while let Some(a) = args.get(i) {
        if matches!(a.as_str(), "--help" | "-h" | "help") {
            return Err((0, DEV_USAGE.to_owned()));
        }
        let name = a.split_once('=').map_or(a.as_str(), |(n, _)| n);
        if !name.starts_with("--") {
            return Err((2, format!("  ✗ wisp-grid dev doesn't take {a}\n{DEV_USAGE}")));
        }
        if !DEV_FLAGS.contains(&name) {
            // its value too, when one follows (`--idle 120000`)
            ignored.push(name.to_owned());
            i += if a.contains('=') || args.get(i + 1).is_none_or(|v| v.starts_with("--")) { 1 } else { 2 };
            continue;
        }
        // a value follows unless it was `--x=v`, or the flag takes none
        i += if a.contains('=') || name == "--real-harnesses" { 1 } else { 2 };
    }
    Ok(ignored)
}

/// The harness CLIs a box looks for, by the variable that names each (each harness plugin's `bin_env`).
const HARNESS_BINS: [(&str, &str); 7] = [
    ("claude", "WISP_CLAUDE_BIN"),
    ("codex", "WISP_CODEX_BIN"),
    ("opencode", "WISP_OPENCODE_BIN"),
    ("cursor-agent", "WISP_CURSOR_BIN"),
    ("copilot", "WISP_COPILOT_BIN"),
    ("grok", "WISP_GROK_BIN"),
    ("agy", "WISP_ANTIGRAVITY_BIN"),
];

/// Setup audit B8: what a dev grid's boxes get, from this process's `env`, and the line that says it. Every
/// `WISP_*_BIN` set here goes to the boxes (fakes: the grid passed only Claude's and Codex's, so the box found this Mac's
/// real Cursor, Grok and Antigravity). A harness with none set never falls back to the real CLI on PATH unless
/// `--real-harnesses` says so: its variable names a file that isn't there, so the box finds it not installed.
fn dev_harness_env(env: &[(String, String)], data: &std::path::Path, real: bool) -> (Vec<(String, String)>, String) {
    let mut out: Vec<(String, String)> = env
        .iter()
        .filter(|(k, v)| !v.is_empty() && ((k.starts_with("WISP_") && k.ends_with("_BIN")) || k == "WISP_WEB_DIR"))
        .cloned()
        .collect();
    let unset: Vec<(&str, &str)> = HARNESS_BINS.iter().copied().filter(|(_, var)| !out.iter().any(|(k, _)| k == var)).collect();
    let names = |l: &[(&str, &str)]| l.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
    let line = if unset.is_empty() {
        "  · boxes run the harnesses your WISP_<HARNESS>_BIN name".to_owned()
    } else if real {
        format!("  ! boxes run the real {} on your PATH (--real-harnesses)", names(&unset))
    } else {
        for (n, var) in &unset {
            out.push(((*var).to_owned(), data.join("no-harness").join(n).display().to_string()));
        }
        format!("  · boxes have no {}: set WISP_<HARNESS>_BIN to a fake, or --real-harnesses for the ones on your PATH", names(&unset))
    };
    (out, line)
}

/// `wisp-grid dev …`: until Ctrl-C or SIGTERM, then every box stopped first.
async fn dev(args: &[String]) -> i32 {
    match dev_words(args) {
        Err((code, text)) => {
            eprintln!("{text}");
            return code;
        }
        Ok(ignored) if !ignored.is_empty() => eprintln!("  ! wisp-grid dev ignores {} (wisp-grid dev --help)", ignored.join(", ")),
        Ok(_) => {}
    }
    let port = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(8080);
    let data =
        flag(args, "--data").map_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".wisp-grid-dev"), PathBuf::from);
    let bin = flag(args, "--wisp").map_or_else(wisp_beside, PathBuf::from);
    let idle = flag(args, "--idle-ms").and_then(|p| p.parse().ok()).unwrap_or(120_000);
    if let Err(e) = std::fs::create_dir_all(&data) {
        eprintln!("  ✗ {}: {e}", data.display());
        return 1;
    }
    // dev only: the fake harnesses and the web bundle for the boxes too, when set here; never a real CLI unasked
    let env: Vec<(String, String)> = std::env::vars().collect();
    let (pass, said) = dev_harness_env(&env, &data, args.iter().any(|a| a == "--real-harnesses"));
    eprintln!("{said}");
    match crate::edge::dev(&data, port, bin, pass, idle).await {
        Ok((grid, port, stop)) => {
            eprintln!(
                "\n  ◌ wisp Grid (dev) at http://wisp.localhost:{port}  ·  sign in at /auth/dev\n    sign in from wisp: wisp cloud signin http://wisp.localhost:{port}\n"
            );
            stopped().await;
            eprintln!("  ◌ stopping every box…");
            // the boxes first, while the grid still records their stops
            grid.boxes.close_all().await;
            grid.boxes_stopped();
            stop.cancel();
            0
        }
        Err(e) => {
            eprintln!("  ✗ {e}");
            1
        }
    }
}

/// The hostd options from the config: the container driver when it says so.
pub fn box_options(cfg: &crate::config::GridConfig) -> crate::Result<crate::hostd::ProcessOptions> {
    let h = &cfg.hostd;
    let boxd = match h.driver.as_str() {
        "container" => Some(h.container.as_ref().and_then(crate::boxd::Driver::of).ok_or_else(|| {
            crate::GridError::Config("hostd.container.boxd: boxd's socket (the container driver runs boxes through wisp-grid-boxd).".into())
        })?),
        "process" => None,
        other => return Err(crate::GridError::Config(format!("hostd.driver: process or container (not {other})."))),
    };
    let url_cfg = cfg.clone();
    Ok(crate::hostd::ProcessOptions {
        bin: h.wisp_bin.as_ref().map_or_else(wisp_beside, PathBuf::from),
        data: cfg.data_dir.clone(),
        idle_ms: h.idle_ms,
        check_ms: h.check_ms,
        start_ms: h.start_ms,
        max_running: usize::try_from(h.max_running).unwrap_or(1).max(1),
        // the grid itself, for what a box asks of it (its channels' URLs, mail replies): v1 hostd.js `WISP_GRID_URL`
        env: std::iter::once(("WISP_GRID_URL".to_owned(), cfg.public_url.clone()))
            .chain(h.env.iter().map(|(k, v)| (k.clone(), v.clone())))
            .collect(),
        url_of: std::sync::Arc::new(move |handle| url_cfg.instance_url(handle)),
        boxd,
        previews: cfg.previews(),
    })
}

/// `wisp-grid waitlist [--data DIR]`: the operator's list (`wishes::report`), read from grid.db.
fn waitlist(data: Option<PathBuf>) -> crate::Result<String> {
    let cfg = crate::config::load(crate::config::LoadOptions { data_dir: data, ..crate::config::LoadOptions::default() })?;
    let clock = crate::clock::system();
    let db = std::sync::Arc::new(crate::db::Db::open(&cfg.data_dir, clock.now_ms())?);
    crate::wishes::report(&crate::accounts::Accounts { db, clock })
}

async fn serve(data: Option<PathBuf>) -> crate::Result<()> {
    let cfg = crate::config::load(crate::config::LoadOptions { data_dir: data, ..crate::config::LoadOptions::default() })?;
    let clock = crate::clock::system();
    let db = std::sync::Arc::new(crate::db::Db::open(&cfg.data_dir, clock.now_ms())?);
    let boxes = crate::hostd::ProcessHost::new(box_options(&cfg)?, clock.clone());
    let at = format!("{}:{}", cfg.listen.host, cfg.listen.port);
    let listener = tokio::net::TcpListener::bind(&at).await.map_err(|e| crate::bad(format!("can't listen on {at}: {e}"), 500))?;
    let providers: Vec<String> = cfg.auth.providers.keys().cloned().collect();
    eprintln!(
        "  ◌ wisp Grid at {} (listening on {at}) · sign-in: {} · boxes: {}",
        cfg.public_url,
        if providers.is_empty() { "none set up (auth.providers)".to_owned() } else { providers.join(", ") },
        cfg.hostd.driver
    );
    let grid = crate::edge::Grid::new(cfg, db, clock, boxes);
    let stop = tokio_util::sync::CancellationToken::new();
    let server = tokio::spawn(crate::edge::serve(grid.clone(), listener, stop.clone()));
    stopped().await;
    eprintln!("  ◌ stopping every box…");
    grid.boxes.close_all().await;
    grid.boxes_stopped();
    stop.cancel();
    let _ = server.await;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn w(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Setup audit B7: `wisp-grid dev --help` started a dev grid on :8080 in ~/.wisp-grid-dev. It prints the usage; a
    /// word dev doesn't take is refused; the flags it takes still run it.
    #[test]
    fn dev_help_prints_the_usage_and_a_stray_word_is_refused_never_a_grid() {
        for h in [&["dev", "--help"][..], &["dev", "-h"], &["dev", "--port", "9", "--help"]] {
            let (code, text) = dev_words(&w(h)).err().unwrap_or_else(|| panic!("{h:?} ran a grid"));
            assert!(code == 0 && text.contains("--real-harnesses") && text.contains("--port N"), "{h:?}: {text}");
        }
        for bad in [&["dev", "start"][..], &["dev", "--port=9", "extra"]] {
            let (code, text) = dev_words(&w(bad)).err().unwrap_or_else(|| panic!("{bad:?} ran a grid"));
            assert!(code == 2 && text.contains("doesn't take"), "{bad:?}: {text}");
        }
        for ok in
            [&["dev"][..], &["dev", "--port", "4972", "--data", "/tmp/g"], &["dev", "--idle-ms=5", "--real-harnesses", "--wisp", "/x"]]
        {
            assert_eq!(dev_words(&w(ok)), Ok(Vec::new()), "{ok:?}");
        }
        // qa/grid.mjs's older flags: said, not a stop
        assert_eq!(
            dev_words(&w(&["dev", "--fake-agents", "--port", "8090", "--idle", "120000", "--fake-delay", "800"])),
            Ok(vec!["--fake-agents".to_owned(), "--idle".to_owned(), "--fake-delay".to_owned()])
        );
    }

    /// Setup audit B8: a dev grid with fake Claude and Codex gave its boxes only those two, so they ran this Mac's real
    /// Cursor, Grok and Antigravity. Every `WISP_*_BIN` set goes to the boxes, and the rest are off unless asked for.
    #[test]
    fn dev_boxes_get_every_fake_harness_and_never_a_real_cli_unasked() {
        let env: Vec<(String, String)> = [
            ("WISP_CLAUDE_BIN", "/f/claude"),
            ("WISP_CODEX_BIN", "/f/codex"),
            ("WISP_GROK_BIN", "/f/grok"),
            ("WISP_GH_BIN", "/f/gh"),
            ("WISP_WEB_DIR", "/w"),
            ("PATH", "/usr/bin"),
            ("WISP_COPILOT_BIN", ""),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let data = std::path::Path::new("/d");
        let (pass, said) = dev_harness_env(&env, data, false);
        let get = |k: &str| pass.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        assert_eq!(get("WISP_GROK_BIN").as_deref(), Some("/f/grok"), "every fake goes to the boxes");
        assert_eq!(get("WISP_GH_BIN").as_deref(), Some("/f/gh"));
        assert_eq!(get("WISP_WEB_DIR").as_deref(), Some("/w"));
        assert!(get("PATH").is_none());
        for (n, var) in [
            ("cursor-agent", "WISP_CURSOR_BIN"),
            ("copilot", "WISP_COPILOT_BIN"),
            ("agy", "WISP_ANTIGRAVITY_BIN"),
            ("opencode", "WISP_OPENCODE_BIN"),
        ] {
            assert_eq!(get(var), Some(data.join("no-harness").join(n).display().to_string()), "{var}: off, not the real one");
        }
        assert!(said.contains("cursor-agent") && said.contains("--real-harnesses"), "said, not silent: {said}");
        let (pass, said) = dev_harness_env(&env, data, true);
        assert!(!pass.iter().any(|(k, _)| k == "WISP_CURSOR_BIN") && said.contains("the real ") && said.contains("cursor-agent"), "{said}");
    }

    #[test]
    fn serve_runs_boxes_through_boxd_when_the_config_says_container() {
        let dir = std::env::temp_dir().join(format!("wisp-grid-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = |hostd: serde_json::Value| crate::config::for_test(&dir, serde_json::json!({ "domain": "grid.test", "hostd": hostd }));
        let o = box_options(
            &cfg(serde_json::json!({ "driver": "container", "container": { "boxd": "/run/wisp-grid/box.sock", "memory": "2g" } })).unwrap(),
        )
        .unwrap();
        let d = o.boxd.unwrap();
        assert_eq!((d.socket.to_str(), d.limits["memory"].as_str()), (Some("/run/wisp-grid/box.sock"), Some("2g")));
        let e = cfg(serde_json::json!({ "driver": "container", "container": {} }))
            .map_err(|e| e.to_string())
            .and_then(|c| box_options(&c).map(|_| ()).map_err(|e| e.to_string()))
            .unwrap_err();
        assert!(e.contains("boxd"), "{e}");
        assert!(box_options(&cfg(serde_json::json!({ "driver": "process" })).unwrap()).unwrap().boxd.is_none());
    }
}
