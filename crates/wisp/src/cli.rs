//! The generated half of the command line (ADR-0022): `wisp <plugin> <verb>` for every command the
//! caller may run, `wisp help [plugin]` and `wisp completions zsh|bash|fish`, all from `cli.catalogue`
//! (wisp.cli). The parsing, help and rendering are wisp-cli-core's; this is the IO around them.
use std::io::IsTerminal;
use std::path::Path;

use serde_json::{Value, json};
use wisp_cli_core::verbs::{self, Catalogue};
use wisp_cli_core::{Images, Style, completions, human};

use crate::client::{CliError, Client, Result};
use crate::scripted::say;

/// What `wisp` exits with when the command line didn't fit (a refusal from wisp exits 1).
pub const USAGE_EXIT: i32 = 2;

/// The terminal's width: `COLUMNS`, else `stty size` on the terminal, else 80.
pub fn columns() -> usize {
    if let Some(n) = std::env::var("COLUMNS").ok().and_then(|c| c.parse::<usize>().ok()).filter(|n| *n >= 20) {
        return n;
    }
    #[cfg(unix)]
    if let Ok(tty) = std::fs::File::open("/dev/tty")
        && let Ok(o) = std::process::Command::new("stty").arg("size").stdin(tty).stderr(std::process::Stdio::null()).output()
        && let Some(n) = String::from_utf8_lossy(&o.stdout).split_whitespace().nth(1).and_then(|c| c.parse::<usize>().ok())
        && n >= 20
    {
        return n;
    }
    80
}

/// How stdout draws: colour on a terminal unless `NO_COLOR` (or `TERM=dumb`); OSC 8 links with colour;
/// inline images in `iTerm2`, `WezTerm` and kitty.
pub fn style() -> Style {
    let tty = std::io::stdout().is_terminal();
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) || std::env::var("TERM").is_ok_and(|t| t == "dumb");
    let width = if tty { columns() } else { std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()).unwrap_or(80) };
    if !tty || no_color {
        return Style::plain(width);
    }
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let images = if program == "iTerm.app" || program == "WezTerm" {
        Images::Iterm
    } else if std::env::var_os("KITTY_WINDOW_ID").is_some() || std::env::var("TERM").is_ok_and(|t| t.contains("kitty")) {
        Images::Kitty
    } else {
        Images::None
    };
    Style { images, ..Style::color(width) }
}

/// The catalogue this caller sees, from the running wisp (kept in `<home>/cli-catalogue.json` too).
pub async fn catalogue(c: &Client, home: &Path) -> Result<Catalogue> {
    let list = c.cmd("cli.catalogue", json!({})).await?;
    if let Ok(text) = serde_json::to_vec(&list) {
        let _ = std::fs::write(home.join("cli-catalogue.json"), text);
    }
    let list: Vec<wisp_kernel::CommandInfo> = serde_json::from_value(list).map_err(|e| CliError::said(format!("cli.catalogue: {e}")))?;
    Ok(Catalogue::new(list))
}

/// The catalogue as this wisp offers it, for help, completion and running alike (setup audit B6, C6): Auto's verbs
/// (`wisp router …`) only while its one switch is on (`runtime.auto`, off until the owner reworks it: 2026-11-17).
/// Off, `wisp router …` is a word that's no command, as `wisp help router` says; the code stays.
pub fn offered(mut cat: Catalogue, auto: bool) -> Catalogue {
    if !auto {
        cat.verbs.retain(|v| v.plugin != "router");
    }
    cat
}

/// [`offered`] with the switch read from the running wisp.
pub async fn shown(c: &Client, cat: Catalogue) -> Catalogue {
    let auto = c.cmd("settings.get", json!({ "key": "runtime.auto" })).await.ok() == Some(Value::Bool(true));
    offered(cat, auto)
}

/// [`shown`] only when the words could reach what it puts away (`router`): every other call skips the switch's read.
pub async fn shown_for(c: &Client, cat: Catalogue, first: Option<&String>) -> Catalogue {
    if first.is_some_and(|w| cat.verbs.iter().any(|v| v.plugin == *w && v.plugin == "router")) { shown(c, cat).await } else { cat }
}

/// The command line from `first` on, as typed, each word that needs it in quotes: a refusal's `wisp -- …` way out
/// (audit F orchestration F6: it kept only the words, dropping `--agent codex --on done`, and `--cron "every minute"`
/// lost its quotes).
pub fn as_typed(argv: &[String], first: &str) -> String {
    let start = argv.iter().position(|a| a == first).unwrap_or(0);
    argv[start..]
        .iter()
        .map(|a| {
            if !a.is_empty() && a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+%".contains(c)) {
                a.clone()
            } else {
                format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The words after the verb, as typed: from the raw argv (the global parser has eaten flags such as
/// `--mode` that a command may declare too).
fn rest_after(argv: &[String], first: &str, taken: usize) -> Vec<String> {
    let start = argv.iter().position(|a| a == first).unwrap_or(0);
    argv.get(start + taken..).map(<[String]>::to_vec).unwrap_or_default()
}

/// `wisp <plugin> <verb> …` when the words name one; None: they don't (a task, then).
pub async fn generated(c: &Client, cat: &Catalogue, words: &[String], argv: &[String]) -> Result<Option<i32>> {
    let Some((v, taken)) = cat.resolve(words) else { return Ok(None) };
    // a verb whose plugin waits for its first call is listed without its argument type: ask again with it woken, so
    // its flags are checked (a flag it doesn't take refused, not sent as a field) and `--help` lists them
    let woken;
    let (v, taken) = if v.info.args.is_none() {
        let list = c.cmd("cli.catalogue", json!({ "wake": v.info.id })).await?;
        let list: Vec<wisp_kernel::CommandInfo> =
            serde_json::from_value(list).map_err(|e| CliError::said(format!("cli.catalogue: {e}")))?;
        woken = Catalogue::new(list);
        woken.resolve(words).filter(|(w, _)| w.info.args.is_some()).unwrap_or((v, taken))
    } else {
        (v, taken)
    };
    let (own, rest) = verbs::own(&rest_after(argv, &words[0], taken));
    if own.help {
        say(&verbs::help_verb(v));
        return Ok(Some(0));
    }
    let mut args = match verbs::build(v, &rest) {
        Ok(a) => a,
        Err(u) => {
            // every refusal says the way out (QA round 14)
            eprintln!("  ✗ {u}\n  a task that says this: wisp -- {}", as_typed(argv, &words[0]));
            return Ok(Some(USAGE_EXIT));
        }
    };
    let named = goal_by_name(&v.info.id, &rest, &mut args);
    let sent = args.clone();
    let out = match c.cmd(&v.info.id, args).await {
        Err(CliError::Http { status, message, body }) if named && message.contains("no goal named") => {
            return Err(CliError::Http { status, message: format!("{message}; use --text \"…\" for words"), body });
        }
        r => r?,
    };
    let json = own.json || argv.iter().any(|a| a == "--json");
    match crate::said::said(&v.info.id, &sent, &out).filter(|_| !json) {
        Some(line) => say(&line),
        None => print_result(&out, json),
    }
    Ok(Some(0))
}

/// `wisp goal set <task> <name>`: one word after the task is a declared goal's name, refused when no goal has it, as
/// `--goal` is at launch; `--text "…"` (or several words, which no name is) sets words. True: the word was sent as a
/// name. Audit G16: `wisp goal set <task> nevr`, a typo of `never`, silently became the goal "nevr".
fn goal_by_name(id: &str, rest: &[String], args: &mut Value) -> bool {
    let flag = |f: &str| rest.iter().any(|a| a == f || a.starts_with(&format!("{f}=")));
    if id != "goal.set" || flag("--text") || flag("--name") {
        return false;
    }
    let Some(word) = args.get("text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty() && !t.contains(char::is_whitespace))
    else {
        return false;
    };
    args["name"] = json!(word);
    args["text"] = json!("");
    true
}

pub fn print_result(v: &Value, json: bool) {
    if json {
        say(&serde_json::to_string_pretty(v).unwrap_or_default());
        return;
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_millis()).ok());
    say(&human::render(v, &style(), now).join("\n"));
}

/// `wisp help` and `wisp help <plugin>` (`wisp help <plugin> <verb>` is that verb's `--help`).
pub fn help(cat: &Catalogue, words: &[String]) -> i32 {
    // `wisp help runtime "queue order"`: a quoted verb as its list prints it is its words (audit B #15)
    let words: Vec<String> = words.iter().flat_map(|w| w.split_whitespace()).map(str::to_owned).collect();
    match words.get(1..).unwrap_or_default() {
        [] => say(&verbs::help_all(cat, crate::args::HELP)),
        // a built-in group (`wisp help computer`, QA round 11 #2): its usage, and its plugin's verbs when it has one
        [p] if crate::words::is_group(p) => {
            let usage = crate::words::usage(p);
            match verbs::help_plugin(cat, p) {
                // a group the usage doesn't document (`settings`): its verbs, not "needs more" (audit G GS10)
                Some(h) if usage.contains(" needs more: ") => say(&h),
                Some(h) => say(&format!("{usage}\n\n{h}")),
                None => say(&usage),
            }
        }
        [p] => {
            let Some(h) = verbs::help_plugin(cat, p) else {
                // `wisp help me` (QA round 15 #6): how to get help, and the way to a task
                eprintln!(
                    "  ✗ wisp help {p}: no command or plugin named {p}\n  wisp help lists every command and plugin; wisp help <plugin> its verbs; wisp --help the usage\n  a task that says this: wisp -- help {p}"
                );
                return USAGE_EXIT;
            };
            say(&h);
        }
        more => {
            let Some((v, _)) = cat.resolve(more) else {
                if cat.has_plugin(&more[0]) || crate::words::is_group(&more[0]) {
                    eprintln!("  ✗ no command wisp {}: wisp help {} lists its verbs", more.join(" "), more[0]);
                } else {
                    // `wisp help me fix the login page`: never a task (audit B #15), the way to one is said
                    eprintln!(
                        "  ✗ wisp help {}: no command or plugin named {}\n  wisp help lists every command and plugin\n  a task that says this: wisp -- help {}",
                        more.join(" "),
                        more[0],
                        more.join(" ")
                    );
                }
                return USAGE_EXIT;
            };
            say(&verbs::help_verb(v));
        }
    }
    0
}

/// `wisp sdk types ts|py [FILE]`: the SDKs' typed commands, printed or written to FILE.
pub fn sdk_types(cat: &Catalogue, lang: Option<&String>, out: Option<&String>) -> i32 {
    let Some(text) = lang.and_then(|l| wisp_cli_core::sdk::types(cat, l)) else {
        eprintln!("  ✗ wisp sdk types ts|py [FILE]");
        return USAGE_EXIT;
    };
    match out {
        None => {
            // the text ends with its own newline
            say(text.strip_suffix('\n').unwrap_or(&text));
            0
        }
        Some(f) => match std::fs::write(f, text) {
            Ok(()) => {
                say(&format!("  ✓ wrote {f}"));
                0
            }
            Err(e) => {
                eprintln!("  ✗ couldn't write {f}: {e}");
                1
            }
        },
    }
}

pub async fn completions(c: &Client, cat: &Catalogue, shell: Option<&String>) -> i32 {
    // what the catalogue doesn't say (setup audit F27): wisp's own groups' verbs, the harnesses, the setting keys
    // every harness here, as `--agent` takes it (setup audit B10: accounts' rows named only claude and codex)
    let agents = c.cmd("runtime.agents", json!({})).await.ok();
    let settings = c.cmd("settings.list", json!({})).await.ok();
    let strings = |v: Option<&Value>, k: &str| -> Vec<String> {
        v.and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.get(k).and_then(Value::as_str).map(str::to_owned)).collect()
    };
    // an enum setting's values, after its key (B10: `wisp settings set worktrees.max <TAB>` offered flags)
    let values = settings
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| {
            let vs: Vec<String> = x.pointer("/decl/values")?.as_array()?.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
            Some((x.get("key")?.as_str()?.to_owned(), vs)).filter(|(_, vs)| !vs.is_empty())
        })
        .collect();
    // `wisp export <TAB>`: the kinds this wisp has (by section, as a formula names them), then export's flags
    let kinds = c.cmd("resources.kinds", json!({})).await.ok();
    let mut export = strings(kinds.as_ref(), "section");
    export.extend(["--format", "--out", "--include", "--with-deps", "--single-file"].map(str::to_owned));
    // `--goal <TAB>`: the goals declared by name (`goals:` in a formula, ADR-0110; none from a wisp without wisp.goal)
    let goals = c.cmd("goal.declared", json!({})).await.ok();
    let extra = completions::Extra {
        groups: crate::words::group_verbs(),
        agents: strings(agents.as_ref(), "id"),
        settings: strings(settings.as_ref(), "key"),
        values,
        after: vec![("export".to_owned(), export)],
        goals: strings(goals.as_ref(), "name"),
    };
    let Some(s) = shell.and_then(|sh| completions::script_with(sh, cat, &crate::args::COMMANDS, &extra)) else {
        eprintln!("  ✗ wisp completions zsh|bash|fish");
        return USAGE_EXIT;
    };
    say(&s);
    0
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;

    /// Audit G16: the word after the task is a goal's name; `--text`, or several words, are words.
    #[test]
    fn goal_set_sends_one_word_as_a_name_audit_g16() {
        let w = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let mut a = json!({ "id": "t1", "text": "nevr" });
        assert!(goal_by_name("goal.set", &w("t1 nevr"), &mut a));
        assert_eq!(a, json!({ "id": "t1", "text": "", "name": "nevr" }));
        for (rest, text) in [("t1 --text nevr", "nevr"), ("t1 --text=nevr", "nevr"), ("t1 the tests pass", "the tests pass"), ("t1", "")] {
            let mut a = json!({ "id": "t1", "text": text });
            assert!(!goal_by_name("goal.set", &w(rest), &mut a), "{rest}");
            assert_eq!(a["text"], text, "{rest}");
        }
        assert!(!goal_by_name("crew.update", &w("t1 x"), &mut json!({ "id": "t1", "text": "x" })), "only goal.set");
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with("node_modules") && !p.ends_with("dist") {
                files(&p, out);
            } else if p.extension().is_some_and(|x| x == "ts" || x == "tsx") {
                out.push(p);
            }
        }
    }

    /// Every command the plugin manifests declare.
    fn declared() -> Vec<wisp_kernel::CommandInfo> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(root().join("plugins")).unwrap().flatten() {
            let Ok(text) = std::fs::read_to_string(e.path().join("host/wisp-plugin.json")) else { continue };
            let m: Value = serde_json::from_str(&text).unwrap();
            for c in m["contributes"]["commands"].as_array().into_iter().flatten() {
                out.push(wisp_kernel::CommandInfo {
                    id: c["id"].as_str().unwrap().to_owned(),
                    plugin: m["id"].as_str().unwrap().to_owned(),
                    title: c["title"].as_str().map(str::to_owned),
                    read_only: false,
                    permission: String::new(),
                    args: None,
                    ui_only: c["uiOnly"].as_str().map(str::to_owned),
                });
            }
        }
        out
    }

    /// The command ids the UI calls (`defineCommand({ id: '…' })`).
    fn ui_calls() -> BTreeSet<String> {
        let mut paths = Vec::new();
        for d in ["packages/shell/src", "packages/client/src", "apps/web/src"] {
            files(&root().join(d), &mut paths);
        }
        for e in std::fs::read_dir(root().join("plugins")).unwrap().flatten() {
            files(&e.path().join("ui/src"), &mut paths);
        }
        let mut out = BTreeSet::new();
        for p in paths {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            for (at, _) in text.match_indices("defineCommand") {
                let near = &text[at..text.len().min(at + 400)];
                if let Some(i) = near.find("id: '") {
                    let id: String = near[i + 5..].chars().take_while(|c| *c != '\'').collect();
                    if id.contains('.') {
                        out.insert(id);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_command_the_ui_calls_has_a_cli_path() {
        let cat = Catalogue::new(declared());
        // every declared command resolves from its own path
        for v in &cat.verbs {
            let mut words = vec![v.plugin.clone()];
            words.extend(v.words.iter().cloned());
            assert_eq!(
                cat.resolve(&words).map(|(x, _)| x.info.id.clone()),
                Some(v.info.id.clone()),
                "wisp {} doesn't reach {}",
                words.join(" "),
                v.info.id
            );
        }
        let calls = ui_calls();
        assert!(calls.len() > 20, "the UI's commands were found: {calls:?}");
        let missing: BTreeSet<String> = calls.iter().filter(|id| cat.by_id(id).is_none()).cloned().collect();
        // the known gaps (UI written ahead of its host): the list only shrinks
        let known: BTreeSet<String> = std::fs::read_to_string(root().join("plugins/cli/parity-gaps.txt"))
            .unwrap()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_owned)
            .collect();
        let new: Vec<&String> = missing.difference(&known).collect();
        assert!(new.is_empty(), "the UI calls commands no manifest declares, so the CLI can't reach them: {new:?}");
        let closed: Vec<&String> = known.difference(&missing).collect();
        assert!(closed.is_empty(), "these have a host command now: take them off plugins/cli/parity-gaps.txt: {closed:?}");
    }

    /// Audit B #15: `wisp help runtime` lists "queue order"; asking help for that spelling shows the verb's help.
    #[test]
    fn help_for_a_verb_spelled_as_its_list_prints_it_shows_its_help() {
        let cat = Catalogue::new(declared());
        let w = |x: &[&str]| x.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(help(&cat, &w(&["help", "runtime", "queue order"])), 0);
        assert_eq!(help(&cat, &w(&["help", "runtime", "queue", "send-now"])), 0);
        assert_eq!(help(&cat, &w(&["help", "runtime", "no such verb"])), USAGE_EXIT);
    }

    /// Audit F orchestration F6: the hint kept the words only (`wisp -- crew create --name Bob proj`) and lost the
    /// quotes of `--cron "every minute"`.
    #[test]
    fn a_refusals_way_out_keeps_every_flag_and_its_quotes_audit_f_orchestration_f6() {
        let argv: Vec<String> = [
            "--port",
            "9",
            "crew",
            "create",
            "--name",
            "Bob",
            "--agent",
            "codex",
            "--on",
            "done",
            "--cron",
            "every minute",
            "--x",
            "it's \"q\" $HOME",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            as_typed(&argv, "crew"),
            r#"crew create --name Bob --agent codex --on done --cron "every minute" --x "it's \"q\" \$HOME""#
        );
    }

    #[test]
    fn rest_after_reads_the_raw_words() {
        let argv: Vec<String> = ["--port", "9", "crew", "create", "--mode", "auto"].map(str::to_owned).to_vec();
        assert_eq!(rest_after(&argv, "crew", 2), ["--mode", "auto"]);
    }

    /// Audit C6: `wisp help router` said there's no such command while `wisp router` ran and talked about Auto. While
    /// `runtime.auto` is off the router plugin is put away: its verbs neither resolve nor list, so `wisp router …`
    /// answers like any word that's no command, as help does. On, it's back.
    #[test]
    fn auto_off_puts_the_router_away_for_running_too_audit_c6() {
        let w = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let off = offered(Catalogue::new(declared()), false);
        assert!(off.resolve(&w(&["router", "preview"])).is_none());
        assert!(!off.plugins().iter().any(|(p, _)| *p == "router"));
        assert!(off.resolve(&w(&["settings", "get"])).is_some(), "the rest stays");
        let on = offered(Catalogue::new(declared()), true);
        assert!(on.resolve(&w(&["router", "preview"])).is_some());
        assert_eq!(
            crate::words::not_a_task(&w(&["router"]), crate::args::Literal::No, &[]),
            Some(crate::words::NotATask::Unknown("router".into())),
            "with the plugin away, `wisp router` is a word that's no command"
        );
    }
}
