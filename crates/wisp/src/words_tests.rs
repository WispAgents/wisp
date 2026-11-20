use super::*;
use crate::args::parse;
use std::path::Path;

fn p(args: &[&str]) -> crate::args::Parsed {
    let v: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    parse(&v, None, Path::new("/home/ada"), Path::new("/work")).unwrap()
}

fn w(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

/// Round 255: `wisp tasks` (no flags) launched an agent with the prompt "tasks". A command word alone,
/// or with a flag the parser doesn't know, is never a task; v1 had the same trap for `wisp ship`,
/// `wisp merge`, `wisp btw` and `wisp run` without their argument.
#[test]
fn a_known_command_word_alone_is_never_a_task() {
    for word in ["tasks", "ship", "merge", "btw", "run", "attach", "plan", "share", "cloud", "grid"] {
        let x = p(&[word]);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Usage(word.to_owned())), "{word}");
    }
    let x = p(&["tasks", "--touchng", "src/a.rs"]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Usage("tasks".to_owned())), "an unknown flag isn't a prompt");
    assert!(usage("tasks").contains("wisp tasks --touching <path>"), "{}", usage("tasks"));
    assert!(usage("ship").contains("wisp ship <id>"), "{}", usage("ship"));
    assert!(usage("validate").contains("wisp diff|validate -f F"), "{}", usage("validate"));
    assert!(usage("tui").contains("wisp -- tui"), "a word without a help line still says how to make it a task");
    assert!(usage("list").contains("wisp ls") && usage("list").contains("wisp -- list"), "{}", usage("list"));
    assert!(usage("plugins").contains("wisp plugin new") && usage("plugins").contains("wisp -- plugins"), "{}", usage("plugins"));
}

#[test]
fn a_word_one_or_two_letters_off_a_command_asks_did_you_mean() {
    for (typo, cmd) in [("tsks", "tasks"), ("taks", "tasks"), ("sl", "ls"), ("shwo", "show"), ("recipies", "recipes"), ("Tasks", "tasks")] {
        let x = p(&[typo]);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Near(cmd.to_owned())), "{typo}");
    }
    // a plugin from the catalogue: its verbs, and a typo of it
    let plugins = w(&["runtime", "settings"]);
    assert_eq!(not_a_task(&w(&["runtime"]), Literal::No, &plugins), Some(NotATask::Plugin("runtime".to_owned())));
    assert_eq!(not_a_task(&w(&["setings"]), Literal::No, &plugins), Some(NotATask::Near("settings".to_owned())));
}

#[test]
fn a_real_prompt_is_still_a_task() {
    for words in [
        &["fix", "the", "add", "bug"][..],
        &["fix the add bug"],
        &["tasks", "are", "slow", "to", "list"],
        &["ship", "the", "docs"],
        &["help me"],
        &["run", "the", "tests"],
        &["enable", "2fa", "on", "login"],
        &["plan", "the", "migration", "-", "postgres", "first"],
        &["tasks", "are", "slow", "--", "fix", "them"],
    ] {
        let x = p(words);
        assert_eq!(not_a_task(&x.words, x.literal, &w(&["runtime"])), None, "{words:?}");
    }
    // `--` first: whatever follows is the task, a command word too (`wisp -- tasks`)
    let x = p(&["--", "tasks"]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), None);
    assert_eq!(x.words, ["tasks"]);
}

/// TRY-7 audit: `wisp get <id>` launched an agent with the prompt "get 01a1e9…". Another tool's word for one of wisp's
/// says which (`get` is `show`), and an unknown word followed by task ids is never a task.
#[test]
fn another_tools_word_or_an_unknown_one_with_a_task_id_is_never_a_task() {
    let id = "01a1e9e05eb676c799c41712ad5656b7";
    for (theirs, ours) in [("get", "show"), ("view", "show"), ("cat", "show"), ("status", "show"), ("logs", "attach")] {
        let x = p(&[theirs, id]);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Near(ours.to_owned())), "{theirs}");
    }
    let x = p(&["fetch", id]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Unknown("fetch".to_owned())));
    let x = p(&["fetch", "local:01a1e9e05eb6"]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Unknown("fetch".to_owned())), "a peer-keyed id too");
    // still tasks: a prompt that only mentions an id among other words, and a word followed by a non-id
    for words in [&["fix", "the", "bug", "in", id][..], &["fetch", "the", "logs"], &["review", "abc123"]] {
        let x = p(words);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), None, "{words:?}");
    }
}

/// The table is main.rs's own: every first word one of its `match first` arms routes on is known.
#[test]
fn every_word_main_routes_on_is_a_known_word() {
    let mut n = 0;
    for line in include_str!("main.rs").lines() {
        // a match arm: `"a" | "b" if …`, `"a" => …`, or the words alone on their line
        let mut rest = line.trim_start();
        let mut arm = Vec::new();
        while let Some(after) = rest.strip_prefix('"') {
            let Some((word, tail)) = after.split_once('"') else { break };
            arm.push(word);
            rest = tail.strip_prefix(" | ").unwrap_or(tail);
        }
        let is_arm = !arm.is_empty()
            && arm.iter().all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'))
            && (rest.is_empty() || rest.starts_with(" if") || rest.starts_with(" =>"));
        for word in arm.iter().filter(|_| is_arm) {
            assert!(known(word), "main.rs routes on {word:?}: add it to words::ROUTED");
            n += 1;
        }
    }
    assert!(n > 30, "read {n} words from main.rs");
    for c in COMMANDS {
        assert!(known(c));
    }
    // and the other way: a word left in ROUTED after its arm went would still print usage
    for r in ROUTED {
        assert!(include_str!("main.rs").contains(&format!("\"{r}\"")), "{r} is in ROUTED but main.rs no longer routes on it");
    }
}

/// QA round 9 #1: `wisp computer on --grid wisp.localhost:4797` (what the Grid's Add a computer prints) launched an
/// agent task titled "computer on", twice. A line that starts with a built-in verb path is that command: a flag it
/// doesn't take is an error with its usage, and nothing on it ever falls through to a task.
#[test]
fn a_built_in_verb_path_refuses_a_flag_it_does_not_take_and_is_never_a_task() {
    let mut n = 0;
    for vp in VERB_PATHS {
        for verb in vp.verbs {
            let x = p(&[vp.cmd, verb, "--definitely-not-a-flag"]);
            let (cmd, v) = verb_path(&x.words).unwrap_or_else(|| panic!("{} {verb}", vp.cmd));
            assert_eq!((cmd, v), (vp.cmd, *verb));
            assert_eq!(bad_flag(&x.words, &x.argv).as_deref(), Some("--definitely-not-a-flag"), "{} {verb}", vp.cmd);
            // its own flags, and the ones every command takes, are fine
            for f in vp.flags.iter().chain(["--json", "--yes", "--port"].iter()) {
                let x = p(&[vp.cmd, verb, f, "1"]);
                assert_eq!(bad_flag(&x.words, &x.argv), None, "{} {verb} {f}", vp.cmd);
            }
            // whatever follows the verb path, the words are that command's, never a prompt
            let x = p(&[vp.cmd, verb, "and", "then", "some"]);
            assert!(verb_path(&x.words).is_some(), "{} {verb} …", vp.cmd);
            n += 1;
        }
    }
    assert!(n > 40, "{n} verb paths");
    // what the Grid prints: --grid goes with `computer on`, and a task never starts from it
    let x = p(&["computer", "on", "--grid", "wisp.localhost:4797"]);
    assert_eq!(x.words, w(&["computer", "on"]));
    assert_eq!((verb_path(&x.words), bad_flag(&x.words, &x.argv)), (Some(("computer", "on")), None));
    let x = p(&["computer", "on", "--grid=http://wisp.localhost:4797"]);
    assert_eq!(bad_flag(&x.words, &x.argv), None);
    // a flag's value is never read as a flag; the word after a switch is
    let x = p(&["trigger", "add", "--prompt", "-v flag broken", "--json"]);
    assert_eq!(bad_flag(&x.words, &x.argv), None);
    for (args, bad) in [
        (&["trigger", "add", "--off", "--bogus"][..], "--bogus"),
        (&["events", "tail", "-f", "--bogus"], "--bogus"),
        (&["computer", "on", "--json", "--bogus"], "--bogus"),
        // hook runs git with -q and -m; it takes neither
        (&["hook", "install", "-q"], "-q"),
    ] {
        let x = p(args);
        assert_eq!(bad_flag(&x.words, &x.argv).as_deref(), Some(bad), "{args:?}");
    }
    // after `--`, words are words: a task on purpose
    let x = p(&["--", "computer", "on", "--definitely-not-a-flag"]);
    assert_eq!(bad_flag(&x.words, &x.argv), None);
    // not a verb path: a prompt that merely starts with a command's name stays a task
    for prompt in [&["computer", "science", "notes"][..], &["cloud", "costs", "report"]] {
        assert_eq!(verb_path(&w(prompt)), None, "{prompt:?}");
    }
}

/// Every verb a built-in arm in main.rs routes is in `VERB_PATHS`, so none of them can fall through to a task.
#[test]
fn every_verb_main_routes_for_a_built_in_is_in_the_table() {
    let lines: Vec<&str> = include_str!("main.rs").lines().collect();
    for vp in VERB_PATHS {
        // its arms: a line whose pattern (before ` if` or `=>`) names it, and the verbs in the guard that follows
        let quoted = format!("\"{}\"", vp.cmd);
        let arms: Vec<usize> = (0..lines.len())
            .filter(|&i| {
                let l = lines[i].trim_start();
                l.starts_with('"') && l.split(" if").next().unwrap_or("").split("=>").next().unwrap_or("").contains(&quoted)
            })
            .collect();
        assert!(!arms.is_empty(), "main.rs routes {}", vp.cmd);
        for at in arms {
            // the arm's pattern and guard, up to its `=>` (six lines read on into the next arm's verbs)
            let end = (at..lines.len()).find(|&j| lines[j].contains("=>")).unwrap_or(at).min(at + 6);
            let arm = lines[at..=end.min(lines.len() - 1)].join(" ");
            for verb in arm.split("Some(").nth(1).unwrap_or("").split(')').next().unwrap_or("").split('|') {
                let verb = verb.trim().trim_matches('"');
                if verb.is_empty() || verb.contains(' ') {
                    continue;
                }
                assert!(vp.verbs.contains(&verb), "main.rs routes `wisp {} {verb}`: add it to words::VERB_PATHS", vp.cmd);
            }
        }
    }
}

/// `wisp mcp` takes `--repo <dir>` and the global flags only; `wisp plugin ls` the global ones: anything else is named,
/// a flag's value is never read as one, and a `--` ends the flags.
#[test]
fn a_command_that_reads_its_own_flags_names_the_first_it_doesnt_take() {
    let a = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(stray_flag("mcp", &["--repo"], &a("mcp --repo /w --port 4800")), None);
    assert_eq!(stray_flag("mcp", &["--repo"], &a("mcp tools --definitely-not-a-flag")).as_deref(), Some("--definitely-not-a-flag"));
    assert_eq!(stray_flag("mcp", &["--repo"], &a("mcp --repo --x")), None, "--repo's value");
    assert_eq!(stray_flag("plugins", &[], &a("plugins list --json")), None);
    assert_eq!(stray_flag("plugins", &[], &a("plugins list --all")).as_deref(), Some("--all"));
    assert_eq!(stray_flag("plugins", &[], &a("plugins list -- --all")), None);
}

/// QA round 10 #1: `wisp computer start|stop|enable`, `wisp recipes list` and `wisp grid start` launched agent tasks.
/// A line whose first word is a command group (built in, or a plugin of the catalogue) is a command line: a next word
/// that's none of its verbs is an error naming them, never a task. One quoted argument, or `--` first, is the task.
#[test]
fn an_unknown_verb_under_a_command_group_is_an_error_naming_its_verbs_and_never_a_task() {
    // a catalogue as the server lists it: built-in groups' plugin verbs (crew, machines, mcp) and plugins of their own
    let cat: Vec<Vec<String>> = [
        &["crew", "create"][..],
        &["crew", "list"],
        &["mcp", "session", "set"],
        &["mcp", "tools"],
        &["machines", "list"],
        &["ship", "status"],
        &["review", "start"],
        &["update", "check"],
        &["git", "github-repos"],
        &["learn", "view"],
    ]
    .map(w)
    .to_vec();
    for (args, group) in [
        (&["computer", "start"][..], "computer"),
        (&["computer", "stop"], "computer"),
        (&["computer", "enable"], "computer"),
        (&["recipes", "list"], "recipes"),
        (&["grid", "start"], "grid"),
        (&["crew", "frobnicate"], "crew"),
        (&["mcp", "session", "bogus"], "mcp"),
        (&["trigger", "make"], "trigger"),
        (&["completions", "powershell"], "completions"),
        (&["machines", "frob"], "machines"),
    ] {
        let x = p(args);
        let u = unknown_verb(&x.words, x.literal, &cat).unwrap_or_else(|| panic!("{args:?}"));
        assert_eq!(u.group, group, "{args:?}");
        let said = u.said(&x.words);
        assert!(said.contains(&format!("wisp {group} has no ")), "{said}");
        assert!(said.contains(&format!("To start a task with these words: wisp -- \"{}\"", args.join(" "))), "{said}");
    }
    let verbs = |args: &[&str]| unknown_verb(&p(args).words, Literal::No, &cat).map(|u| u.verbs).unwrap_or_default();
    assert_eq!(verbs(&["computer", "start"]), w(&["on", "off", "status"]));
    let grid = verbs(&["grid", "start"]);
    assert!(crate::args::GRID_SUB.iter().chain(["deploy"].iter()).all(|v| grid.contains(&(*v).to_owned())), "{grid:?}");
    let crew = verbs(&["crew", "start"]);
    assert!(["ls", "wake", "graph", "create", "list"].iter().all(|v| crew.contains(&(*v).to_owned())), "{crew:?}");
    assert!(verbs(&["mcp", "x"]).contains(&"session set".to_owned()));
    assert!(
        unknown_verb(&p(&["recipes", "list"]).words, Literal::No, &cat).unwrap().said(&w(&["recipes", "list"])).contains("takes no verb")
    );
    // its verbs aren't unknown (their own arms decide), nor a flag, nor one word alone
    for args in [
        &["computer", "on"][..],
        &["crew", "create", "--name", "Ada"],
        &["mcp", "session", "set"],
        &["computer", "--bogus"],
        &["computer"],
        &["grid", "deploy", "-f", "x.yml"],
    ] {
        assert!(unknown_verb(&p(args).words, p(args).literal, &cat).is_none(), "{args:?}");
    }
    // a prompt: one quoted argument, `--` first, a first word that's no group, or a plugin's name followed by none of
    // its verbs (owner's delegate: `wisp <prompt>` is how tasks start, so only built-in groups are strict)
    for args in [
        &["computer start fails on boot, fix it"][..],
        &["--", "computer", "start"],
        &["fix", "the", "computer"],
        &["tasks", "are", "slow", "to", "list"],
        &["ship", "the", "docs"],
        &["review", "the", "auth", "module"],
        &["update", "the", "readme"],
        &["git", "rebase", "onto", "main"],
        // (`learn rust`, one word under a plugin, is a slip since B1: the test below)
        &["learn", "rust", "properly"],
    ] {
        let x = p(args);
        assert!(unknown_verb(&x.words, x.literal, &cat).is_none(), "{args:?}");
    }
}

/// Owner's delegate (QA round 10 #1): the word after the first decides. A prompt whose later words look like a flag
/// is still a prompt (`wisp review the auth module --strict`); a command word followed at once by a flag it doesn't
/// take is still that command (`wisp tasks --touchng x`, round 255).
#[test]
fn only_the_next_word_makes_a_line_command_shaped() {
    let plugins = w(&["review", "runtime"]);
    for args in
        [&["review", "the", "auth", "module", "--strict"][..], &["ship", "the", "docs", "--fast"], &["tasks", "are", "slow", "--fix"]]
    {
        let x = p(args);
        assert_eq!(not_a_task(&x.words, x.literal, &plugins), None, "{args:?}");
    }
    let x = p(&["tasks", "--touchng", "src/a.rs"]);
    assert_eq!(not_a_task(&x.words, x.literal, &plugins), Some(NotATask::Usage("tasks".to_owned())));
    let x = p(&["review", "--strict"]);
    assert_eq!(not_a_task(&x.words, x.literal, &plugins), Some(NotATask::Plugin("review".to_owned())));
}

/// QA round 11 #1: `wisp version`, `wisp update now`, `wisp machine add` and `wisp share list` started tasks. Each
/// word a user may type as a command (v2's, and v1's: usage, spell, version, update, machine) is a strict group, a
/// command that takes an argument, or a word a prompt may start with ([`PROMPT_WORDS`]), never two of these.
#[test]
fn every_command_word_is_strict_takes_an_argument_or_starts_a_prompt() {
    let cat: Vec<Vec<String>> = [
        &["machines", "add"][..],
        &["machines", "list"],
        &["machine", "read"],
        &["share", "send"],
        &["share", "destinations"],
        &["update", "check"],
        &["update", "install"],
    ]
    .map(w)
    .to_vec();
    for (args, group) in [
        (&["version", "now"][..], "version"),
        (&["update", "now"], "update"),
        (&["update", "now", "--yes"], "update"),
        (&["machine", "frob"], "machine"),
        (&["share", "list"], "share"),
        (&["shares", "list"], "shares"),
        (&["usage", "today"], "usage"),
        (&["spell", "cast"], "spell"),
        (&["serve", "now"], "serve"),
        (&["tui", "please"], "tui"),
        (&["shell-init", "powershell"], "shell-init"),
        (&["schema", "dump"], "schema"),
    ] {
        let x = p(args);
        let u = unknown_verb(&x.words, x.literal, &cat).unwrap_or_else(|| panic!("{args:?}"));
        assert_eq!(u.group, group, "{args:?}");
    }
    let verbs = |args: &[&str]| unknown_verb(&p(args).words, Literal::No, &cat).map(|u| u.verbs).unwrap_or_default();
    // `machine` is this machine's plugin, and `machines`' verbs are its too (`wisp machine add` is `wisp machines add`)
    assert!(["read", "add", "list"].iter().all(|v| verbs(&["machine", "frob"]).contains(&(*v).to_owned())));
    assert!(unknown_verb(&p(&["machine", "add"]).words, Literal::No, &cat).is_none());
    assert_eq!(machines_alias(&w(&["machine", "add", "--name", "studio"])), Some(w(&["machines", "add", "--name", "studio"])));
    assert_eq!(machines_alias(&w(&["machine", "read"])), Some(w(&["machines", "read"])), "tried after machine's own verbs");
    assert_eq!(machines_alias(&w(&["machines", "add"])), None);
    // share: its forms and its plugin's verbs, and where your shares are
    let share = unknown_verb(&p(&["share", "list"]).words, Literal::No, &cat).unwrap();
    assert!(share.verbs.contains(&"<task id>".to_owned()) && share.verbs.contains(&"send".to_owned()), "{share:?}");
    assert!(share.said(&w(&["share", "list"])).contains("wisp shares"), "{}", share.said(&w(&["share", "list"])));
    // `update` is a prompt's first word too: strict with one word after it, a prompt with more
    assert!(unknown_verb(&p(&["update", "the", "readme"]).words, Literal::No, &cat).is_none());
    // update's own `apply`, and its plugin's verbs
    assert!(["apply", "check", "install"].iter().all(|v| verbs(&["update", "now"]).contains(&(*v).to_owned())));
    // alone, each is a known word (its usage, or its command), never a one-word task
    for word in ["version", "update", "usage", "spell", "spells", "machine", "share", "shares", "serve", "tui"] {
        assert!(known(word), "{word}");
        assert!(usage(word).contains(&format!("wisp {word}")) || usage(word).contains(&format!("|{word}")), "{word}: {}", usage(word));
    }
    // the three kinds never overlap: a prompt word is no group
    for word in PROMPT_WORDS {
        assert!(!is_group(word), "{word} is both a prompt word and a strict group");
        assert!(known(word), "{word} isn't a command word at all");
    }
}

/// A built-in group that's also a plugin (`machines`, `machine`), alone: its plugin's verbs, as before it was a group.
#[test]
fn a_group_that_is_a_plugin_alone_lists_its_verbs() {
    let plugins = w(&["machines", "machine", "update"]);
    for word in ["machines", "machine"] {
        assert_eq!(not_a_task(&w(&[word]), Literal::No, &plugins), Some(NotATask::Plugin(word.to_owned())), "{word}");
    }
    // a group that's only wisp's: its usage
    assert_eq!(not_a_task(&w(&["spell"]), Literal::No, &plugins), Some(NotATask::Usage("spell".to_owned())));
}

/// QA round 12 #1: `wisp login`, `wisp doctor`, `wisp rm 123`, `wisp kill 123`… each launched an agent (a paid run).
/// One word that isn't a command, or a word followed only by id-shaped arguments, is never a task: the words other tools
/// use say what to type instead, and the rest say they aren't commands. One quoted argument, or `--`, is the task.
#[test]
fn one_word_or_a_word_and_ids_is_never_a_task_and_says_what_to_type() {
    for (args, instead) in [
        (&["login"][..], "wisp cloud signin"),
        (&["signin"], "wisp cloud signin"),
        (&["logout"], "wisp cloud signout"),
        (&["signout"], "wisp cloud signout"),
        (&["whoami"], "wisp cloud status"),
        (&["config"], "wisp settings list"),
        (&["projects"], "wisp ls"),
        (&["remove"], "wisp runtime delete <id>"),
        (&["rm", "123"], "wisp runtime delete <id>"),
        (&["delete", "01a1ef07823670f68eb62f4ddb8ededc"], "wisp runtime delete <id>"),
        (&["kill", "123"], "wisp stop <id>"),
        (&["upgrade"], "wisp update"),
        (&["agents"], "wisp accounts harnesses"),
        (&["top"], "wisp ls --watch"),
        (&["ps"], "wisp ls"),
        (&["cancel", "123"], "wisp stop <id>"),
        (&["pause"], "wisp stop <id>"),
        (&["start"], "wisp serve"),
        (&["restart"], "wisp serve"),
        (&["new"], "wisp \"<what to do>\""),
        (&["info"], "wisp version"),
        (&["doctor"], "wisp accounts harnesses"),
    ] {
        let x = p(args);
        match not_a_task(&x.words, x.literal, &[]) {
            Some(NotATask::Instead(t)) => assert!(t.contains(instead), "{args:?}: {t}"),
            other => panic!("{args:?}: {other:?}"),
        }
    }
    // any other word alone, or with ids: not a command, never a task
    for args in [
        &["refactor"][..],
        &["fix"],
        &["test"],
        &["tests"],
        &["lint"],
        &["state"],
        &["trace"],
        &["go"],
        &["frobnicate", "--x"],
        &["README.md"],
        &["fetch", "123"],
    ] {
        let x = p(args);
        assert!(matches!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Unknown(_) | NotATask::Near(_))), "{args:?}");
    }
    // the task, on purpose: `--` first, or one quoted argument; a `file:line` isn't an id
    for args in
        [&["--", "login"][..], &["--", "refactor"], &["log me in with the new SSO"], &["explain", "src/lib.rs:42"], &["fix", "main.rs:120"]]
    {
        let x = p(args);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), None, "{args:?}");
    }
}

/// QA round 14 (decided rule): after a word that is a command, one word it doesn't take is a slip (its usage and the
/// `wisp -- …` way out, exit 2); two or more are a prompt. For every strict group (`settings` too), the words other
/// tools use (`status`), and the commands that take a task (`show foo` isn't an id).
#[test]
fn one_word_after_a_command_word_is_a_slip_and_two_or_more_are_a_prompt() {
    let cat: Vec<Vec<String>> = [&["settings", "list"][..], &["settings", "get"], &["settings", "set"], &["share", "send"]].map(w).to_vec();
    // strict groups: one unknown word
    for args in [&["update", "deps"][..], &["settings", "open"], &["computer", "start"], &["share", "list"]] {
        assert!(unknown_verb(&p(args).words, Literal::No, &cat).is_some(), "{args:?}");
    }
    // two or more: a prompt (a group's own verb first stays its command)
    for args in [
        &["share", "the", "results", "with", "the", "team"][..],
        &["settings", "page", "needs", "a", "dark", "mode"],
        &["computer", "start", "it", "now"],
        &["update", "the", "readme"],
    ] {
        let x = p(args);
        assert!(unknown_verb(&x.words, x.literal, &cat).is_none() && !group_line(&x.words), "{args:?}");
        assert_eq!(not_a_task(&x.words, x.literal, &w(&["settings", "share"])), None, "{args:?}");
    }
    assert!(group_line(&w(&["crew", "ls", "extra"])), "a group's verb then words: its command, refused");
    // another tool's word, one more word: its hint; more: a prompt
    let x = p(&["status", "all"]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Near("show".to_owned())));
    let x = p(&["status", "badge", "in", "the", "README"]);
    assert_eq!(not_a_task(&x.words, x.literal, &[]), None);
    // a command that takes a task, one word that's no id: its usage
    for args in [&["show", "foo"][..], &["stop", "it"], &["wait", "x"], &["reply", "here"], &["attach", "it"]] {
        let x = p(args);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Usage(args[0].to_owned())), "{args:?}");
    }
    assert!(settings_is_a_group());
}

fn settings_is_a_group() -> bool {
    is_group("settings")
}

/// QA round 15 #2, #6: `wisp export foo` launched a task (one stray word after a command is a slip; more is a prompt),
/// and `wisp fork` pointed nowhere.
#[test]
fn one_stray_word_after_a_prompt_word_is_a_slip_and_fork_points_to_runtime_fork() {
    for args in [&["export", "foo"][..], &["ship", "foo"], &["add", "tests"], &["plan", "x"], &["fresh", "x"]] {
        let x = p(args);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), Some(NotATask::Usage(args[0].to_owned())), "{args:?}");
    }
    for args in [
        &["export", "the", "results", "as", "CSV"][..],
        &["fork", "the", "repo", "and", "set", "up", "CI"],
        &["add", "a", "README"],
        &["ship", "the docs"],
        &["add", "tests for login"],
        &["export", "--", "foo"],
    ] {
        let x = p(args);
        assert_eq!(not_a_task(&x.words, x.literal, &[]), None, "{args:?}");
    }
    match not_a_task(&w(&["fork"]), Literal::No, &[]) {
        Some(NotATask::Instead(t)) => assert!(t.contains("wisp runtime fork --id <id>"), "{t}"),
        other => panic!("{other:?}"),
    }
}

/// Setup audit F30 and B1: `wisp budget stop-at`, then `wisp budget frobnicate`, launched a Full-auto task (from a
/// script, with no terminal, as in one). A plugin's name and one word that's none of its verbs is a slip, whatever the
/// word looks like, as under a built-in group (`group_line`): a slip never launches. Two or more words are a prompt.
#[test]
fn a_plugin_name_and_any_one_other_word_is_a_slip() {
    let cat: Vec<Vec<String>> = [&["budget", "view"][..], &["budget", "set-stop"], &["learn", "view"], &["learn", "run"]].map(w).to_vec();
    for args in [
        &["budget", "stop-at"][..],
        &["budget", "setStop"],
        &["budget", "veiw"],
        &["learn", "viewed"],
        &["budget", "frobnicate"],
        &["learn", "rust"],
        &["budget", "frobnicate", "--json"],
    ] {
        let u = unknown_verb(&p(args).words, Literal::No, &cat).unwrap_or_else(|| panic!("{args:?}"));
        assert!(u.verbs.contains(&"set-stop".to_owned()) || u.verbs.contains(&"view".to_owned()), "{u:?}");
    }
    for args in [&["learn", "rust", "properly"][..], &["budget", "dashboard", "for", "the", "team"], &["budget", "view"]] {
        assert!(unknown_verb(&p(args).words, Literal::No, &cat).is_none(), "{args:?}");
    }
    // `--` or one quoted argument: the words are a task
    assert!(unknown_verb(&p(&["--", "learn", "rust"]).words, p(&["--", "learn", "rust"]).literal, &cat).is_none());
    // one quoted argument after a plugin's name is the task too (`wisp learn "rust properly"`)
    assert!(unknown_verb(&p(&["learn", "rust properly"]).words, p(&["learn", "rust properly"]).literal, &cat).is_none());
    // a word that's no plugin's name: a prompt, as ever
    assert!(unknown_verb(&p(&["refactor", "it"]).words, Literal::No, &cat).is_none());
}

/// Auto off (`runtime.auto`, audit C6): `wisp router` followed by any words is refused with the `wisp -- "…"` way out,
/// never a prompt that launches a paid run; on, or after `--`, the words are what they'd be anyway.
#[test]
fn with_auto_off_router_and_any_words_is_refused_never_a_prompt() {
    for args in [&["router", "preview"][..], &["router", "pick", "the", "best", "harness", "for", "this"], &["router", "--json"]] {
        let x = p(args);
        let said = router_away(&x.words, x.literal, true).unwrap_or_else(|| panic!("{args:?} was read as a prompt"));
        assert!(said.contains(&format!("wisp -- \"{}\"", x.words.join(" "))), "{said}");
    }
    assert!(router_away(&p(&["router", "preview"]).words, p(&["router", "preview"]).literal, false).is_none(), "Auto on");
    let lit = p(&["--", "router", "preview"]);
    assert!(router_away(&lit.words, lit.literal, true).is_none(), "`wisp -- …` is the task");
    let quoted = p(&["router preview"]);
    assert!(router_away(&quoted.words, quoted.literal, true).is_none(), "one quoted argument is the task");
    assert!(router_away(&p(&["review", "the", "router"]).words, Literal::No, true).is_none(), "router later on is a prompt");
}

/// Audit F setup F1: with `wisp.settings` (or `wisp.learn`) off, `wisp settings get crew.tint` launched a Full-auto
/// task. An off plugin's words are refused with its state and the way to switch it on; `--` or one quoted argument is
/// still the task, and a word that names no off plugin is what it'd be anyway.
#[test]
fn an_off_plugins_words_are_refused_never_a_task_audit_f_setup_f1() {
    let off = vec![("wisp.settings".to_owned(), Some("wisp.settings was switched off".to_owned())), ("user.qa-theme".to_owned(), None)];
    for args in [&["settings", "get", "crew.tint"][..], &["settings", "please", "fix", "the", "login"], &["qa-theme", "view"]] {
        let x = p(args);
        let said = plugin_off(&x.words, x.literal, &off).unwrap_or_else(|| panic!("{args:?} was read as a prompt"));
        assert!(said.contains("is switched off") && said.contains("wisp plugin enable"), "{said}");
        assert!(said.contains(&format!("wisp -- \"{}\"", x.words.join(" "))), "{said}");
    }
    let said = plugin_off(&p(&["settings", "get", "x"]).words, Literal::No, &off).unwrap();
    assert!(said.contains("(wisp.settings was switched off)") && said.contains("wisp plugin enable wisp.settings"), "{said}");
    let lit = p(&["--", "settings", "get", "x"]);
    assert!(plugin_off(&lit.words, lit.literal, &off).is_none(), "`wisp -- …` is the task");
    assert!(plugin_off(&p(&["learn", "view"]).words, Literal::No, &off).is_none(), "wisp.learn is on");
    assert!(plugin_off(&p(&["fix", "the", "settings"]).words, Literal::No, &off).is_none(), "settings later on is a prompt");
}

/// Audit D9: `wisp validate -f nope.yml`, `apply -f` and `wisp import <folder>` printed "No such file or directory (os
/// error 2)". A file that can't be read is said in words, as a missing project folder is.
#[test]
fn a_file_that_cant_be_read_is_said_in_words_audit_d9() {
    use std::io::{Error, ErrorKind};
    let f = Path::new("/tmp/qa/nope.yml");
    assert_eq!(unreadable(f, &Error::from(ErrorKind::NotFound)), "No file at /tmp/qa/nope.yml");
    assert_eq!(unreadable(f, &Error::from(ErrorKind::IsADirectory)), "/tmp/qa/nope.yml is a folder, not a file");
    assert_eq!(unreadable(f, &Error::from(ErrorKind::PermissionDenied)), "Can't read /tmp/qa/nope.yml: permission denied");
    assert!(!unreadable(f, &Error::from(ErrorKind::NotFound)).contains("os error"));
    let gone = Path::new("/wisp-no-such-v1-home");
    assert_eq!(
        no_v1_home(gone, &gone.join("config.json"), &Error::from(ErrorKind::NotFound)),
        "No folder at /wisp-no-such-v1-home (wisp import [v1 home folder])"
    );
    let here = std::env::temp_dir();
    let said = no_v1_home(&here, &here.join("config.json"), &Error::from(ErrorKind::NotFound));
    assert!(said.contains("has no config.json") && !said.contains("os error"), "{said}");
}

/// Audit H2: `wisp "H typo2" --mdoe plan` launched in Full auto with `--mdoe plan` in its prompt. A flag-shaped word
/// before `--` is a stray (refused with the close flag); after `--`, or inside the quoted prompt, it's the prompt's.
#[test]
fn a_misspelled_launch_flag_is_a_stray_named_with_its_close_flag_audit_h2() {
    assert_eq!(p(&["H typo2", "--mdoe", "plan"]).strays, ["--mdoe"]);
    assert_eq!(p(&["H", "--agnet=codex"]).strays, ["--agnet"]);
    let none: [String; 0] = [];
    assert_eq!(p(&["H", "--", "--mdoe", "plan"]).strays, none, "after -- it's the prompt");
    assert_eq!(p(&["fix the --mdoe flag parsing"]).strays, none, "inside the prompt's quotes");
    assert_eq!(p(&["H", "--mode", "plan", "-d"]).strays, none);
    let said = launch_stray(&w(&["H typo2", "--mdoe", "plan"]), "--mdoe");
    assert!(said.starts_with("wisp takes no --mdoe: did you mean --mode?") && said.contains("Nothing was launched"), "{said}");
    assert!(said.contains("wisp \"H typo2\" -- --mdoe plan"), "{said}");
    assert!(launch_stray(&w(&["x", "--agnet"]), "--agnet").contains("did you mean --agent?"));
    let bogus = launch_stray(&w(&["x", "--bogus"]), "--bogus");
    assert!(!bogus.contains("did you mean"), "{bogus}");
    assert!(launch_stray(&w(&["x", "--at"]), "--at").contains("can't start later from here"));
    // nothing before the flag: no empty quotes in the way out
    assert!(launch_stray(&w(&["--mdoe", "plan"]), "--mdoe").ends_with("A task that says it: wisp -- --mdoe plan"));
}

/// Audit H11: `wisp run nosuch` said "no recipe or workflow named nosuch: wisp recipes lists them"; it runs declared
/// tasks too, and the line names each kind with what lists it.
#[test]
fn a_run_that_names_nothing_names_each_kind_and_its_list_audit_h11() {
    let said = run_miss("nosuch");
    assert!(said.starts_with("no declared task, recipe or workflow named nosuch"), "{said}");
    assert!(said.contains("wisp recipes lists the declared tasks") && said.contains("wisp workflows list"), "{said}");
}
