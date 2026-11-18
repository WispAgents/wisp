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
        &["refactor"],
        &["fix"],
        &["test"],
        &["help me"],
        &["tests"],
        &["lint"],
        &["start"],
        &["state"],
        &["trace"],
        &["go"],
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
            let arm = lines[at..(at + 6).min(lines.len())].join(" ");
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
        (&["crew", "frobnicate", "now"], "crew"),
        (&["mcp", "session", "bogus"], "mcp"),
        (&["trigger", "make", "one"], "trigger"),
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
        &["learn", "rust"],
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
