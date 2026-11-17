//! Which words on the launch path are a task (round 255: `wisp tasks` started one). Pure, beside
//! [`crate::args`]: a command word alone prints its usage, a typo of one says did you mean, a plugin's
//! name lists its verbs; `wisp -- <word>` is always the task.
use crate::args::{COMMANDS, HELP, Literal};

/// The first words main.rs routes on itself besides COMMANDS (wisp as code, areas, shared and cloud: its
/// `match first` arms). `every_word_main_routes_on_is_a_known_word` reads main.rs and keeps them in step.
pub const ROUTED: [&str; 41] = [
    "apply",
    "secret",
    "diff",
    "validate",
    "plan",
    "destroy",
    "init",
    "track",
    "untrack",
    "blame",
    "log",
    "tasks",
    "resume",
    "adopt",
    "fmt",
    "workspace",
    "output",
    "graph",
    "mv",
    "schema",
    "export",
    "history",
    "rerun",
    "task",
    "share",
    "events",
    "shares",
    "cloud",
    "computer",
    "open",
    "hook",
    "tui",
    "shell-init",
    "trigger",
    "triggers",
    "sandbox",
    "sandboxes",
    "team",
    "teams",
    "notes",
    "fresh",
];

/// A word the CLI routes on as its first word (COMMANDS or ROUTED).
pub fn known(word: &str) -> bool {
    COMMANDS.contains(&word) || ROUTED.contains(&word)
}

/// Why words that reached the launch path aren't a task (round 255: `wisp tasks` started one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotATask {
    /// a command word without what it needs (`wisp tasks`, `wisp ship`): print its usage
    Usage(String),
    /// a plugin's name alone (`wisp runtime`): print its verbs
    Plugin(String),
    /// one or two letters off a command word or a plugin (`wisp tsks`), or another tool's name for one (`wisp get`
    /// is `wisp show`): did you mean
    Near(String),
    /// no command, but followed by a task id (`wisp fetch 01a1e9…`): never a prompt (it would start a run)
    Unknown(String),
}

/// Other tools' words for wisp's (`gh`, `kubectl`, `docker`): `wisp get <id>` means `wisp show <id>`.
const SYNONYMS: [(&str, &str); 6] =
    [("get", "show"), ("view", "show"), ("cat", "show"), ("status", "show"), ("list", "ls"), ("logs", "attach")];

/// A task's id as wisp prints it (12 to 32 hex digits), alone or as `<peer>:<id>`.
fn id_shaped(w: &str) -> bool {
    let id = w.rsplit_once(':').map_or(w, |(_, id)| id);
    (12..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// One word alone, or a first word followed by something flag-shaped or by task ids only, with no `--` before them.
pub fn command_shaped(words: &[String], literal: Literal) -> bool {
    let Some((_, rest)) = words.split_first() else { return false };
    literal == Literal::No && (rest.is_empty() || rest.iter().any(|w| flag_shaped(w)) || rest.iter().all(|w| id_shaped(w)))
}

/// `-x` or `--word`; a bare `-` or `--` is punctuation in a prompt (`plan it - postgres first`).
fn flag_shaped(w: &str) -> bool {
    w.strip_prefix("--").or_else(|| w.strip_prefix('-')).is_some_and(|r| r.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()))
}

/// None: the words are a task. Only words that look like a command are checked: one word alone, or a
/// first word followed by something flag-shaped the parser didn't take (`wisp tasks --touchng x`). A
/// prompt of several words (`wisp ship the docs`) and anything after a leading `--` stay a task.
pub fn not_a_task(words: &[String], literal: Literal, plugins: &[String]) -> Option<NotATask> {
    if !command_shaped(words, literal) {
        return None;
    }
    let first = words.first()?;
    if known(first) {
        return Some(NotATask::Usage(first.clone()));
    }
    if plugins.iter().any(|p| p == first) {
        return Some(NotATask::Plugin(first.clone()));
    }
    let lower = first.to_lowercase();
    if let Some((_, ours)) = SYNONYMS.iter().find(|(theirs, _)| *theirs == lower) {
        return Some(NotATask::Near((*ours).to_owned()));
    }
    let with_ids = words.len() > 1 && words[1..].iter().all(|w| id_shaped(w));
    if lower.is_empty() || !lower.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
        return with_ids.then(|| NotATask::Unknown(first.clone()));
    }
    let candidates = COMMANDS.iter().chain(ROUTED.iter()).copied().chain(plugins.iter().map(String::as_str));
    // the closest wins, the table's order breaks ties
    candidates
        .map(|c| (distance(&lower, c), c))
        .filter(|(d, c)| near(&lower, c, *d))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| NotATask::Near(c.to_owned()))
        .or_else(|| with_ids.then(|| NotATask::Unknown(first.clone())))
}

/// Whether `typed` reads as a slip of `cmd`, `d` apart. Under six letters only a dropped, added or
/// swapped letter counts (`tsks`, `taks`, `sl`, `shwo`); one letter changed is another word (`lint`,
/// `trace`), and so are two (`tests`, `start`). Two letters off counts when both are six or longer.
fn near(typed: &str, cmd: &str, d: usize) -> bool {
    let short = typed.len().min(cmd.len());
    let swapped = || {
        let (mut a, mut b): (Vec<u8>, Vec<u8>) = (typed.bytes().collect(), cmd.bytes().collect());
        a.sort_unstable();
        b.sort_unstable();
        a == b
    };
    match d {
        0 => true,
        1 if short >= 6 => true,
        1 if typed.len() == cmd.len() => swapped(),
        1 => typed.len() >= 3,
        2 => short >= 6,
        _ => false,
    }
}

/// Edit distance with adjacent swaps (`shwo` is one from `show`).
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    if let Some(row) = d.first_mut() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = j;
        }
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[a.len()][b.len()]
}

/// `wisp <word>`'s lines of HELP (a line naming it, `wisp diff|validate` too, with its continuation
/// lines), and how to make the word a task anyway.
pub fn usage(word: &str) -> String {
    let mut out = String::new();
    let mut taking = false;
    for line in HELP.lines() {
        let t = line.trim_start();
        if let Some(after) = t.strip_prefix("wisp ") {
            let head = after.split(' ').next().unwrap_or("");
            taking = head.split('|').any(|h| h == word);
        } else if !line.starts_with("    ") {
            taking = false;
        }
        if taking {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        // a plural or a synonym is documented under its other name
        let other = match word {
            "list" => "ls",
            w => w.strip_suffix('s').unwrap_or(w),
        };
        if other != word {
            return usage(other)
                .replace(&format!("wisp -- {other} "), &format!("wisp -- {word} "))
                .replace(&format!("with {other}:"), &format!("with {word}:"));
        }
        out = format!("  wisp {word} needs more: wisp --help\n");
    }
    format!("{out}  a task that starts with {word}: wisp -- {word} …")
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;
