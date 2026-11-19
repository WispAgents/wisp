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

/// A word the CLI routes on as its first word (COMMANDS, ROUTED, or a built-in group).
pub fn known(word: &str) -> bool {
    COMMANDS.contains(&word) || ROUTED.contains(&word) || is_group(word)
}

/// Command words a prompt may also start with: their command takes a particular argument (`ship <id>`, `run <recipe>`,
/// `plan` in a `.wisp/` folder, `notes <task>`), and when it isn't there the words are a task (`wisp ship the docs`,
/// `wisp plan the migration`, `wisp run the tests`). Every other command word is a strict group or a command that
/// takes an argument, never a task (QA round 11 #1; `words_tests.rs` and the e2e in `tests/serve_cli_verbs.rs`, which
/// reads this list from the source, hold each word to it: the routing itself lives in main.rs's arms and `args::fits`).
pub const PROMPT_WORDS: &[&str] = &[
    "add", "ship", "merge", "run", "plan", "apply", "diff", "validate", "destroy", "init", "track", "untrack", "blame", "log", "tasks",
    "resume", "fmt", "output", "graph", "mv", "import", "export", "history", "rerun", "open", "notes", "fresh", "attach",
];

/// `wisp machine <verb>`: this machine's plugin first, else the same verb of `machines` (`wisp machine add` is
/// `wisp machines add`, QA round 11 #1). The words as `machines`', for the caller to try.
pub fn machines_alias(words: &[String]) -> Option<Vec<String>> {
    (words.first().map(String::as_str) == Some("machine"))
        .then(|| std::iter::once("machines".to_owned()).chain(words[1..].iter().cloned()).collect())
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
    /// no command, but followed by a task id (`wisp fetch 01a1e9…`), or one word alone (`wisp refactor`): never a
    /// prompt (it would start a run)
    Unknown(String),
    /// a word another tool or habit uses (`wisp login`, `wisp rm 123`): what to type in wisp (QA round 12 #1)
    Instead(String),
}

/// Other tools' words for wisp's (`gh`, `kubectl`, `docker`): `wisp get <id>` means `wisp show <id>`.
const SYNONYMS: [(&str, &str); 6] =
    [("get", "show"), ("view", "show"), ("cat", "show"), ("status", "show"), ("list", "ls"), ("logs", "attach")];

/// A task's id as wisp prints it (12 to 32 hex digits), alone or as `<peer>:<id>` (a machine's name: no `/` or `.`,
/// so `src/lib.rs:42` is a place in a file, not an id), or a bare number (`wisp kill 123`).
fn id_shaped(w: &str) -> bool {
    let hex = |id: &str| (12..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit());
    match w.rsplit_once(':') {
        Some((peer, id)) => !peer.is_empty() && peer.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') && hex(id),
        None => hex(w) || (!w.is_empty() && w.bytes().all(|b| b.is_ascii_digit())),
    }
}

/// Words other tools (or habit) use, and what to type in wisp instead (QA round 12 #1: each launched an agent).
const INSTEAD: &[(&str, &str)] = &[
    ("login", "wisp cloud signin"),
    ("signin", "wisp cloud signin"),
    ("logout", "wisp cloud signout"),
    ("signout", "wisp cloud signout"),
    ("whoami", "wisp cloud status"),
    ("config", "wisp settings list"),
    ("projects", "wisp ls (each task with its project; wisp add [path] adds one)"),
    ("remove", "wisp runtime delete <id>"),
    ("rm", "wisp runtime delete <id>"),
    ("delete", "wisp runtime delete <id>"),
    ("kill", "wisp stop <id>"),
    ("upgrade", "wisp update"),
    // QA round 15 #6: Fork is a task's ⋯ in the app
    ("fork", "wisp runtime fork --id <id> (Fork… in a task's ⋯)"),
    // QA round 13 #3: read-only intents get read-only answers (`top` was "did you mean wisp stop?")
    ("agents", "wisp accounts harnesses (the agents it can run)"),
    ("top", "wisp ls --watch (your tasks, live)"),
    ("ps", "wisp ls"),
    ("cancel", "wisp stop <id>"),
    ("pause", "wisp stop <id>"),
    ("start", "wisp serve (runs wisp here; wisp alone opens it)"),
    ("restart", "wisp serve (runs wisp here; wisp alone opens it)"),
    ("new", "wisp \"<what to do>\""),
    ("info", "wisp version (a task's: wisp show <id>)"),
    // outside a wisp checkout (main.rs points a checkout at `npm run doctor`)
    ("doctor", "wisp version, wisp accounts harnesses (the agents it can run) and wisp cloud status"),
];

/// The commands that take a task (`wisp show <id>`; args.rs `fits` routes them only for a task's id or `last`).
const TASK_COMMANDS: [&str; 6] = ["show", "stop", "wait", "reply", "attach", "btw"];

/// QA round 14 (decided rule): a word that means a command (another tool's, `wisp status`, or one that takes a task,
/// `wisp stop`) and exactly one word it doesn't take (`wisp status all`, `wisp stop it`): a slip, its hint or its usage.
/// Two or more words are a prompt (`wisp stop using var in math.js`).
fn one_word_slip(words: &[String], literal: Literal) -> Option<NotATask> {
    let (first, rest) = words.split_first()?;
    let said: Vec<&String> = rest.iter().filter(|w| !flag_shaped(w)).collect();
    // one quoted argument is the prompt (`wisp ship "the docs"`)
    if literal != Literal::No || said.len() != 1 || crate::args::task_ref(said[0]) || said[0].contains(char::is_whitespace) {
        return None;
    }
    let lower = first.to_lowercase();
    if let Some((_, ours)) = SYNONYMS.iter().find(|(theirs, _)| *theirs == lower) {
        return Some(NotATask::Near((*ours).to_owned()));
    }
    if let Some((_, instead)) = INSTEAD.iter().find(|(theirs, _)| *theirs == lower) {
        return Some(NotATask::Instead((*instead).to_owned()));
    }
    // a command that takes a task, or a word a prompt may start with (`wisp export foo`, QA round 15 #2): its usage
    (TASK_COMMANDS.contains(&first.as_str()) || PROMPT_WORDS.contains(&first.as_str())).then(|| NotATask::Usage(first.clone()))
}

/// Each built-in group and its verbs, for the shell completions (a form like `<task id>` left out).
pub fn group_verbs() -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let all = VERB_PATHS.iter().map(|p| (p.cmd, p.verbs)).chain(GROUPS.iter().map(|(g, vs)| (*g, *vs)));
    for (g, vs) in all {
        let vs: Vec<String> = vs.iter().filter(|v| !v.starts_with('<')).map(|v| (*v).to_owned()).collect();
        match out.iter_mut().find(|(x, _)| x == g) {
            Some((_, have)) => have.extend(vs.into_iter().filter(|v| !have.contains(v)).collect::<Vec<_>>()),
            None => out.push((g.to_owned(), vs)),
        }
    }
    out
}

/// Whether `word` is one of `group`'s own built-in verbs (`plugin install`, `skill install`, `crew ls`).
pub fn own_verb(group: &str, word: &str) -> bool {
    VERB_PATHS.iter().filter(|p| p.cmd == group).any(|p| p.verbs.contains(&word))
        || GROUPS.iter().filter(|(g, _)| *g == group).any(|(_, vs)| vs.contains(&word))
}

/// One word alone, or a first word followed at once by something flag-shaped, or by task ids only, with no `--` before
/// them. Only the next word counts: `wisp review the auth module --strict` is a prompt (owner's delegate, QA round 10).
pub fn command_shaped(words: &[String], literal: Literal) -> bool {
    let Some((_, rest)) = words.split_first() else { return false };
    literal == Literal::No && (rest.first().is_none_or(|w| flag_shaped(w)) || rest.iter().all(|w| id_shaped(w)))
}

/// `-x` or `--word`; a bare `-` or `--` is punctuation in a prompt (`plan it - postgres first`).
fn flag_shaped(w: &str) -> bool {
    w.strip_prefix("--").or_else(|| w.strip_prefix('-')).is_some_and(|r| r.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()))
}

/// None: the words are a task. Only words that look like a command are checked: one word alone, or a
/// first word followed by something flag-shaped the parser didn't take (`wisp tasks --touchng x`). A
/// prompt of several words (`wisp ship the docs`) and anything after a leading `--` stay a task.
pub fn not_a_task(words: &[String], literal: Literal, plugins: &[String]) -> Option<NotATask> {
    if let Some(slip) = one_word_slip(words, literal) {
        return Some(slip);
    }
    if !command_shaped(words, literal) {
        return None;
    }
    let first = words.first()?;
    let plugin = plugins.iter().any(|p| p == first);
    // a group that's also a plugin (`machines`): its plugin's verbs; a word that's only wisp's: its usage
    if known(first) && !(plugin && !COMMANDS.contains(&first.as_str()) && !ROUTED.contains(&first.as_str())) {
        return Some(NotATask::Usage(first.clone()));
    }
    if plugin {
        return Some(NotATask::Plugin(first.clone()));
    }
    let lower = first.to_lowercase();
    if let Some((_, ours)) = SYNONYMS.iter().find(|(theirs, _)| *theirs == lower) {
        return Some(NotATask::Near((*ours).to_owned()));
    }
    // one quoted argument is the prompt (`wisp "log me in with SSO"`)
    if first.contains(char::is_whitespace) {
        return None;
    }
    if let Some((_, instead)) = INSTEAD.iter().find(|(theirs, _)| *theirs == lower) {
        return Some(NotATask::Instead((*instead).to_owned()));
    }
    if !lower.is_empty() && lower.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
        let candidates = COMMANDS.iter().chain(ROUTED.iter()).copied().chain(plugins.iter().map(String::as_str));
        // the closest wins, the table's order breaks ties
        if let Some((_, c)) = candidates.map(|c| (distance(&lower, c), c)).filter(|(d, c)| near(&lower, c, *d)).min_by_key(|(d, _)| *d) {
            return Some(NotATask::Near(c.to_owned()));
        }
    }
    // one word, or a word and ids, that's no command: never a task (QA round 12 #1)
    Some(NotATask::Unknown(first.clone()))
}

/// A built-in command's verb path (`wisp computer on`, `wisp trigger add`): its first word, the verbs main.rs routes
/// for it, and the flags they read besides the ones every command takes ([`GLOBAL_FLAGS`]).
#[derive(Debug, Clone, Copy)]
pub struct VerbPath {
    pub cmd: &'static str,
    pub verbs: &'static [&'static str],
    pub flags: &'static [&'static str],
}

const fn vp(cmd: &'static str, verbs: &'static [&'static str], flags: &'static [&'static str]) -> VerbPath {
    VerbPath { cmd, verbs, flags }
}

/// Flags any command takes: where wisp is (`--port`, `--on`, `--cwd`), how it answers (`--json`, `--yes`), `--help`.
pub const GLOBAL_FLAGS: [&str; 11] = ["--json", "--yes", "-y", "--port", "-p", "--on", "--cwd", "-C", "--help", "-h", "--no-open"];

/// The flags above that are switches, not followed by a value (`events`' `-f` is `--follow`; elsewhere `-f` is a file).
const SWITCHES: &[&str] = &[
    "--json",
    "--yes",
    "-y",
    "--help",
    "-h",
    "--no-open",
    "--approval",
    "--follow-up",
    "--off",
    "--no",
    "--follow",
    "--fill-from-snapshot",
    "--wait",
    "-w",
    "--single-file",
    "--with-deps",
    "--dry-run",
    "--prune",
    "--trust",
    "--upgrade",
    "-upgrade",
    "--refresh-only",
    "-refresh-only",
];

fn takes_value(cmd: &str, flag: &str) -> bool {
    let follow = cmd == "events" && flag == "-f";
    !follow && !SWITCHES.contains(&flag)
}

const TRIGGER: &[&str] = &[
    "--agent",
    "-a",
    "--approval",
    "--branch",
    "--budget-day",
    "--concurrency",
    "--cooldown",
    "--crew",
    "--cron",
    "--debounce",
    "--event",
    "--every",
    "--follow-up",
    "--if",
    "--label",
    "--last",
    "--match",
    "--mode",
    "-m",
    "--model",
    "--no",
    "--off",
    "--payload",
    "--prompt",
    "--rate",
    "--secret",
    "--task",
    "--tz",
    "--users",
    "--why",
    "--workflow",
];
const TASK: &[&str] = &[
    "--formula",
    "--profile",
    "--include",
    "--format",
    "--from",
    "--steps",
    "--last",
    "--project",
    "--agent",
    "-a",
    "--model",
    "--effort",
    "--mode",
    "-m",
    "--out",
    "-o",
    "-f",
    "--file",
    "--fill-from-snapshot",
    "--wait",
    "-w",
    "--to",
    "--single-file",
    "--with-deps",
];
const APPLY: &[&str] = &[
    "-f",
    "--file",
    "--formula",
    "--profile",
    "--var",
    "-var",
    "--var-file",
    "-var-file",
    "--dry-run",
    "--prune",
    "--trust",
    "--upgrade",
    "-upgrade",
    "--refresh-only",
    "-refresh-only",
    "--wait",
    "-w",
];

/// Every built-in verb path main.rs routes (its `areas` and `shared_and_cloud` arms), one per line: the e2e
/// (`tests/serve_cli_verbs.rs`) reads these lines, and `every_verb_main_routes_for_a_built_in_is_in_the_table` keeps
/// them in step with main.rs. A line starting with one is that command, never a task (QA round 9 #1).
pub const VERB_PATHS: &[VerbPath] = &[
    vp("computer", &["on", "off", "status"], &["--grid"]),
    vp("cloud", &["signin", "login", "signout", "logout", "status", "accounts", "switch", "remove", "rm"], &[]),
    vp("sandbox", &["new", "add", "ls", "list", "rm", "remove", "delete"], &[]),
    vp("sandboxes", &["new", "add", "ls", "list", "rm", "remove", "delete"], &[]),
    vp("team", &["ls", "list", "new", "create", "show", "add", "rm", "remove"], &[]),
    vp("teams", &["ls", "list", "new", "create", "show", "add", "rm", "remove"], &[]),
    vp("trigger", &["add", "ls", "list", "rm", "remove", "fire", "test", "log", "resume", "approve", "sources"], TRIGGER),
    vp("triggers", &["add", "ls", "list", "rm", "remove", "fire", "test", "log", "resume", "approve", "sources"], TRIGGER),
    vp("crew", &["wake", "graph"], TRIGGER),
    vp("hook", &["install", "uninstall", "remove", "status"], &[]),
    vp("events", &["tail", "why", "retry", "parked", "ls"], &["--follow", "-f", "--limit", "--since", "--subject", "--type"]),
    vp("task", &["spec", "export", "formula", "setup", "history"], TASK),
    vp("grid", &["deploy"], APPLY),
    vp("update", &["apply"], &[]),
];

/// The built-in verb path the words start with (`computer on …` → `("computer", "on")`); None for a prompt that
/// merely starts with a command's name (`computer science notes`).
pub fn verb_path(words: &[String]) -> Option<(&'static str, &'static str)> {
    let (first, verb) = (words.first()?, words.get(1)?);
    VERB_PATHS.iter().find(|p| p.cmd == first).and_then(|p| p.verbs.iter().find(|v| **v == verb).map(|v| (p.cmd, *v)))
}

/// The first flag after a verb path that it doesn't take (`computer on --definitely-not-a-flag`); None when they're
/// all its own or global, when the words aren't a verb path, and for anything after a `--` (words, on purpose).
pub fn bad_flag(words: &[String], argv: &[String]) -> Option<String> {
    let (cmd, _) = verb_path(words)?;
    let path = VERB_PATHS.iter().find(|p| p.cmd == cmd)?;
    stray_flag(cmd, path.flags, argv)
}

/// The first flag after `cmd` in `argv` that is neither one of `own` nor global, before any `--`: a built-in command
/// that reads its own flags (`wisp mcp --repo <dir>`, `wisp plugin ls`) refuses the rest with its usage.
pub fn stray_flag(cmd: &str, own: &[&str], argv: &[String]) -> Option<String> {
    let at = argv.iter().position(|a| a == cmd)?;
    if argv[..at].iter().any(|a| a == "--") {
        return None;
    }
    let mut rest = argv[at + 1..].iter().take_while(|a| *a != "--");
    while let Some(a) = rest.next() {
        if !flag_shaped(a) {
            continue;
        }
        if !own.contains(&a.as_str()) && !GLOBAL_FLAGS.contains(&a.as_str()) {
            return Some(a.clone());
        }
        // its value is never read as a flag (`--prompt "-v is broken"`)
        if takes_value(cmd, a) {
            rest.next();
        }
    }
    None
}

/// `wisp plugin …` (`plugin_cmd::run`, and main.rs's `new`).
const PLUGIN_VERBS: &[&str] =
    &["ls", "list", "enable", "disable", "on", "off", "install", "add", "trust", "untrust", "remove", "uninstall", "rm", "new"];

/// Built-in command groups besides [`VERB_PATHS`]: COMMANDS whose next word is a verb (`wisp grid dev`, `wisp skill
/// install`), and plugins wisp's own words lead to (`machines`). `recipes` takes none. A group's plugin verbs from the
/// catalogue are its verbs too (`crew create`). Any other plugin's name isn't a group: `wisp review the auth module` is
/// a task (owner's delegate, QA round 10 #1).
const GROUPS: &[(&str, &[&str])] = &[
    ("grid", &["dev", "init", "serve", "doctor", "rotate-key", "backup", "users", "waitlist", "help", "deploy"]),
    ("plugin", PLUGIN_VERBS),
    ("plugins", PLUGIN_VERBS),
    ("skill", &["install"]),
    // the stdio MCP server: no verb (its catalogue commands are MCP's, not the CLI's)
    ("mcp", &[]),
    ("completions", &["zsh", "bash", "fish"]),
    ("sdk", &["types"]),
    ("recipes", &[]),
    ("crew", &["ls"]),
    ("machines", &[]),
    // QA round 14 #2: `wisp settings open` launched a task (its verbs: the settings plugin's)
    ("settings", &[]),
    // QA round 11 #1: words people type as commands, v1's included (`wisp usage`, `wisp spell`); `machine` is this
    // machine's plugin, with `machines`' verbs too; `share` takes a task or a setup, or one of its plugin's verbs
    ("version", &[]),
    ("update", &["apply"]),
    ("machine", &[]),
    ("share", &["<task id>", "<kind>.<name>"]),
    ("shares", &[]),
    ("usage", &[]),
    ("spell", &[]),
    ("spells", &[]),
    ("serve", &[]),
    ("desktop", &[]),
    ("tui", &[]),
    ("shell-init", &["zsh", "bash", "fish"]),
    ("schema", &[]),
];

/// A group whose catalogue verbs include another plugin's (`machine`: `machines`').
const ALSO: &[(&str, &str)] = &[("machine", "machines")];

/// A line under a group's error: where else to look.
const HINTS: &[(&str, &str)] = &[("share", "what you've shared: wisp shares")];

/// Whether these words are a built-in group's command line, never a task: the group and at most one word that isn't
/// a flag (`wisp update deps`, a slip), or one of its own verbs first (`wisp crew ls extra`, that command misused). Two
/// or more other words are a prompt (`wisp share the results with the team`): the owner's delegate's rule, QA round 14.
pub fn group_line(words: &[String]) -> bool {
    let Some((first, rest)) = words.split_first() else { return false };
    if !is_group(first) {
        return false;
    }
    let said: Vec<&String> = rest.iter().filter(|w| !flag_shaped(w)).collect();
    said.len() <= 1 || said.first().is_some_and(|v| own_verb(first, v))
}

/// Whether `word` is a built-in command group (strict: a verb it doesn't have is an error, never a task).
pub fn is_group(word: &str) -> bool {
    VERB_PATHS.iter().any(|p| p.cmd == word) || GROUPS.iter().any(|(g, _)| *g == word)
}

/// A command group's word followed by none of its verbs (`wisp computer start`): the group and its verbs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVerb {
    pub group: String,
    pub verbs: Vec<String>,
}

impl UnknownVerb {
    /// What the CLI says (exit 2): the verbs there are, and how to make the words a task anyway.
    pub fn said(&self, words: &[String]) -> String {
        let typed = words.get(1).map_or("", String::as_str);
        let have = if self.verbs.is_empty() {
            format!("  ✗ wisp {g} has no {typed}: it takes no verb (wisp {g})", g = self.group)
        } else {
            format!("  ✗ wisp {} has no {typed}: {}", self.group, self.verbs.join(", "))
        };
        let all = words.join(" ").replace('"', "\\\"");
        let hint = HINTS.iter().find(|(g, _)| *g == self.group).map(|(_, h)| format!("\n  {h}")).unwrap_or_default();
        format!("{have}{hint}\n  To start a task with these words: wisp -- \"{all}\"   (or quote them as one: wisp \"{all}\")")
    }
}

/// Setup audit F30 and B1: a plugin's name and one word that's none of its verbs is a slip, never a task, whatever the
/// word looks like (`wisp budget stop-at`, `wisp budget frobnicate`, `wisp learn rust`): the rule [`group_line`] holds
/// for the built-in groups, so a slip under any group never launches a Full-auto task. Two or more words are a prompt
/// (`wisp learn rust properly`); `wisp -- …` or one quoted argument is always the task.
fn plugin_slip(first: &str, rest: &[String], catalogue: &[Vec<String>]) -> bool {
    let verbs: Vec<&str> = catalogue.iter().filter(|c| c.len() > 1 && c[0] == first).map(|c| c[1].as_str()).collect();
    let said: Vec<&String> = rest.iter().filter(|w| !flag_shaped(w)).collect();
    let [word] = said.as_slice() else { return false };
    // one quoted argument (`wisp ship "the docs"`) is the task, as after every command word
    !verbs.is_empty() && !verbs.contains(&word.as_str()) && !word.contains(char::is_whitespace)
}

/// QA round 10 #1: the words start with a built-in command group (a [`VERB_PATHS`] or [`GROUPS`] word; `catalogue`,
/// each command's path such as `["crew", "create"]`, adds its plugin verbs) and the next word is none of its verbs: a
/// command line typed wrong, never a task. None: no group (another plugin's name included: a prompt), one of its
/// verbs, a flag (its own checks), one word alone, or `--` first. A prompt that starts with a group's word is one
/// quoted argument (`wisp "computer start fails"`) or follows `--`.
pub fn unknown_verb(words: &[String], literal: Literal, catalogue: &[Vec<String>]) -> Option<UnknownVerb> {
    if literal == Literal::Task {
        return None;
    }
    let (first, rest) = words.split_first()?;
    if rest.first().is_none_or(|w| flag_shaped(w)) {
        return None;
    }
    let slip = !is_group(first) && plugin_slip(first, rest, catalogue);
    if !is_group(first) && !slip {
        return None;
    }
    // a catalogue verb's first word counts as the group's own too (`wisp mcp session bogus`)
    let catalogue_own = catalogue.iter().any(|c| c.len() > 1 && c[0] == *first && rest.first().is_some_and(|w| *w == c[1]));
    if !slip && !group_line(words) && !catalogue_own {
        return None;
    }
    let mut paths: Vec<Vec<&str>> = Vec::new();
    for vp in VERB_PATHS.iter().filter(|p| p.cmd == first) {
        paths.extend(vp.verbs.iter().map(|v| vec![*v]));
    }
    for (_, verbs) in GROUPS.iter().filter(|(g, _)| g == first) {
        paths.extend(verbs.iter().map(|v| vec![*v]));
    }
    let also = ALSO.iter().find(|(g, _)| g == first).map(|(_, o)| *o);
    for c in catalogue.iter().filter(|c| c.len() > 1 && c.first().is_some_and(|n| n == first || Some(n.as_str()) == also)) {
        paths.push(c[1..].iter().map(String::as_str).collect());
    }
    // `<task id>`: a form, said in the list, never a word typed
    if paths.iter().any(|p| !p[0].starts_with('<') && rest.len() >= p.len() && rest.iter().zip(p).all(|(a, b)| a == b)) {
        return None;
    }
    let mut verbs: Vec<String> = Vec::new();
    for p in paths {
        let v = p.join(" ");
        if !verbs.contains(&v) {
            verbs.push(v);
        }
    }
    Some(UnknownVerb { group: first.clone(), verbs })
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
