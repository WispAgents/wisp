use super::*;

fn p(args: &[&str]) -> Parsed {
    let v: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    parse(&v, None, Path::new("/home/ada"), Path::new("/work")).unwrap()
}

#[test]
fn tasks_and_commands() {
    assert_eq!(p(&["fix", "the", "tests"]).command, None);
    assert_eq!(p(&["ls"]).command, Some("ls"));
    assert_eq!(p(&["serve"]).command, Some("serve"));
    // a command name that doesn't fit is the first word of a task
    assert_eq!(p(&["ship", "the", "docs"]).command, None);
    assert_eq!(p(&["ship", "0123456789ab"]).command, Some("ship"));
    assert_eq!(p(&["ship", "0190f3c2-5b1e-7c4a-9d2e-0123456789ab"]).command, Some("ship"));
    assert_eq!(p(&["usage", "of", "memory"]).command, None);
    assert_eq!(p(&["run"]).command, None);
    assert_eq!(p(&["run", "deploy"]).command, Some("run"));
    assert_eq!(p(&["grid", "serve"]).command, Some("grid"));
    assert_eq!(p(&["grid", "the", "page"]).command, None);
    assert_eq!(p(&["crew", "export", "ada"]).command, None, "a crew is shared as a formula now");
    assert_eq!(p(&["crew"]).command, Some("crew"));
    assert_eq!(p(&["plugin", "new", "x"]).command, Some("plugin"));
    // help never starts a task (audit B #15: a quoted verb as `wisp help runtime` lists it launched one)
    assert_eq!(p(&["help", "runtime", "queue order"]).command, Some("help"));
    assert_eq!(p(&["help", "me", "fix", "the", "login", "page"]).command, Some("help"));
    assert_eq!(p(&["--", "help", "me"]).command, None);
    // `--` first: always a task
    assert_eq!(p(&["--", "ls", "the", "repo"]).command, None);
    assert_eq!(p(&["--", "ls", "the", "repo"]).words, ["ls", "the", "repo"]);
}

#[test]
fn every_grid_command_routes_to_the_grid() {
    for sub in GRID_SUB {
        assert_eq!(p(&["grid", sub]).command, Some("grid"), "{sub}");
    }
}

#[test]
fn flags() {
    let x = p(&["--state=review", "ls", "--port", "5000", "-C", "~/code", "--also", "lib", "--compare", "claude,codex:gpt-5", "--no-kit"]);
    assert_eq!(x.flags.state.as_deref(), Some("review"));
    assert_eq!((x.flags.port, x.flags.port_given), (5000, true));
    assert_eq!(x.flags.cwd, Some(PathBuf::from("/home/ada/code")));
    assert_eq!(x.flags.also, vec![PathBuf::from("/work/lib")]);
    assert_eq!(x.flags.compare.as_ref().map(Vec::len), Some(2));
    assert_eq!(x.flags.compare.as_ref().unwrap()[1], CompareRun { agent: "codex".into(), model: Some("gpt-5".into()) });
    assert!(!x.flags.kit);
    assert_eq!(x.command, Some("ls"));
    assert_eq!(p(&["--version"]).action, Action::Version);
    assert_eq!(p(&["-h"]).action, Action::Help);
}

#[test]
fn a_flag_without_its_value_is_a_usage_error() {
    let v = vec!["--port".to_owned()];
    assert_eq!(parse(&v, None, Path::new("/"), Path::new("/")).unwrap_err(), UsageError("--port needs a value — see wisp --help".into()));
    let v = vec!["--port".to_owned(), "nope".to_owned()];
    assert!(parse(&v, None, Path::new("/"), Path::new("/")).is_err());
}

#[test]
fn env_port_and_literal_after_words() {
    let v: Vec<String> = ["launch", "--", "--weird"].iter().map(|s| (*s).to_owned()).collect();
    let x = parse(&v, Some("4500"), Path::new("/"), Path::new("/")).unwrap();
    assert_eq!(x.flags.port, 4500);
    assert!(!x.flags.port_given);
    assert_eq!(x.words, ["launch", "--weird"]);
    assert_eq!(x.literal, Literal::Yes);
}

#[test]
fn session_ids() {
    assert!(session_id("0123456789ab"));
    assert!(session_id("0190f3c25b1e7c4a9d2e0123456789ab"));
    assert!(!session_id("0123456789AB"));
    assert!(!session_id("the"));
}

/// QA round 13 #1: `wisp add a README` registered a project "a" for a folder that doesn't exist and lost the prompt.
/// `add` is the command only alone or with one argument that is a folder here or reads as a path or a link; else the
/// words are a task.
#[test]
fn add_is_a_command_only_for_a_folder_a_path_or_a_link() {
    let here = std::env::temp_dir().join(format!("wisp-add-{}", std::process::id()));
    std::fs::create_dir_all(here.join("api")).unwrap();
    let at = |args: &[&str]| {
        let v: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        parse(&v, None, Path::new("/home/ada"), &here).unwrap().command
    };
    for args in [
        &["add"][..],
        &["add", "."],
        &["add", "api"],
        &["add", "./web"],
        &["add", "../other"],
        &["add", "~/code/site"],
        &["add", "/srv/app"],
        &["add", "code/site"],
        &["add", "https://github.com/acme/api"],
        &["add", "git@github.com:acme/api.git"],
    ] {
        assert_eq!(at(args), Some("add"), "{args:?}");
    }
    for args in [
        &["add", "a", "README"][..],
        &["add", "tests", "for", "math.js"],
        &["add", "dark", "mode"],
        &["add", "tests"],
        &["add", "api", "docs"],
    ] {
        assert_eq!(at(args), None, "{args:?} is a task");
    }
    // `-C dir`: the folder is looked for there
    assert_eq!(at(&["-C", here.to_str().unwrap(), "add", "api"]), Some("add"));
    let _ = std::fs::remove_dir_all(&here);
}

/// QA round 14 #1, #7: `wisp show me where the bug is` and `wisp stop using var in math.js` were refused ("no task
/// me"), and `wisp attach last` hung. A command that takes a task is that command only when its argument is a task's
/// id (or `last`, the newest); otherwise the words are a prompt.
#[test]
fn a_task_command_is_a_command_only_for_a_task_id_or_last() {
    let id = "01a1ef07823670f68eb62f4ddb8ededc";
    for args in [
        &["show", id][..],
        &["show", "last"],
        &["stop", "0123456789ab"],
        &["stop", "last"],
        &["wait", id],
        &["attach", "last"],
        &["reply", id, "also cover the empty list"],
        &["btw", "last", "did you touch the migrations?"],
        &["stop", "123"],
    ] {
        assert_eq!(p(args).command, Some(args[0]), "{args:?}");
    }
    for args in [
        &["show", "me", "where", "the", "bug", "is"][..],
        &["stop", "using", "var", "in", "math.js"],
        &["wait", "for", "the", "build"],
        &["attach", "a", "screenshot", "of", "the", "failing", "test"],
        &["reply", "to", "the", "review", "comments"],
        &["btw", "also", "fix", "the", "tests"],
        &["show", "foo"],
    ] {
        assert_eq!(p(args).command, None, "{args:?}");
    }
}

/// QA round 15 #1, #6: `wisp plugin for prettier` was refused (two or more words after a command word are a prompt),
/// and `wisp ls -a` said "-a needs a value" (it's `--all` there).
#[test]
fn a_group_that_routes_as_a_command_takes_its_verb_or_one_word_and_ls_dash_a_is_all() {
    for args in [
        &["plugin"][..],
        &["plugin", "foo"],
        &["plugin", "install", "./x"],
        &["plugins", "ls"],
        &["mcp"],
        &["mcp", "--repo", "x"],
        &["skill", "install"],
    ] {
        assert_eq!(p(args).command, Some(args[0]), "{args:?}");
    }
    for args in [
        &["plugin", "for", "prettier"][..],
        &["plugin", "to", "format", "the", "code"],
        &["mcp", "server", "for", "the", "docs"],
        &["skill", "for", "writing", "tests"],
    ] {
        // not the command: the launch path takes it, and refuses it as a slip (audit I2; `words::unknown_verb`)
        assert_eq!(p(args).command, None, "{args:?} isn't the command");
    }
    let x = p(&["ls", "-a"]);
    assert!(x.command == Some("ls") && x.flags.all && x.flags.agent.is_none(), "{x:?}");
    let x = p(&["fix", "it", "-a", "codex"]);
    assert_eq!(x.flags.agent.as_deref(), Some("codex"), "-a is --agent elsewhere");
}

/// Audit B #14: `--agent claude --compare codex` launched only codex; v1 ran the agent and each compare run.
#[test]
fn agent_and_compare_both_run() {
    let names = |a: &[&str]| compare_runs(&p(a).flags).into_iter().map(|r| (r.agent, r.model)).collect::<Vec<_>>();
    let run = |a: &str, m: Option<&str>| (a.to_owned(), m.map(str::to_owned));
    assert_eq!(names(&["x", "--agent", "claude", "--compare", "codex"]), [run("claude", None), run("codex", None)]);
    assert_eq!(
        names(&["x", "--agent", "claude", "--model", "opus", "--compare", "codex:gpt-5"]),
        [run("claude", Some("opus")), run("codex", Some("gpt-5"))]
    );
    assert_eq!(
        names(&["x", "--compare", "claude,codex:gpt-5"]),
        [run("claude", None), run("codex", Some("gpt-5"))],
        "the list alone is the runs"
    );
    assert_eq!(names(&["x", "--agent", "claude", "--compare", "claude,codex"]), [run("claude", None), run("codex", None)], "not twice");
}

/// Audit B #9: `wisp crew create --on done` was read as the machine to run on ("there's no machine done").
#[test]
fn a_crew_verbs_own_on_is_not_the_machine() {
    let x = p(&["crew", "create", "--name", "Bob", "--on", "done"]);
    assert_eq!(x.flags.on, None);
    assert_eq!(x.argv[x.argv.len() - 2..], ["--on", "done"], "left for the verb to read");
    assert_eq!(
        p(&["--on", "laptop", "crew", "create", "--name", "Bob"]).flags.on.as_deref(),
        Some("laptop"),
        "before the words: the machine"
    );
    assert_eq!(p(&["ls", "--on", "laptop"]).flags.on.as_deref(), Some("laptop"));
    assert_eq!(p(&["runtime", "list", "--on", "laptop"]).flags.on.as_deref(), Some("laptop"));
}

/// Owner-list row 64: the terminal context (ADR-0023) was built but `wisp --help` never said it was there: its flags,
/// what it attaches and `wisp shell-init` are in the usage, and each flag the usage names parses.
#[test]
fn the_usage_names_the_terminal_context_and_its_flags_parse() {
    for w in ["--context", "--no-context", "--show-context", "--lines N", "wisp shell-init zsh|bash|fish", "check the error above"] {
        assert!(HELP.contains(w), "{w}");
    }
    let shown = p(&["check", "the", "error", "above", "--show-context", "--lines", "50"]);
    assert_eq!((shown.flags.context, shown.flags.show_context, shown.flags.lines), (Some(true), true, Some(50)));
    assert_eq!(p(&["what", "failed", "--no-context"]).flags.context, Some(false));
}

#[test]
fn ship_takes_the_steward_mode() {
    let a = p(&["ship", "0123456789ab", "--steward", "review"]);
    assert_eq!((a.command, a.flags.steward.as_deref()), (Some("ship"), Some("review")));
    assert_eq!(p(&["ship", "0123456789ab"]).flags.steward, None);
}

/// Audit cli-r3: `wisp ls --stat done`, `--bogus`, `--limit abc`, `--limit 0` and `--days x` ran and showed the default
/// list as if nothing was typed. A bad count is a usage error naming it; a flag ls doesn't read is refused with its
/// usage (as `wisp mcp --x`), `--host` (serve's alone) included; `--` still makes anything words.
#[test]
fn a_scripted_command_refuses_a_flag_it_does_not_read_and_a_bad_count() {
    let err = |a: &[&str]| parse(&a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(), None, Path::new("/"), Path::new("/")).unwrap_err();
    assert_eq!(err(&["ls", "--limit", "abc"]).to_string(), "--limit abc: a whole number above 0");
    assert_eq!(err(&["ls", "--limit=0"]).to_string(), "--limit 0: a whole number above 0");
    assert_eq!(err(&["ls", "--days", "x"]).to_string(), "--days x: a whole number above 0");
    let refused = |a: &[&str]| crate::commands::refused_flag(&p(a), p(a).command.unwrap());
    for (a, bad) in [
        (&["ls", "--stat", "done"][..], "--stat"),
        (&["ls", "--bogus"], "--bogus"),
        (&["ls", "--all", "--bogus"], "--bogus"),
        (&["ls", "--host", "0.0.0.0"], "--host"),
    ] {
        let said = refused(a).unwrap_or_else(|| panic!("{a:?} ran"));
        assert!(said.starts_with(&format!("  ✗ wisp ls doesn't take {bad}\n")), "{said}");
    }
    let ok = p(&["ls", "--state", "done", "--limit", "5"]);
    assert_eq!((ok.flags.state.as_deref(), ok.flags.limit), (Some("done"), Some(5)));
    for a in [
        &["ls", "--state", "done", "--limit", "5"][..],
        &["ls", "-a", "--json", "--port", "4500"],
        &["ls", "--repo", "--json"],
        &["ship", "last", "--draft"],
        &["wait", "abc123abc123", "--timeout", "10m"],
        &["reply", "abc123abc123", "--", "use --force"],
        &["reply", "abc123abc123", "ok", "--", "--force"],
    ] {
        assert_eq!(refused(a), None, "{a:?}");
    }
    assert_eq!(p(&["--", "ls", "--bogus"]).command, None, "after --: a task");
}

/// cli-r6: `reply --wait --timeout abc` sent the reply and waited no time at all; its --timeout is checked as wait's is.
#[test]
fn reply_with_wait_validates_its_timeout() {
    use crate::commands::validates_timeout;
    assert!(validates_timeout("wait", false));
    assert!(validates_timeout("launch", true) && !validates_timeout("launch", false));
    assert!(validates_timeout("reply", true), "reply --wait reads --timeout");
    assert!(!validates_timeout("reply", false), "reply alone waits on nothing");
    assert!(!validates_timeout("show", true));
    let r = p(&["reply", "abc123abc123", "ok", "--wait", "--timeout", "abc"]);
    assert_eq!((r.command, r.flags.wait, r.flags.timeout.as_deref()), (Some("reply"), true, Some("abc")));
    assert!(crate::scripted::wait_timeout(r.flags.timeout.as_deref()).is_err());
}

/// cli-r6 M2: the watch arms ran before the stray-flag check: `wisp ls --watch --bogus` and `wisp show --watch x --stat`
/// followed on as if the flag weren't there.
#[test]
fn a_watch_refuses_a_flag_it_does_not_read() {
    let refused = |a: &[&str]| crate::commands::refused_flag(&p(a), p(a).command.unwrap());
    let ls = refused(&["ls", "--watch", "--bogus"]).expect("ls --watch --bogus ran");
    assert!(ls.starts_with("  ✗ wisp ls doesn't take --bogus\n"), "{ls}");
    let show = refused(&["show", "--watch", "abc123abc123", "--stat"]).expect("show --watch --stat ran");
    assert!(show.starts_with("  ✗ wisp show doesn't take --stat\n"), "{show}");
    assert_eq!(refused(&["ls", "--watch", "--state", "running", "--agent", "codex"]), None);
}
