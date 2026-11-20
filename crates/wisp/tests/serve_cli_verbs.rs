//! QA round 9 #1: `wisp computer on --grid wisp.localhost:4797`, what the Grid's Add a computer prints, launched an agent
//! task titled "computer on", twice. Every verb path the CLI knows, built in (`words::VERB_PATHS`) or a plugin's verb
//! from the live catalogue, with a flag it doesn't take: never a task, and a usage error (exit non-zero) for every
//! built-in path and every typed catalogue verb (an untyped one takes each flag as a field, `--args JSON`). The
//! real binary against `wisp serve` with the fake harness (nothing paid ever runs).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::json;

fn wisp(dir: &Path, port: u16, args: &[String]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// [`wisp`] for every line, 8 processes at a time, answers in the lines' order: each is a separate CLI reading its own
/// arguments, and one after another they were ~200 spawns, over a minute on Windows (spawns cost ~10× there).
fn wisp_all(dir: &Path, port: u16, lines: &[Vec<String>]) -> Vec<(i32, String, String)> {
    let mut out = Vec::with_capacity(lines.len());
    for batch in lines.chunks(8) {
        std::thread::scope(|s| {
            let running: Vec<_> = batch.iter().map(|l| s.spawn(move || wisp(dir, port, l))).collect();
            out.extend(running.into_iter().map(|h| h.join().unwrap()));
        });
    }
    out
}

/// `words::VERB_PATHS`, read from its source (one `vp("cmd", &[verbs…], …)` per line), so this test follows the table.
fn built_in_paths() -> Vec<Vec<String>> {
    let src = include_str!("../src/words.rs");
    let mut out = Vec::new();
    for line in src.lines().map(str::trim).filter(|l| l.starts_with("vp(\"")) {
        let cmd = line.trim_start_matches("vp(\"").split('"').next().unwrap().to_owned();
        let verbs = line.split("&[").nth(1).unwrap().split(']').next().unwrap();
        for v in verbs.split(',').map(|v| v.trim().trim_matches('"')).filter(|v| !v.is_empty()) {
            out.push(vec![cmd.clone(), v.to_owned()]);
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn no_verb_path_with_a_bad_flag_ever_becomes_a_task() {
    let dir = temp("serve-cli-verbs");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let built_in = built_in_paths();
    assert!(built_in.len() > 40, "{} built-in verb paths", built_in.len());
    let catalogue = api.cmd("cli.catalogue", json!({})).await;
    // (path, typed): a typed verb's flags come from its argument type, so a flag it doesn't have is an error; an
    // untyped one (no declared type: `--args JSON`, each flag a field) can't tell, but is never a task either
    let plugin: Vec<(Vec<String>, bool)> = catalogue
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let (ns, verbs) = wisp_cli_core::verbs::path_of(c["id"].as_str().unwrap());
            (std::iter::once(ns).chain(verbs).collect(), c.get("args").is_some_and(serde_json::Value::is_string))
        })
        .collect();
    let typed = plugin.iter().filter(|(_, t)| *t).count();
    assert!(typed > 50, "{typed} typed catalogue verbs");
    let mut wrong = Vec::new();
    // every verb refuses a flag it doesn't take (exit non-zero): a typed one by its argument type, an untyped one by
    // what its command needs (`--args JSON`); an untyped verb that needs nothing took the flag as a field and exited 0,
    // so those declare their (empty or optional) argument types
    let flagged: Vec<Vec<String>> = built_in
        .iter()
        .chain(plugin.iter().map(|(p, _)| p))
        .map(|path| path.iter().cloned().chain(["--definitely-not-a-flag".to_owned()]).collect())
        .collect();
    for (args, (code, out, err)) in flagged.iter().zip(wisp_all(&dir, port, &flagged)) {
        if code == 0 || out.contains("launched") || err.contains("launched") {
            wrong.push(format!("wisp {} → {code}: {}{}", args.join(" "), out.trim(), err.trim()));
        }
    }
    // --grid goes with `computer on` only
    for verb in ["off", "status"] {
        let args = ["computer", verb, "--grid", "wisp.localhost:4797"].map(str::to_owned);
        let (code, _, err) = wisp(&dir, port, &args);
        if code == 0 {
            wrong.push(format!("wisp computer {verb} --grid … took it: {err}"));
        }
    }
    // past every arm (more words than the path takes): still that command's usage, not a task
    for extra in [&["computer", "on", "now", "please"][..], &["cloud", "status", "of", "my", "box"], &["hook", "install", "it"]] {
        let (code, out, err) = wisp(&dir, port, &extra.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>());
        if code != 2 || out.contains("launched") {
            wrong.push(format!("wisp {} → {code}: {out}{err}", extra.join(" ")));
        }
    }
    // `--help` on a verb path is its usage, never the command (it used to turn the computer on)
    let (code, out, err) = wisp(&dir, port, &["computer", "on", "--help"].map(str::to_owned));
    if code != 0 || !out.contains("wisp computer on|off|status") {
        wrong.push(format!("wisp computer on --help → {code}: {out}{err}"));
    }
    // a path missing what it needs says so
    let (code, _, err) = wisp(&dir, port, &["task", "history"].map(str::to_owned));
    if code != 2 || !err.contains("wisp task history needs more") {
        wrong.push(format!("wisp task history → {code}: {err}"));
    }
    // the flags these verbs do take still work, with no flag and with theirs (the 34 that took any flag: typed now)
    let proj = dir.join("proj").display().to_string();
    for args in [
        &["settings", "list"][..],
        &["channels", "status"],
        &["stuff", "list"],
        &["stuff", "find", "--query", "logo", "--limit", "3"],
        &["stuff", "uploads"],
        &["resources", "export", "--kinds", "settings"],
        &["resources", "write", "--kinds", "settings", "--format", "json"],
        &["resources", "kinds"],
        &["workflows", "list"],
        &["automations", "tasks"],
        &["worktrees", "view"],
        &["learn", "view"],
        &["plugin", "ls"],
        &["plugins", "list", "--json"],
        &["mcp", "--repo", proj.as_str()],
    ] {
        let (code, out, err) = wisp(&dir, port, &args.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>());
        if code != 0 {
            wrong.push(format!("wisp {} → {code}: {out}{err}", args.join(" ")));
        }
    }
    // a verb whose plugin waits for its first call (issues, onCommand) knows its flags too: `--project` is one
    let (code, out, err) = wisp(&dir, port, &["issues", "list", "--help"].map(str::to_owned));
    if code != 0 || !out.contains("--project") {
        wrong.push(format!("wisp issues list --help → {code}: {out}{err}"));
    }
    assert!(wrong.is_empty(), "{} took a flag they don't: \n{}", wrong.len(), wrong.join("\n"));
    let tasks = api.cmd("runtime.list", json!({})).await;
    assert_eq!(tasks.as_array().unwrap().len(), 0, "no task was made: {tasks}");
    // the usage is that command's
    let (code, _, err) = wisp(&dir, port, &["computer", "on", "--definitely-not-a-flag"].map(str::to_owned));
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--definitely-not-a-flag") && err.contains("wisp computer on|off|status"), "{err}");
    drop(serve);
}

/// Every command group's word, built in (`words::VERB_PATHS` and `words::GROUPS`, read from their source lines).
fn built_in_groups() -> Vec<String> {
    let src = include_str!("../src/words.rs");
    let groups = src.split("const GROUPS").nth(1).unwrap().split("];").next().unwrap();
    let mut out: Vec<String> = built_in_paths().into_iter().map(|p| p[0].clone()).collect();
    for line in groups.lines().map(str::trim).filter(|l| l.starts_with("(\"")) {
        out.push(line.trim_start_matches("(\"").split('"').next().unwrap().to_owned());
    }
    out.sort();
    out.dedup();
    out
}

/// QA round 10 #1: `wisp computer start|stop|enable`, `wisp recipes list` and `wisp grid start` launched agent tasks
/// (each a paid run on a real setup). Under every built-in command group, a word that's none of its verbs: exit 2, its
/// verbs named, the way to make it a task said, and no task. A prompt that starts with such a word is still a task as
/// one quoted argument, or after `--`. Another plugin's name followed by none of its verbs is a prompt (owner's
/// delegate: `wisp <prompt>` is how tasks start), and one of its verbs with a bad flag is still refused.
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_verb_under_any_group_is_an_error_and_a_quoted_prompt_is_still_a_task() {
    let dir = temp("serve-cli-groups");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let groups = built_in_groups();
    assert!(
        ["computer", "cloud", "recipes", "grid", "crew", "plugin", "plugins", "skill", "completions", "sdk", "mcp", "machines", "trigger"]
            .iter()
            .all(|g| groups.contains(&(*g).to_owned())),
        "{groups:?}"
    );
    let mut lines: Vec<Vec<String>> = groups.iter().map(|g| vec![g.clone(), "definitely-not-a-verb".into()]).collect();
    // what QA typed
    for l in [&["computer", "start"][..], &["computer", "stop"], &["computer", "enable"], &["recipes", "list"], &["grid", "start"]] {
        lines.push(l.iter().map(|w| (*w).to_owned()).collect());
    }
    let mut wrong = Vec::new();
    for (l, (code, out, err)) in lines.iter().zip(wisp_all(&dir, port, &lines)) {
        let said = format!("To start a task with these words: wisp -- \"{}\"", l.join(" "));
        if code != 2 || out.contains("launched") || !err.contains(&said) || !err.contains(&format!("wisp {} has no {}", l[0], l[1])) {
            wrong.push(format!("wisp {} → {code}: {}{}", l.join(" "), out.trim(), err.trim()));
        }
    }
    assert!(wrong.is_empty(), "{} of {} lines weren't refused: \n{}", wrong.len(), lines.len(), wrong.join("\n"));
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    // the verbs are named
    let (_, _, err) = wisp(&dir, port, &["computer", "start"].map(str::to_owned));
    assert!(err.contains("wisp computer has no start: on, off, status"), "{err}");

    // a built-in group's verb with words it doesn't take (its own arm refused them): its usage, never a task
    for l in
        [&["sdk", "types"][..], &["sdk", "types", "rust"], &["completions", "zsh", "extra"], &["crew", "ls", "extra"], &["grid", "deploy"]]
    {
        let (code, out, err) = wisp(&dir, port, &l.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>());
        assert!(code == 2 && !out.contains("launched") && err.contains(&format!("wisp {}", l[0])), "{l:?} → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    // a plugin's verb with a flag it doesn't take: still refused (a plugin's name isn't a prompt when its verb follows)
    let (code, out, err) = wisp(&dir, port, &["channels", "refresh", "--bad"].map(str::to_owned));
    assert!(code != 0 && !out.contains("launched"), "wisp channels refresh --bad → {code}: {out}{err}");

    // prompts (the fake harness: nothing paid): a group's word quoted as one argument, or after `--`; and a plugin's
    // name followed by none of its verbs, plain (review, ship, update, git and learn are all plugins here)
    let catalogue = api.cmd("cli.catalogue", json!({})).await.to_string();
    let prompts: [&[&str]; 7] = [
        &["computer start fails on boot, fix it"],
        &["--", "grid", "start", "is", "slow"],
        &["review", "the", "auth", "module"],
        &["ship", "the", "docs"],
        &["update", "the", "readme"],
        &["git", "rebase", "onto", "main"],
        &["learn", "rust", "properly"],
    ];
    for words in prompts {
        if let Some(ns) = words.first().filter(|w| **w != "--" && !w.contains(' ')) {
            assert!(catalogue.contains(&format!("\"{ns}.")), "{ns} is a plugin here, as the test means");
        }
        let args: Vec<String> = std::iter::once("--no-open").chain(words.iter().copied()).map(str::to_owned).collect();
        let (code, out, err) = wisp(&dir, port, &args);
        assert!(code == 0 && format!("{out}{err}").contains("launched"), "{words:?} → {code}: {out}{err}");
    }
    // a flag no launch takes after a prompt (audit H2): refused, nothing launched, and the way to say it is `--`
    let before = api.cmd("runtime.list", json!({})).await.as_array().unwrap().len();
    let strict = ["--no-open", "review", "the", "auth", "module", "--strict"].map(str::to_owned);
    let (code, out, err) = wisp(&dir, port, &strict);
    assert!(code == 1 && err.contains("wisp takes no --strict") && !out.contains("launched"), "{strict:?} → {code}: {out}{err}");
    assert!(err.contains("A task that says it: wisp \"review the auth module\" -- --strict"), "{err}");
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), before, "nothing was launched");
    let said = ["--no-open", "review", "the", "auth", "module", "--", "--strict"].map(str::to_owned);
    let (code, out, err) = wisp(&dir, port, &said);
    assert!(code == 0 && format!("{out}{err}").contains("launched"), "{said:?} → {code}: {out}{err}");
    let tasks = api.cmd("runtime.list", json!({})).await;
    let made = tasks.to_string();
    assert!(made.contains("review the auth module --strict"), "after -- the flag is the prompt's: {tasks}");
    for p in [
        "computer start fails on boot, fix it",
        "grid start is slow",
        "review the auth module",
        "ship the docs",
        "update the readme",
        "git rebase onto main",
        "learn rust properly",
    ] {
        assert!(made.contains(p), "{p}: {tasks}");
    }
    drop(serve);
}

/// The quoted words of a `const NAME… = [ … ];` list in a source file.
fn listed(src: &str, name: &str) -> Vec<String> {
    let after = src.split(name).nth(1).unwrap();
    let body = after[after.find('=').unwrap()..].split_once('[').unwrap().1.split("];").next().unwrap();
    body.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

/// `wisp <args>` with this test's home as HOME too: a command that reads harness sessions (adopt) reads none of yours.
fn probe(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// QA round 11 #1: `wisp version`, `wisp update now`, `wisp machine add` and `wisp share list` started tasks. Every
/// word a user may type as a command (v2's COMMANDS, routed words and groups, and v1's: bin/wisp.js's commands and
/// `usage`, `spell`, `version`, `update`, `machine`) followed by words it doesn't take: never a task, unless it's one
/// of `words::PROMPT_WORDS` (a command needing an argument that isn't there: `wisp ship the docs`), which always is.
#[tokio::test(flavor = "multi_thread")]
async fn every_command_word_is_a_command_a_strict_group_or_a_prompt_and_says_so() {
    let dir = temp("serve-cli-words");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let src = include_str!("../src/words.rs");
    let prompt_words = listed(src, "pub const PROMPT_WORDS");
    let v1 = [
        "add", "ls", "list", "show", "wait", "reply", "btw", "stop", "mcp", "skill", "ship", "merge", "run", "recipes", "usage", "spell",
        "spells", "crew", "grid", "computer", "version", "update", "machine", "help",
    ];
    let mut all: Vec<String> = listed(include_str!("../src/args.rs"), "pub const COMMANDS");
    all.extend(listed(src, "pub const ROUTED"));
    all.extend(built_in_groups());
    all.extend(v1.iter().map(|w| (*w).to_owned()));
    all.sort();
    all.dedup();
    assert!(all.len() > 70 && prompt_words.len() > 20, "{} words, {} prompt words", all.len(), prompt_words.len());

    // the commands and groups: none of them a task
    let mut wrong = Vec::new();
    for word in all.iter().filter(|w| !prompt_words.contains(w)) {
        let (code, out, err) = probe(&dir, port, &[word, "definitely-not-a-verb"]);
        if out.contains("launched") || err.contains("launched") {
            wrong.push(format!("wisp {word} definitely-not-a-verb → {code}: {}{}", out.trim(), err.trim()));
        }
    }
    assert!(wrong.is_empty(), "{} command words started a task:\n{}", wrong.len(), wrong.join("\n"));
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");

    // what QA typed
    let version = serde_json::to_string(env!("CARGO_PKG_VERSION")).unwrap();
    for args in [&["version"][..], &["--version"], &["-v"]] {
        let (code, out, err) = probe(&dir, port, args);
        assert!(code == 0 && format!("\"{}\"", out.trim()) == version, "{args:?} → {code}: {out}{err}");
    }
    let (code, _, err) = probe(&dir, port, &["update", "now"]);
    assert!(code == 2 && err.contains("wisp update has no now: apply"), "{code}: {err}");
    let (code, _, err) = probe(&dir, port, &["share", "list"]);
    assert!(code == 2 && err.contains("wisp share has no list: <task id>") && err.contains("wisp shares"), "{code}: {err}");
    // `machine` is this machine's plugin, with `machines`' verbs: the same list
    let (code, mine, err) = probe(&dir, port, &["machine", "list", "--json"]);
    let (_, theirs, _) = probe(&dir, port, &["machines", "list", "--json"]);
    assert!(code == 0 && mine == theirs, "{code}: {mine}{err}");
    let (code, out, err) = probe(&dir, port, &["machine", "add"]);
    assert!(code != 0 && !err.contains("has no add") && !out.contains("launched"), "{code}: {out}{err}");
    // `-C machine`: a flag's value named machine is left alone
    std::fs::create_dir_all(dir.join("proj").join("machine")).unwrap();
    let (code, out, err) = probe(&dir, port, &["-C", "machine", "machine", "list", "--json"]);
    assert!(code == 0 && out == theirs, "{code}: {out}{err}");
    // alone: the verbs, as before (exit 0)
    let (code, out, err) = probe(&dir, port, &["machines"]);
    assert!(code == 0 && out.contains("list"), "{code}: {out}{err}");
    // --help on a group is its usage, never the command (`update --help` ran a check)
    for g in ["update", "usage", "version"] {
        let (code, out, err) = probe(&dir, port, &[g, "--help"]);
        assert!(code == 0 && out.contains(&format!("wisp {g}")) && !out.contains("is the latest"), "{g} --help → {code}: {out}{err}");
    }
    let (code, _, err) = probe(&dir, port, &["machine", "frob"]);
    assert!(code == 2 && err.contains("wisp machine has no frob") && err.contains("list"), "{code}: {err}");
    // v1's `wisp usage` and `wisp spell`
    let (code, out, err) = probe(&dir, port, &["usage"]);
    assert!(code == 0 && !out.contains("launched"), "{code}: {out}{err}");
    // (one word after it: a slip; QA round 14's rule makes two or more a prompt)
    let (code, _, err) = probe(&dir, port, &["spell", "cast"]);
    assert!(code == 2 && err.contains("wisp spell has no cast"), "{code}: {err}");
    let (code, _, err) = probe(&dir, port, &["spell"]);
    assert!(code == 2 && err.contains("plugin new"), "{code}: {err}");
    // audit G GS10: `wisp <group> --help` is `wisp help <group>`, for every kind of group
    for g in ["connectors", "plugin", "channels", "crew", "worktrees", "settings", "machines", "cloud", "update"] {
        let (code, out, err) = probe(&dir, port, &[g, "--help"]);
        let (hcode, hout, _) = probe(&dir, port, &["help", g]);
        assert!(code == 0 && hcode == 0 && out == hout, "{g} --help → {code}: {out}{err}\n  help {g} → {hout}");
        assert!(!out.contains("needs more"), "{g}: {out}");
    }
    // QA round 11 #2: help for a built-in group is its usage
    for (group, line) in
        [("computer", "wisp computer on|off|status"), ("update", "wisp update [apply]"), ("machines", "wisp machines|machine")]
    {
        let (code, out, err) = probe(&dir, port, &["help", group]);
        assert!(code == 0 && out.contains(line), "help {group} → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "still no task");

    // the prompt words: a prompt, always (the fake harness: nothing paid)
    for word in &prompt_words {
        let (code, out, err) = probe(&dir, port, &["--no-open", word, "definitely-not-a-verb", "please"]);
        assert!(code == 0 && format!("{out}{err}").contains("launched"), "wisp {word} … → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), prompt_words.len());
    drop(serve);
}

/// QA round 12 #1, #2, #4: fifteen one-word slips (and their aliases) each launched an agent; `wisp run the tests` said
/// its id where the title belongs; `wisp init` in a folder that's no project wrote there without a word.
#[tokio::test(flavor = "multi_thread")]
async fn one_word_slips_say_what_to_type_and_a_run_prompt_launches_with_its_title() {
    let dir = temp("serve-cli-slips");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let lines: [(&[&str], &str); 21] = [
        (&["upgrade"], "wisp update"),
        (&["login"], "wisp cloud signin"),
        (&["logout"], "wisp cloud signout"),
        (&["signin"], "wisp cloud signin"),
        (&["signout"], "wisp cloud signout"),
        (&["whoami"], "wisp cloud status"),
        (&["doctor"], "wisp accounts harnesses"),
        (&["config"], "wisp settings list"),
        (&["projects"], "wisp ls"),
        (&["remove"], "wisp runtime delete <id>"),
        (&["rm", "123"], "wisp runtime delete <id>"),
        (&["delete", "01a1ef07823670f68eb62f4ddb8ededc"], "wisp runtime delete <id>"),
        (&["start"], "wisp serve"),
        (&["restart"], "wisp serve"),
        (&["kill", "123"], "wisp stop <id>"),
        (&["new"], "<what to do>"),
        (&["info"], "wisp version"),
        // any other word alone, or with an id: not a command
        (&["refactor"], "isn't a command"),
        (&["fetch", "123"], "isn't a command"),
        (&["frobnicate", "--x"], "isn't a command"),
        (&["README.md"], "isn't a command"),
    ];
    let mut wrong = Vec::new();
    for (words, said) in lines {
        let (code, out, err) = probe(&dir, port, words);
        if code != 2 || out.contains("launched") || !err.contains(said) || !err.contains("wisp -- ") {
            wrong.push(format!("wisp {} → {code}: {}{}", words.join(" "), out.trim(), err.trim()));
        }
    }
    assert!(wrong.is_empty(), "{} slips:\n{}", wrong.len(), wrong.join("\n"));
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");

    // #4: `wisp init` here (a scratch folder: no git repo, no project, no terminal) refuses and writes nothing
    let (code, _, err) = probe(&dir, port, &["init"]);
    assert!(
        code == 2 && err.contains("--here") && !dir.join("proj/.wisp").exists() && !dir.join("proj/AGENTS.md").exists(),
        "{code}: {err}"
    );
    for words in [&["init", "--force"][..], &["init", "auto", "--force"]] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(
            code == 2 && err.contains("--here") && !out.contains("launched") && !dir.join("proj/.wisp").exists(),
            "{words:?} → {code}: {out}{err}"
        );
    }

    // on purpose, a task: `--` first, or one quoted argument (the fake harness: nothing paid)
    for words in [&["--no-open", "--", "login"][..], &["--no-open", "log me in with the new SSO"]] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(code == 0 && out.contains("launched"), "{words:?} → {code}: {out}{err}");
    }
    // #2: a prompt that starts with `run` says its title, as every prompt does
    let (code, out, err) = probe(&dir, port, &["--no-open", "run", "the", "tests"]);
    assert!(code == 0 && out.contains("launched: run the tests  ("), "{code}: {out}{err}");
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 3);
    drop(serve);
}

/// QA round 13: `wisp add a README` registered a project "a" and lost the prompt (#1); `wisp ls --all` said "No
/// sessions." with tasks there (#2); `wisp top` was "did you mean wisp stop?", and `ps`, `cancel`, `pause` and `agents`
/// pointed nowhere or the wrong way (#3).
#[tokio::test(flavor = "multi_thread")]
async fn add_a_readme_is_a_task_ls_all_lists_every_task_and_slips_point_the_right_way() {
    let dir = temp("serve-cli-add");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    std::fs::create_dir_all(dir.join("proj/web")).unwrap();

    // #1: prompts that start with add are tasks, with their titles, and no project is made
    for prompt in ["add a README", "add tests for math.js", "add dark mode to the settings page"] {
        let words: Vec<&str> = std::iter::once("--no-open").chain(prompt.split(' ')).collect();
        let (code, out, err) = probe(&dir, port, &words);
        assert!(
            code == 0 && out.contains(&format!("launched: {prompt}  (")) && !out.contains("project added"),
            "{prompt} → {code}: {out}{err}"
        );
    }
    // a folder here, or a path, is the command; a path that isn't there is refused, never a phantom project
    let (code, out, err) = probe(&dir, port, &["add", "web"]);
    assert!(code == 0 && out.contains("project added: web"), "{code}: {out}{err}");
    // `-C dir`: the folder there; `~/…` (no shell expanded it): the user's home (HOME is this test's home)
    std::fs::create_dir_all(dir.join("other/api")).unwrap();
    let other = dir.join("other").display().to_string();
    let (code, out, err) = probe(&dir, port, &["-C", &other, "add", "api"]);
    assert!(code == 0 && out.contains("project added: api"), "{code}: {out}{err}");
    std::fs::create_dir_all(dir.join("home/code/site")).unwrap();
    let (code, out, err) = probe(&dir, port, &["add", "~/code/site"]);
    assert!(code == 0 && out.contains("project added: site"), "{code}: {out}{err}");
    let (code, out, err) = probe(&dir, port, &["add", "./nope"]);
    assert!(code != 0 && err.contains("no folder") && !out.contains("project added") && !out.contains("launched"), "{code}: {out}{err}");

    // #2: --all is the open tasks and the archived ones
    let tasks = api.cmd("runtime.list", json!({})).await;
    let ids: Vec<String> = tasks.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_owned()).collect();
    assert_eq!(ids.len(), 3, "{tasks}");
    api.cmd("runtime.archive", json!({ "id": ids[0], "archived": true })).await;
    let (_, open, _) = probe(&dir, port, &["ls"]);
    let (code, all, err) = probe(&dir, port, &["ls", "--all"]);
    assert!(code == 0 && !all.contains("No tasks."), "{code}: {all}{err}");
    assert!(!open.contains(&ids[0]) && ids.iter().all(|id| all.contains(id.as_str())), "ls:\n{open}\nls --all:\n{all}");

    // #3: each slip points the right way
    for (words, said) in [
        (&["agents"][..], "wisp accounts harnesses"),
        (&["top"], "wisp ls --watch"),
        (&["ps"], "did you mean wisp ls"),
        (&["cancel", "123"], "wisp stop <id>"),
        (&["pause"], "wisp stop <id>"),
    ] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(code == 2 && err.contains(said) && !out.contains("launched"), "{words:?} → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({ "archived": true })).await.as_array().unwrap().len(), 1);
    drop(serve);
}

/// `probe`, but it must end within `secs` (QA round 14 #7: `wisp attach last` hung until killed).
fn probe_within(dir: &Path, port: u16, args: &[&str], secs: u64) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let at = std::time::Instant::now();
    while child.try_wait().unwrap().is_none() {
        if at.elapsed() > std::time::Duration::from_secs(secs) {
            let _ = child.kill();
            panic!("wisp {} still running after {secs} s", args.join(" "));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let o = child.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// QA round 14 (the owner's delegate's rule): after a command word, one word it doesn't take is a slip (exit 2, with
/// the `wisp -- …` way out), and two or more are a prompt; a command that takes a task is one only for an id or
/// `last`. #1: four prompts were refused; #2: `wisp settings open` and `wisp status all` launched tasks; #7: `wisp
/// attach last` hung.
#[tokio::test(flavor = "multi_thread")]
async fn after_a_command_word_one_word_is_a_slip_and_more_are_a_prompt() {
    let dir = temp("serve-cli-r14");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // #7, no task yet: said at once
    let (code, _, err) = probe_within(&dir, port, &["attach", "last"], 20);
    assert!(code != 0 && err.contains("no tasks yet"), "{code}: {err}");

    // #2 and the one-word slips: refused, the way out shown, no task
    for words in [
        &["update", "deps"][..],
        &["settings", "open"],
        &["status", "all"],
        &["computer", "start"],
        &["stop", "it"],
        &["show", "foo"],
        &["wait", "x"],
        &["channels", "refresh", "--bad"],
        // a group that routes as a command, one word: a slip (two or more are a prompt, QA round 15)
        &["mcp", "foo"],
        &["skill", "foo"],
    ] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(code == 2 && !out.contains("launched") && err.contains("wisp -- "), "{words:?} → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");

    // #1 and QA's six that ran: prompts, launched with their titles (the fake harness: nothing paid)
    let prompts = [
        "show me where the bug is",
        "stop using var in math.js",
        "share the results with the team",
        "add logging to math.js",
        "open a PR for the math fix",
        "settings page needs a dark mode",
        "status badge in the README",
        "merge the two helper functions",
        "attach a screenshot of the failing test",
    ];
    for prompt in prompts {
        let words: Vec<&str> = std::iter::once("--no-open").chain(prompt.split(' ')).collect();
        let (code, out, err) = probe(&dir, port, &words);
        assert!(code == 0 && out.contains(&format!("launched: {prompt}  (")), "{prompt} → {code}: {out}{err}");
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), prompts.len());

    // #7: `last` is the newest task, at once
    let (code, out, err) = probe_within(&dir, port, &["show", "last"], 20);
    assert!(code == 0 && out.contains("attach a screenshot of the failing test"), "{code}: {out}{err}");
    let (code, out, err) = probe_within(&dir, port, &["attach", "last"], 30);
    assert!(code == 0, "{code}: {out}{err}");
    drop(serve);
}

/// QA round 15: `wisp plugin for prettier` was refused (#1, two or more words after a command word are a prompt);
/// `wisp export foo` launched a task (#2, one stray word is a slip); `wisp ls -a` said "-a needs a value", `wisp fork`
/// pointed nowhere, and `wisp help me` said "no plugin named me" (#6).
#[tokio::test(flavor = "multi_thread")]
async fn one_stray_word_is_a_slip_more_is_a_prompt_for_every_command_word() {
    let dir = temp("serve-cli-r15");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    for (words, said) in [
        // the kinds there are (audit C5), then the way out
        (&["export", "foo"][..], "no kind foo"),
        (&["run", "foo"], "no declared task, recipe or workflow named foo"),
        // a plugin's group line (B1): its verbs, then `wisp -- "ship foo"`
        (&["ship", "foo"], "wisp ship has no foo"),
        (&["plugin", "foo"], "wisp plugin has no foo"),
        (&["mcp", "foo"], "wisp mcp has no foo"),
        (&["init", "foo"], "auto, ask or manual"),
        (&["fork"], "wisp runtime fork --id <id>"),
        (&["help", "me"], "wisp help lists every command and plugin"),
        // audit B #15: help never starts a task, however many words follow; the task is `wisp -- help …`
        (&["help", "me", "write", "a", "test", "for", "add()"], "wisp -- help me write a test for add()"),
    ] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(code == 2 && !out.contains("launched") && err.contains(said) && err.contains("wisp -- "), "{words:?} → {code}: {out}{err}");
    }
    assert!(!dir.join("proj/.wisp").exists(), "`init foo` made nothing");
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");

    // prompts (the fake harness: nothing paid)
    let prompts = [
        "plugin for prettier",
        "plugin to format the code",
        "mcp server for the docs",
        "fork the repo and set up CI",
        "export the results as CSV",
        "run the failing tests twice",
    ];
    // one quoted argument, or `--`, after a prompt word: the prompt
    for words in [&["--no-open", "ship", "the docs"][..], &["--no-open", "run", "the tests"], &["--no-open", "run", "--", "deploy"]] {
        let (code, out, err) = probe(&dir, port, words);
        assert!(code == 0 && out.contains("launched"), "{words:?} → {code}: {out}{err}");
    }
    for prompt in prompts {
        let words: Vec<&str> = std::iter::once("--no-open").chain(prompt.split(' ')).collect();
        let (code, out, err) = probe(&dir, port, &words);
        assert!(code == 0 && out.contains(&format!("launched: {prompt}  (")), "{prompt} → {code}: {out}{err}");
    }
    // #6: `ls -a` is --all
    let ids: Vec<String> =
        api.cmd("runtime.list", json!({})).await.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_owned()).collect();
    assert_eq!(ids.len(), prompts.len() + 3);
    api.cmd("runtime.archive", json!({ "id": ids[0], "archived": true })).await;
    let (code, out, err) = probe(&dir, port, &["ls", "-a"]);
    assert!(code == 0 && ids.iter().all(|id| out.contains(id.as_str())), "{code}: {out}{err}");
    // with a filter too (`-a` was read as --agent and swallowed `--state`)
    let (code, out, err) = probe(&dir, port, &["ls", "-a", "--state", "running"]);
    let applied = out.trim() == "No tasks." || out.lines().filter(|l| !l.trim().is_empty()).all(|l| l.contains(" running "));
    assert!(code == 0 && applied, "only running tasks, or none: {code}: {out}{err}");
    drop(serve);
}

/// Setup audit F30 (H): from a script, an agent or CI (no terminal), a mistyped verb launched a real Full-auto task.
/// The slip rules hold with no terminal: every line here is refused (exit 2, the `wisp -- …` way out), and no task.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_terminal_a_mistyped_verb_is_refused_never_launched() {
    let dir = temp("serve-cli-notty");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let mut wrong = Vec::new();
    for words in [
        &["budget", "stop-at"][..],
        &["crew", "frobnicate"],
        &["settings", "frobnicate"],
        &["plugin", "frobnicate"],
        &["cloud", "frobnicate"],
        &["crew", "frobnicate", "--json"],
        &["--wait", "crew", "frobnicate"],
        &["-d", "settings", "frobnicate"],
    ] {
        let (code, out, err) = probe(&dir, port, words);
        if code != 2 || out.contains("launched") || err.contains("launched") || !err.contains("wisp -- ") {
            wrong.push(format!("wisp {} → {code}: {}{}", words.join(" "), out.trim(), err.trim()));
        }
    }
    assert!(wrong.is_empty(), "{} launched or weren't refused:\n{}", wrong.len(), wrong.join("\n"));
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    drop(serve);
}

/// Setup audit B1 (F30's rest): `wisp budget frobnicate`, `wisp git frobnicate`, … under 21 plugin groups launched a
/// Full-auto task in the current folder. Every plugin the catalogue names, followed by one word that's none of its
/// verbs: refused (exit 2, its verbs, the `wisp -- …` way out), with no terminal, and no task made.
#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_name_and_a_word_that_is_none_of_its_verbs_never_launches() {
    let dir = temp("serve-cli-plugin-slip");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let catalogue = api.cmd("cli.catalogue", json!({})).await;
    let mut plugins: Vec<String> = catalogue
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["id"].as_str().and_then(|id| id.split_once('.')).map(|(p, _)| kebab(p)))
        .filter(|p| !wisp_words_known(p))
        .collect();
    plugins.sort();
    plugins.dedup();
    for g in ["budget", "git", "connectors", "channels", "runtime"] {
        assert!(plugins.contains(&g.to_owned()), "{g} is a plugin group here: {plugins:?}");
    }
    let mut wrong = Vec::new();
    for g in &plugins {
        let (code, out, err) = probe(&dir, port, &[g.as_str(), "frobnicate"]);
        if code != 2 || out.contains("launched") || !err.contains(&format!("wisp {g} has no frobnicate")) || !err.contains("wisp -- ") {
            wrong.push(format!("wisp {g} frobnicate → {code}: {}{}", out.trim(), err.trim()));
        }
    }
    assert!(wrong.is_empty(), "{} of {} plugin groups launched or weren't refused:\n{}", wrong.len(), plugins.len(), wrong.join("\n"));
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    drop(serve);
}

/// `gridClient` → `grid-client`, as the CLI names a plugin.
fn kebab(w: &str) -> String {
    w.chars().flat_map(|c| if c.is_uppercase() { vec!['-', c.to_ascii_lowercase()] } else { vec![c] }).collect()
}

/// A built-in word (a command, a routed word or a group): its own rules, not a plugin's.
fn wisp_words_known(w: &str) -> bool {
    let (words, args) = (include_str!("../src/words.rs"), include_str!("../src/args.rs"));
    listed(words, "pub const ROUTED").iter().chain(listed(args, "pub const COMMANDS").iter()).any(|x| x == w)
        || built_in_groups().iter().any(|x| x == w)
}

/// Auto off (the default, `runtime.auto`, audit C6): `wisp router` and any words after it is refused like a slip
/// (exit 2, the `wisp -- "…"` way out), never a prompt: the router's former command words must not launch a paid run.
#[tokio::test(flavor = "multi_thread")]
async fn with_auto_off_router_and_any_words_is_refused_and_makes_no_task() {
    let dir = temp("serve-cli-router-away");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    for words in
        [&["router", "preview"][..], &["router", "pick", "the", "best", "harness"], &["--no-open", "router", "explain", "this", "task"]]
    {
        let (code, out, err) = probe(&dir, port, words);
        let typed: Vec<&str> = words.iter().copied().filter(|w| !w.starts_with("--")).collect();
        assert!(
            code == 2 && !out.contains("launched") && err.contains(&format!("wisp -- \"{}\"", typed.join(" "))),
            "{words:?} → {code}: {out}{err}"
        );
    }
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 0, "no task was made");
    drop(serve);
}
