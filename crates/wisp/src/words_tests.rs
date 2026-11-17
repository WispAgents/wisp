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
