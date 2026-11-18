//! `wisp`: the one binary (ADR-0013). Parses the command line, then starts only what the command
//! needs: a scripted command talks to the running wisp, `serve` (and the default with nothing
//! running) runs the kernel and the edge here.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod args;
mod attach;
#[cfg(test)]
mod ci_tests;
mod cli;
mod client;
mod cloud_cli;
mod code;
mod commands;
mod desktop;
mod events_cli;
mod extras;
mod folder;
mod fresh_cli;
mod groups;
mod history_cli;
mod hook_cli;
mod mcp_tasks;
mod notes_cli;
mod offload_cli;
mod plugin_cmd;
mod repo_cli;
mod scripted;
mod secret_cli;
mod serve;
mod services;
mod settings_store;
mod share_cli;
mod tasks_cli;
mod termctx_cli;
mod trial_cli;
mod trigger_cli;
mod trim;
mod tui;
mod update_cli;
mod v1_formula;
mod words;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use args::{Action, Parsed};
use client::{CliError, Client, Result};
use scripted::say;
use serde_json::{Value, json};

fn main() {
    // before any thread: glibc's arenas are made per thread (ADR-0087)
    #[cfg(feature = "proc")]
    wisp_proc::tune_heap();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    // Claude Code's Bash hook runs on every command: answer before the runtime starts (ADR-0079)
    if offload_cli::fast_path(&argv) {
        return;
    }
    let user_home = services::user_home().unwrap_or_else(|| PathBuf::from("."));
    // the CLI's folder as every path in wisp is spelled (ADR-0071): `wisp "task"` in `C:\Users\RUNNER~1\…` runs in
    // the folder its project stores as `C:\Users\runneradmin\…`
    let cwd = services::plain(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let parsed = match args::parse(&argv, std::env::var("WISP_PORT").ok().as_deref(), &user_home, &cwd) {
        Ok(p) => p,
        Err(e) => die(&e.to_string()),
    };
    match parsed.action {
        Action::Version => return say(serve::VERSION),
        Action::Help => return say(args::HELP),
        Action::Run => {}
    }
    init_tracing();
    let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(worker_threads()).enable_all().build() {
        Ok(rt) => rt,
        Err(e) => die(&format!("runtime: {e}")),
    };
    let code = rt.block_on(run(parsed, &user_home, &cwd));
    rt.shutdown_timeout(std::time::Duration::from_secs(2));
    std::process::exit(code);
}

/// A dev tool shares the machine: a few workers are plenty (the blocking pool does compression).
fn worker_threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4))
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("WISP_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).try_init();
}

fn die(msg: &str) -> ! {
    eprintln!("  ✗ {msg}");
    std::process::exit(1)
}

async fn run(p: Parsed, user_home: &Path, cwd: &Path) -> i32 {
    match dispatch(&p, user_home, cwd).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("  ✗ {e}");
            1
        }
    }
}

async fn dispatch(p: &Parsed, user_home: &Path, cwd: &Path) -> Result<i32> {
    let home = services::home(std::env::var("WISP_HOME").ok().as_deref(), Some(user_home.to_owned()));
    let f = &p.flags;
    let cwd_s = f.cwd.clone().unwrap_or_else(|| cwd.to_owned()).display().to_string();
    // scripted commands go to the advertised wisp unless --port was given
    let port = if f.port_given { f.port } else { client::discover(&home).unwrap_or(f.port) };
    // `--on <machine>`: every command from here runs on that peer (ADR-0041)
    if let Some(m) = &f.on {
        client::set_on(m);
    }
    let rest = p.words.get(1..).unwrap_or_default();
    // a task this terminal left running that finished, asks or failed: one line before anything else
    if !matches!(p.command, Some("serve" | "mcp" | "grid" | "desktop" | "completions" | "sdk")) && Client::new(port).ping().await {
        attach::notices(&Client::new(port), &home).await;
    }
    if let Some(code) = misused_verb_path(p, true) {
        return Ok(code);
    }
    if let Some(code) = as_code(p, port, f.cwd.as_deref().unwrap_or(cwd), &home).await? {
        return Ok(code);
    }
    // the catalogue `live` read for words it didn't take, for `not_a_task` (read once)
    let mut cat = None;
    if let Some(code) = live(p, &home, port, &mut cat).await? {
        return Ok(code);
    }
    if f.help {
        say(args::HELP);
        return Ok(0);
    }
    if p.command.is_none()
        && let Some(code) = not_a_task(p, home.as_path(), port, cat).await
    {
        return Ok(code);
    }
    // a group command with a verb it doesn't have (`wisp plugin frob`, `wisp mcp start`): its verbs, exit 2
    // a group that routed as a command (`mcp`, `skill`, `plugin`): its next word decides, however many follow (`wisp
    // mcp foo bar` would serve on stdin)
    if p.command.is_some()
        && let Some(u) = words::unknown_verb(&p.words[..p.words.len().min(2)], p.literal, &[])
    {
        eprintln!("{}", u.said(&p.words));
        return Ok(cli::USAGE_EXIT);
    }
    // a built-in verb path its arm didn't take (`computer on --try x`) is that command misused, never a task
    if let Some(code) = misused_verb_path(p, false) {
        return Ok(code);
    }
    match p.command {
        Some("grid") => return extras::grid(&p.argv),
        Some("mcp") => {
            // the stdio server takes `--repo <dir>` only: any other flag is a slip (`wisp mcp tools --x` served on)
            if let Some(bad) = words::stray_flag("mcp", &["--repo"], &p.argv) {
                eprintln!("  ✗ wisp mcp doesn't take {bad}\n{}", words::usage("mcp"));
                return Ok(cli::USAGE_EXIT);
            }
            return repo_cli::mcp(&p.argv, cwd, port).await;
        }
        Some("skill") => {
            extras::skill(p.words.get(1).map(String::as_str), f, user_home, cwd)?;
            if p.words.get(1).map(String::as_str) == Some("install") {
                return repo_cli::skill_install(&p.argv, cwd);
            }
        }
        Some("serve") => serve_here(&home, f.port, f.host.as_deref(), false, None).await?,
        Some("desktop") => {
            desktop::desktop(&home, user_home, port, desktop::reach(port).await?, true).await?;
        }
        Some("plugin" | "plugins") if rest.first().map(String::as_str) == Some("new") => {
            let name = scripted::need(rest.get(1), "a plugin name")?;
            let dir = plugin_cmd::new_plugin(&home, name)?;
            let id = format!("user.{}", dir.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
            say(&format!("  ✓ {}\n    edit wisp-plugin.json, then: wisp plugin enable {id}", dir.display()));
            // listed at once, off (setup audit F18: `wisp plugin` and Settings → Plugins showed nothing until enabled)
            if let Ok(c) = Client::connect(port, false).await
                && let Err(e) = c.cmd("plugins.setEnabled", json!({ "id": id, "enabled": false })).await
            {
                eprintln!("  ◌ {id} isn't listed yet: {e}");
            }
        }
        // `-C dir` is where the folder is, as the parser looked; `~` is the user's home
        Some("add") => add(user_home, port, p.words.get(1), f.cwd.as_deref().unwrap_or(cwd)).await?,
        // `wisp run the tests`: no recipe of that name, so a prompt, launched as every prompt is (its title, attached in
        // a terminal; QA round 12 #2 printed the id where the title goes)
        // `wisp run foo`, no recipe or workflow of that name: one stray word is a slip (QA round 15 #2); more is a prompt
        Some("run") if p.words.len() == 2 && p.literal == args::Literal::No && !p.words[1].contains(char::is_whitespace) => {
            eprintln!("  ✗ no recipe or workflow named {}: wisp recipes lists them\n{}", p.words[1], words::usage("run"));
            return Ok(cli::USAGE_EXIT);
        }
        Some("run") if !(f.json || f.wait || !f.tries.is_empty() || f.compare.is_some()) => {
            return launch_and_open(p, &home, port, &cwd_s).await;
        }
        Some(cmd) => commands::run(p, cmd, rest, &cwd_s, port).await?,
        None if !p.words.is_empty() && (f.json || f.wait || !f.tries.is_empty() || f.compare.is_some()) => {
            commands::run(p, "launch", &p.words, &cwd_s, port).await?;
        }
        None if !p.words.is_empty() => return launch_and_open(p, &home, port, &cwd_s).await,
        None => desktop::open(&home, user_home, port, f.open, f.browser).await?,
    }
    Ok(0)
}

/// QA round 9 #1 (`wisp computer on --grid …` started a task): words that start with a built-in verb path are that
/// command. `flags`: refuse a flag it doesn't take, before anything runs; else (past every arm, on the launch path)
/// refuse whatever it was given. Either way its usage, exit 2. None: not a verb path, or after `--` (a task on purpose).
fn misused_verb_path(p: &Parsed, flags: bool) -> Option<i32> {
    if p.literal == args::Literal::Task || p.command.is_some() {
        return None;
    }
    // `wisp update --help`: a group alone with --help is its usage, never the command (it ran a check)
    if flags
        && p.flags.help
        && let [g] = p.words.as_slice()
        && words::is_group(g)
    {
        say(&words::usage(g));
        return Some(0);
    }
    let Some((cmd, verb)) = words::verb_path(&p.words) else {
        // past every arm, a built-in group's line its own command refused (`wisp sdk types`, `wisp crew ls extra`)
        let first = p.words.first().filter(|_| !flags && words::group_line(&p.words))?;
        eprintln!("  ✗ wisp {} isn't a {first} command", p.words.join(" "));
        eprintln!("{}", words::usage(first));
        return Some(cli::USAGE_EXIT);
    };
    if flags && p.flags.help {
        // `wisp computer on --help` is its usage, never the command
        say(&words::usage(cmd));
        return Some(0);
    }
    if flags {
        let bad = words::bad_flag(&p.words, &p.argv)?;
        eprintln!("  ✗ wisp {cmd} {verb} doesn't take {bad}");
    } else if p.words.len() > 2 {
        eprintln!("  ✗ wisp {cmd} {verb} doesn't take {}", p.words[2..].join(" "));
    } else {
        eprintln!("  ✗ wisp {cmd} {verb} needs more");
    }
    eprintln!("{}", words::usage(cmd));
    Some(cli::USAGE_EXIT)
}

/// Words on the launch path that aren't a task (`wisp tasks`, `wisp tsks`, `wisp runtime`): what they
/// say instead and the exit code; None: launch them. The catalogue is read only when the word isn't one
/// of the CLI's own (a plugin's name, or a typo of one).
async fn not_a_task(p: &Parsed, home: &Path, port: u16, mut cat: Option<wisp_cli_core::verbs::Catalogue>) -> Option<i32> {
    let first = p.words.first()?;
    // a built-in group and another word (`wisp crew start`): its plugin verbs too, to name them
    let maybe_group = p.words.len() >= 2 && p.literal != args::Literal::Task && words::is_group(first);
    if cat.is_none()
        && ((words::command_shaped(&p.words, p.literal) && (!words::known(first) || words::is_group(first))) || maybe_group)
        && let Ok(c) = Client::connect(port, true).await
    {
        cat = cli::catalogue(&c, home).await.ok();
    }
    let paths: Vec<Vec<String>> = cat
        .as_ref()
        .map(|c| c.verbs.iter().map(|v| std::iter::once(v.plugin.clone()).chain(v.words.iter().cloned()).collect()).collect())
        .unwrap_or_default();
    // QA round 10 #1: `wisp computer start` is a command typed wrong, never a task
    if let Some(u) = words::unknown_verb(&p.words, p.literal, &paths) {
        eprintln!("{}", u.said(&p.words));
        return Some(cli::USAGE_EXIT);
    }
    let plugins: Vec<String> = cat.as_ref().map(|c| c.plugins().iter().map(|(p, _)| (*p).to_owned()).collect()).unwrap_or_default();
    match words::not_a_task(&p.words, p.literal, &plugins)? {
        words::NotATask::Usage(w) => {
            eprintln!("{}", words::usage(&w));
            Some(cli::USAGE_EXIT)
        }
        words::NotATask::Plugin(w) => {
            if let Some(h) = cat.as_ref().and_then(|c| wisp_cli_core::verbs::help_plugin(c, &w)) {
                say(&h);
                Some(0)
            } else {
                eprintln!("{}", words::usage(&w));
                Some(cli::USAGE_EXIT)
            }
        }
        words::NotATask::Near(w) => {
            let line = p.words.join(" ");
            eprintln!("  ✗ wisp {line} isn't a command: did you mean wisp {w}?   (a task that says this: wisp -- {line})");
            Some(cli::USAGE_EXIT)
        }
        words::NotATask::Instead(said) => {
            let line = p.words.join(" ");
            // in a wisp checkout, `npm run doctor` is the one that checks it (AGENTS.md)
            let said = if first == "doctor" && wisp_checkout(p.flags.cwd.clone()).is_some() {
                "npm run doctor (checks this checkout)".to_owned()
            } else {
                said
            };
            eprintln!("  ✗ wisp {line} isn't a command: did you mean {said}?   (a task that says this: wisp -- {line})");
            Some(cli::USAGE_EXIT)
        }
        words::NotATask::Unknown(w) => {
            eprintln!("  ✗ wisp {w} isn't a command: wisp help lists them   (a task that says this: wisp -- {})", p.words.join(" "));
            Some(cli::USAGE_EXIT)
        }
    }
}

/// The wisp checkout this folder is in (`tools/doctor.ts` beside `crates/wisp`), if any.
fn wisp_checkout(cwd: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    let here = cwd.or_else(|| std::env::current_dir().ok())?;
    here.ancestors().find(|d| d.join("tools/doctor.ts").is_file() && d.join("crates/wisp").is_dir()).map(Path::to_path_buf)
}

/// Whether `word` could be a plugin's first word (`crew`, `runtime`, `grid-client`).
fn plugin_word(word: &str) -> bool {
    word.len() <= 32
        && word.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// The commands that follow a topic or read the catalogue (ADR-0022); None: not one of them.
/// wisp as code (ADR-0026, ADR-0027): apply, diff, validate, schema, export; task spec/export/formula,
/// rerun, run -f, run <workflow|task>. None: the words aren't one of them.
async fn as_code(p: &Parsed, port: u16, cwd: &Path, home: &Path) -> Result<Option<i32>> {
    let words = &p.words;
    let Some(first) = words.first().map(String::as_str) else { return Ok(None) };
    if p.literal == args::Literal::Task {
        return Ok(None);
    }
    let start = p.argv.iter().position(|a| a == first).map_or(0, |i| i + 1);
    // `wisp ls -a` is --all (the parser said so); here `-a` would read as --agent
    let rest: Vec<String> =
        p.argv[start.min(p.argv.len())..].iter().filter(|a| !(matches!(first, "ls" | "list") && *a == "-a")).cloned().collect();
    let o = code::opts(&rest);
    let o = code::Opts {
        json: o.json || p.flags.json,
        yes: o.yes || p.flags.yes,
        dry_run: o.dry_run || p.flags.dry_run,
        wait: o.wait || p.flags.wait,
        // `wisp --grid <grid> computer on`: the flag before the command word counts too
        grid: o.grid.or_else(|| p.flags.grid.clone()),
        ..o
    };
    if let Some(code) = areas(first, p, &o, port, cwd).await? {
        return Ok(Some(code));
    }
    if let Some(code) = shared_and_cloud(first, p, &o, port, cwd).await? {
        return Ok(Some(code));
    }
    let connect = || Client::connect(port, true);
    let code = match first {
        "apply" | "diff" | "validate" | "plan" | "destroy" if !o.files.is_empty() || folder::folder(&o, cwd).is_some() => {
            code::apply(&connect().await?, first, &o, cwd).await?
        }
        // a prompt that starts with init (`wisp init the database schema`) stays one; a flag goes to init, which refuses
        // the ones it doesn't take (`wisp init auto --force` launched an agent)
        "init" if o.words.len() <= 1 || o.words.iter().any(|w| w.starts_with('-')) => folder::init(cwd, &o)?,
        "track" if o.words.len() == 1 => repo_cli::track(&connect().await?, &o, cwd).await?,
        "untrack" if o.words.len() == 1 => repo_cli::untrack_cmd(Client::connect(port, false).await.ok().as_ref(), &o, cwd).await?,
        "blame" if o.words.len() == 1 => repo_cli::blame_any(Client::connect(port, false).await.ok().as_ref(), &o, cwd).await?,
        "log" if o.words.len() <= 1 => repo_cli::log(&p.argv, &o, cwd)?,
        "tasks" if p.argv.iter().any(|a| a == "--touching" || a == "--tracked") => {
            repo_cli::log(&p.argv, &code::Opts { words: Vec::new(), ..o.clone() }, cwd)?
        }
        "ls" | "list" if p.argv.iter().any(|a| a == "--repo") => {
            repo_cli::log(&p.argv, &code::Opts { words: Vec::new(), ..o.clone() }, cwd)?
        }
        "resume" if o.words.len() == 1 => repo_cli::resume(&connect().await?, &o, cwd).await?,
        "secret" => secret_cli::run(&connect().await?, &o.words).await?,
        "adopt" => repo_cli::adopt(&connect().await?, &p.argv, &o, cwd).await?,
        "fmt" if o.words.is_empty() => folder::fmt_cmd(&o, cwd)?,
        "workspace" => folder::workspace_cmd(&folder::folder(&o, cwd).unwrap_or_else(|| cwd.join(".wisp")), &o.words)?,
        "output" | "graph" | "mv" if folder::folder(&o, cwd).is_some() => folder_view(first, &o, port, cwd).await?,
        "import" if o.words.len() == 1 && o.words[0].contains('.') && !o.words[0].contains('/') && !o.words[0].starts_with('.') => {
            let c = connect().await?;
            let ks = code::kinds(&c).await?;
            folder::import_address(&c, &ks, &folder::folder(&o, cwd).unwrap_or_else(|| cwd.join(".wisp")), &o.words[0]).await?
        }
        "schema" if o.words.is_empty() => code::schema(&connect().await?).await?,
        "export" if o.words.iter().any(|w| w.contains('.')) && o.words.iter().all(|w| !w.contains(' ')) => {
            code::export_cmd(&connect().await?, &o).await?
        }
        "export" if o.words.iter().all(|w| w.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')) => {
            let c = connect().await?;
            if !code::kind_words(&o.words, &code::kinds(&c).await?) {
                return Ok(None);
            }
            code::export_cmd(&c, &o).await?
        }
        "ls" | "list" if history_cli::wants_history(&o) && !p.flags.watch => history_cli::ls(&connect().await?, &o, cwd).await?,
        "history" if o.words.first().map(String::as_str) == Some("export") => history_cli::export(&connect().await?, &o, cwd).await?,
        "import" if o.words.len() <= 1 => history_cli::import(&connect().await?, &o, home).await?,
        "rerun" if (o.words.len() == 1 && args::session_id(&o.words[0])) || o.last.is_some() => {
            let o = code::Opts { words: std::iter::once("rerun".to_owned()).chain(o.words.clone()).collect(), ..o };
            tasks_cli::rerun(&connect().await?, &o, cwd).await?
        }
        "run" if !o.files.is_empty() => tasks_cli::run_file(&connect().await?, &o, cwd).await?,
        "run" if !o.formulas.is_empty() && !o.words.is_empty() => tasks_cli::run_overlay(&connect().await?, &o, cwd).await?,
        "run" if !o.words.is_empty() => match tasks_cli::run_named(&connect().await?, &o.words, &o, &p.flags).await? {
            Some(code) => code,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// `wisp output`, `wisp graph` and `wisp mv` over the declared folder.
async fn folder_view(first: &str, o: &code::Opts, port: u16, cwd: &Path) -> Result<i32> {
    let c = Client::connect(port, true).await?;
    let ks = code::kinds(&c).await?;
    let dir = folder::folder(o, cwd).unwrap_or_default();
    let cfg = folder::config(o, &ks, &dir)?;
    Ok(match first {
        "output" => folder::output(&cfg, o),
        "graph" => folder::graph(&cfg),
        _ => match o.words.as_slice() {
            [from, to] => {
                let (stored, kind) =
                    folder::stored_name(&cfg, from).ok_or_else(|| CliError::said(format!("{from} isn't declared here")))?;
                if folder::stored_name(&cfg, to).is_some() {
                    return Err(CliError::said(format!("{to} is declared already")));
                }
                let new_name = format!(
                    "{}{}",
                    stored.strip_suffix(from.split_once('.').map_or("", |(_, n)| n)).unwrap_or_default(),
                    to.split_once('.').map_or("", |(_, n)| n)
                );
                let has = c
                    .cmd("resources.export", serde_json::json!({ "kinds": [kind] }))
                    .await
                    .is_ok_and(|l| l.as_array().is_some_and(|a| a.iter().any(|r| r["name"] == stored.as_str())));
                if has {
                    c.cmd("resources.rename", serde_json::json!({ "kind": kind, "from": stored, "to": new_name })).await?;
                }
                let n = folder::mv(&dir, &cfg, from, to)?;
                scripted::say(&format!(
                    "  ✓ {from} → {to} in {n} file{}{}",
                    if n == 1 { "" } else { "s" },
                    if has { " and in wisp" } else { "" }
                ));
                0
            }
            _ => return Err(CliError::said("wisp mv <kind>.<name> <kind>.<new name>")),
        },
    })
}

/// `wisp task spec|export|formula|setup|history`, `wisp share`, `wisp events`, `wisp shares`, `wisp cloud`,
/// `wisp computer`, `wisp grid deploy` and `wisp open`. None: the words aren't one of them.
async fn shared_and_cloud(first: &str, p: &Parsed, o: &code::Opts, port: u16, cwd: &Path) -> Result<Option<i32>> {
    let connect = || Client::connect(port, true);
    let o = o.clone();
    let code = match first {
        "task" if matches!(o.words.first().map(String::as_str), Some("spec" | "export" | "formula" | "setup")) && o.words.len() >= 2 => {
            tasks_cli::task(&connect().await?, &o).await?
        }
        "task" if o.words.first().map(String::as_str) == Some("history") && o.words.len() == 2 => {
            history_cli::task_history(&connect().await?, &o.words[1], o.json).await?
        }
        "share" if o.words.len() == 1 && args::session_id(&o.words[0]) => share_cli::share(&connect().await?, &o, cwd).await?,
        "share"
            if o.words.len() == 1
                && o.words[0].split_once('.').is_some_and(|(k, n)| !k.is_empty() && !n.is_empty() && !n.contains(' ')) =>
        {
            cloud_cli::share_setup(&connect().await?, &o).await?
        }
        "events" if matches!(o.words.first().map(String::as_str), None | Some("tail" | "why" | "retry" | "parked" | "ls")) => {
            events_cli::run(&connect().await?, &p.argv, o.json).await?
        }
        // `wisp notes <task>`, `--set …` or bare (WISP_TASK); "notes on the refactor" stays a task
        "notes" if o.words.first().is_none_or(|w| args::session_id(w) || w.starts_with("--set")) => {
            notes_cli::run(&connect().await?, &o.words, o.json).await?
        }
        // `wisp fresh <task>` or bare (WISP_TASK); "fresh look at the parser" stays a task
        "fresh" if o.words.first().is_none_or(|w| args::session_id(w)) && o.words.len() <= 1 => {
            fresh_cli::run(&connect().await?, &o).await?
        }
        "shares" if o.words.is_empty() => cloud_cli::shares(&connect().await?, &o).await?,
        "cloud"
            if o.words.len() <= 2
                && matches!(
                    o.words.first().map(String::as_str),
                    None | Some("signin" | "login" | "signout" | "logout" | "status" | "accounts" | "switch" | "remove" | "rm")
                ) =>
        {
            cloud_cli::cloud(&connect().await?, &o).await?
        }
        "computer" if o.words.len() <= 1 && matches!(o.words.first().map(String::as_str), None | Some("on" | "off" | "status")) => {
            cloud_cli::computer(&connect().await?, &o).await?
        }
        // `wisp grid deploy -f <file> [--on <box>]`: the formula applied on a box (the cloud box by default)
        "grid" if o.words.first().map(String::as_str) == Some("deploy") && !o.files.is_empty() => {
            grid_deploy(port, p.flags.on.is_some(), o, cwd).await?
        }
        "open" if o.words.len() == 1 && cloud_cli::is_link(&o.words[0]) => cloud_cli::open_link(&connect().await?, &o, cwd).await?,
        "open"
            if o.words.len() == 1
                && (o.words[0].ends_with(".wisp-task") || o.words[0].starts_with("http") || o.words[0].starts_with("gist:")) =>
        {
            share_cli::open(&connect().await?, &o, cwd).await?
        }
        "open" if o.words.len() == 1 => repo_cli::open_task(&connect().await?, &p.argv, &o, cwd).await?,
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// `wisp shell-init`, `wisp trigger …`, `wisp crew wake|graph`, `wisp sandbox …`, `wisp team …` (ADR-0023, 0043, 0041).
async fn areas(first: &str, p: &Parsed, o: &code::Opts, port: u16, cwd: &Path) -> Result<Option<i32>> {
    let connect = || Client::connect(port, true);
    let o = o.clone();
    let code = match first {
        "hook"
            if o.words.len() <= 1
                && matches!(o.words.first().map(String::as_str), None | Some("install" | "uninstall" | "remove" | "status")) =>
        {
            hook_cli::run(&o.words, cwd)?
        }
        "tui" if o.words.is_empty() => tui::run(&connect().await?, &cwd.display().to_string()).await?,
        // QA round 11 #1: what people type for these (`wisp version`, v1's `wisp usage`)
        "version" if o.words.is_empty() => {
            say(serve::VERSION);
            0
        }
        "update" if o.words.is_empty() => update_cli::check(&connect().await?, o.json).await?,
        "update" if o.words.len() == 1 && matches!(o.words.first().map(String::as_str), Some("apply")) => {
            update_cli::apply(&connect().await?, o.json).await?
        }
        // every account's limits this period and what each task took of them; `--agent X`: that harness's, named
        "usage" if o.words.is_empty() => {
            let c = connect().await?;
            let v = match o.over.get("agent").and_then(Value::as_str) {
                Some(a) => c.cmd("budget.view", json!({ "agent": a, "named": true })).await?,
                None => c.cmd("budget.planWindows", json!({})).await?,
            };
            cli::print_result(&v, o.json);
            0
        }
        "shell-init" if o.words.len() == 1 && matches!(o.words[0].as_str(), "zsh" | "bash" | "fish") => {
            match termctx_cli::shell_init(&o.words[0]) {
                Ok(script) => {
                    say(&script);
                    0
                }
                Err(e) => return Err(CliError::said(e)),
            }
        }
        "trigger" | "triggers"
            if matches!(
                o.words.first().map(String::as_str),
                None | Some("add" | "ls" | "list" | "rm" | "remove" | "fire" | "test" | "log" | "resume" | "approve" | "sources")
            ) =>
        {
            trigger_cli::run(&connect().await?, p, first, &cwd.display().to_string()).await?
        }
        "crew" if matches!(o.words.first().map(String::as_str), Some("wake" | "graph")) => {
            trigger_cli::run(&connect().await?, p, "crew", &cwd.display().to_string()).await?
        }
        "sandbox" | "sandboxes"
            if matches!(o.words.first().map(String::as_str), None | Some("new" | "add" | "ls" | "list" | "rm" | "remove" | "delete")) =>
        {
            cloud_cli::sandbox(&connect().await?, &o).await?
        }
        "team" | "teams"
            if matches!(
                o.words.first().map(String::as_str),
                None | Some("ls" | "list" | "new" | "create" | "show" | "add" | "rm" | "remove")
            ) =>
        {
            cloud_cli::team(&connect().await?, &o).await?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// `wisp grid deploy -f <file> [--on <box>]`: the formula applied on a box (the cloud box by default).
async fn grid_deploy(port: u16, named: bool, o: code::Opts, cwd: &Path) -> Result<i32> {
    if !named {
        let status = Client::connect(port, true).await?.cmd("grid-client.status", json!({})).await?;
        let machine = status.get("machine").and_then(Value::as_str).unwrap_or_default();
        if machine.is_empty() {
            return Err(CliError::said("not signed in to wisp Cloud: wisp cloud signin (or name the box: --on <machine>)"));
        }
        client::set_on(machine);
    }
    let o = code::Opts { words: Vec::new(), ..o };
    code::apply(&Client::connect(port, true).await?, "apply", &o, cwd).await
}

async fn live(p: &Parsed, home: &Path, port: u16, read: &mut Option<wisp_cli_core::verbs::Catalogue>) -> Result<Option<i32>> {
    let f = &p.flags;
    let words = &p.words;
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let code = match p.command {
        // `last` is the newest task; a task that isn't there is said at once (QA round 14 #7: `attach last` hung)
        Some("attach") => {
            let c = Client::connect(port, true).await?;
            let id = scripted::task_id(&c, scripted::need(words.get(1), "session id")?).await?;
            attach::attach(&c, home, &id, attach::Mode { interactive: tty, until_idle: !tty, fresh: false }).await?
        }
        Some("show") if f.watch => {
            let c = Client::connect(port, true).await?;
            let id = scripted::task_id(&c, scripted::need(words.get(1), "session id")?).await?;
            attach::attach(&c, home, &id, attach::Mode { interactive: false, until_idle: true, fresh: false }).await?
        }
        Some("ls" | "list") if f.watch => attach::ls_watch(&Client::connect(port, true).await?, f.all).await?,
        Some("help") => {
            let c = Client::connect(port, true).await?;
            cli::help(&cli::catalogue(&c, home).await?, words)
        }
        Some("completions") => {
            let c = Client::connect(port, true).await?;
            cli::completions(&c, &cli::catalogue(&c, home).await?, words.get(1)).await
        }
        Some("sdk") => {
            let c = Client::connect(port, true).await?;
            cli::sdk_types(&cli::catalogue(&c, home).await?, words.get(2), words.get(3))
        }
        // `wisp <plugin> <verb> …`: a command of the catalogue, else (it names none) a task
        None if p.literal != args::Literal::Task && words.first().is_some_and(|w| plugin_word(w)) && words.len() >= 2 => {
            let c = Client::connect(port, true).await?;
            let cat = cli::catalogue(&c, home).await?;
            let mut done = cli::generated(&c, &cat, words, &p.argv).await?;
            // `wisp machine add`: this machine's plugin has no `add`, `machines` does (QA round 11 #1)
            if done.is_none()
                && let Some(alias) = words::machines_alias(words)
            {
                // the command word, not a flag's value named machine (`-C machine machine add`): followed by the verb
                let at = p
                    .argv
                    .windows(2)
                    .position(|w| w[0] == "machine" && w[1] == words[1])
                    .or_else(|| p.argv.iter().position(|a| a == "machine"));
                let argv: Vec<String> =
                    p.argv.iter().enumerate().map(|(i, a)| if Some(i) == at { "machines".to_owned() } else { a.clone() }).collect();
                done = cli::generated(&c, &cat, &alias, &argv).await?;
            }
            let Some(code) = done else {
                *read = Some(cat);
                return Ok(None);
            };
            code
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// Runs wisp here until a signal; `open`: then show it in the browser at `path`.
pub(crate) async fn serve_here(home: &Path, port: u16, host: Option<&str>, open: bool, path: Option<&str>) -> Result<()> {
    let r = serve::boot(home, port, host).await?;
    let url = r.server.url();
    tracing::info!("listening at {url} after {} ms", r.started.elapsed().as_millis());
    eprintln!("\n  ◌ wisp is floating at {url}\n");
    if open {
        serve::open_url(&format!("{url}{}", path.unwrap_or("/")));
    }
    serve::until_signal(r).await;
    Ok(())
}

/// `wisp add [path|repo URL]`: a `project` resource (a URL is cloned here first, through git).
async fn add(user_home: &Path, port: u16, what: Option<&String>, cwd: &Path) -> Result<()> {
    let c = Client::connect(port, true).await?;
    let w = what.map_or("", String::as_str);
    let lower = w.to_ascii_lowercase();
    let url = ["https:", "http:", "ssh:", "git:", "git@", "github.com/", "www.github.com/"].iter().any(|p| lower.starts_with(p));
    let path = if url {
        eprintln!("  ◌ cloning {w}…");
        let cloned = c.cmd("git.clone", json!({ "url": w, "parent": cwd.display().to_string() })).await?;
        cloned.get("dir").and_then(Value::as_str).unwrap_or_default().to_owned()
    } else {
        let at = args::expand(if w.is_empty() { "." } else { w }, user_home, cwd);
        // a folder that isn't there is never a project (QA round 13 #1 made "a", "tests", "dark")
        if !at.is_dir() {
            return Err(CliError::said(format!(
                "no folder {}: wisp add <folder>   (a task that starts with add: wisp -- add {w} …)",
                at.display()
            )));
        }
        // `wisp add .` is the folder itself, never "/repo/." (and no `\\?\` on Windows): canonical before it's sent
        wisp_runtime_api::paths::canonical_project_path(&at.display().to_string(), None)
    };
    // named on the server, in the store's transaction (ADR-0076, ADR-0077): `/a/wisp` then `/b/wisp` are `wisp` and
    // `wisp-2`, never the second replacing the first; a folder that's a project already stays that one
    let added = c.cmd("runtime.projectAdd", json!({ "path": path })).await?;
    let name = added.get("name").and_then(Value::as_str).unwrap_or_default();
    let path = added.get("path").and_then(Value::as_str).unwrap_or(&path);
    if added.get("created").and_then(Value::as_bool) != Some(true) {
        say(&format!("  ✓ already a project: {name}  ({path})"));
        return Ok(());
    }
    say(&format!("  ✓ project added: {name}  ({path})"));
    Ok(())
}

/// `wisp "task"`: launched on the running wisp (one starts if none answers). In a terminal it stays
/// attached (the run live, then a reply box); `-d`, or no terminal, returns with its id.
async fn launch_and_open(p: &Parsed, home: &Path, port: u16, cwd: &str) -> Result<i32> {
    let c = Client::connect(port, true).await?;
    let prompt = p.words.join(" ");
    let mut args = scripted::launch_args(&p.flags, &prompt, cwd);
    scripted::in_project(&c, &mut args).await;
    termctx_cli::attach(&c, p, &mut args, &prompt, cwd, home).await;
    let made = c.cmd("runtime.create", args).await?;
    let id = made.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
    let title = c
        .cmd("runtime.get", json!({ "id": id }))
        .await
        .ok()
        .and_then(|t| t.get("title").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if tty && !p.flags.detach && !id.is_empty() {
        let st = cli::style();
        say(&format!(
            "{} {}  {}",
            wisp_cli_core::term::paint(&st, wisp_cli_core::term::BLUE, "◌"),
            wisp_cli_core::term::bold(&st, &title),
            wisp_cli_core::term::dim(&st, &id)
        ));
        return attach::attach(&c, home, &id, attach::Mode { interactive: true, until_idle: false, fresh: true }).await;
    }
    say(&format!("  ◌ launched: {title}  ({id})\n    wisp attach {id}"));
    attach::remember(home, &id, &title);
    Ok(0)
}
