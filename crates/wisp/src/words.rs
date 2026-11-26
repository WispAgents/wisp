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
const TASK_COMMANDS: [&str; 7] = ["show", "stop", "wait", "reply", "attach", "btw", "held"];

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
    // the scripted commands' switches (`commands::own_flags`): `ls --all --bogus` must not read --bogus as a value
    "--all",
    "--watch",
    "--draft",
    "--queue",
    "--detach",
];

fn takes_value(cmd: &str, flag: &str) -> bool {
    let follow = cmd == "events" && flag == "-f";
    // `wisp ls --repo` (the repo's tracked tasks) and `wisp ls -a` (--all) are switches; `wisp mcp --repo <dir>` takes a
    // folder, and `-a` elsewhere is --agent
    let ls_repo = matches!(cmd, "ls" | "list") && (flag == "--repo" || flag == "-a");
    !follow && !ls_repo && !SWITCHES.contains(&flag)
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
    vp("trigger", &["add", "ls", "list", "rm", "remove", "off", "on", "fire", "test", "log", "resume", "approve", "sources"], TRIGGER),
    // the alias takes the same verbs, `off`/`on` included (audit H HS2: its refusal and completion lacked them); each
    // list written out, as serve_cli_verbs reads this table from its source (words_tests keeps the two the same)
    vp("triggers", &["add", "ls", "list", "rm", "remove", "off", "on", "fire", "test", "log", "resume", "approve", "sources"], TRIGGER),
    vp("crew", &["wake", "graph", "pause", "resume"], TRIGGER),
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

/// Auto put away (`runtime.auto` off, audit C6): `wisp router <any words>` is refused like a slip (exit 2, the
/// `wisp -- "…"` way out), never read as a prompt: a former command word must not launch a paid run. `away`: the
/// router's verbs aren't offered. None when the words don't start with `router`, or Auto is on.
pub fn router_away(words: &[String], literal: Literal, away: bool) -> Option<String> {
    if !away || literal != Literal::No || words.first().map(String::as_str) != Some("router") {
        return None;
    }
    let all = words.join(" ").replace('"', "\\\"");
    // the slip's own words (`wisp router has no …`), then why it has none
    let typed = words.get(1).map_or("", String::as_str);
    Some(format!(
        "  ✗ wisp router has no {typed}: Auto is off (runtime.auto), so the router is put away\n  To start a task with these words: wisp -- \"{all}\"   (or quote them as one: wisp \"{all}\")"
    ))
}

/// A plugin that's switched off (audit F setup F1): `wisp settings get crew.tint` with `wisp.settings` off fell through to
/// a prompt and launched a Full-auto task. The words name the plugin (`settings` is `wisp.settings`, a user plugin's
/// last id part) and it isn't on: refused with its state, like a slip (exit 2, the `wisp -- "…"` way out), never a
/// task. `off`: the plugins that aren't on, each with why (`plugins.list` rows: id, reason). None: no such plugin.
pub fn plugin_off(words: &[String], literal: Literal, off: &[(String, Option<String>)]) -> Option<String> {
    let first = words.first().filter(|_| literal == Literal::No)?;
    let (id, why) = off.iter().find(|(id, _)| id.rsplit('.').next() == Some(first.as_str()))?;
    let all = words.join(" ").replace('"', "\\\"");
    let why = why.as_deref().filter(|w| !w.is_empty()).map_or_else(String::new, |w| format!(" ({w})"));
    Some(format!(
        "  ✗ wisp {first} is off: {id} is switched off{why}. Switch it on: wisp plugin enable {id}\n  To start a task with these words: wisp -- \"{all}\"   (or quote them as one: wisp \"{all}\")"
    ))
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
    /// where the verb that isn't one sits in the words: 1 (`crew pasue Zed`), 2 for `held <id> <action>`
    pub at: usize,
}

impl UnknownVerb {
    /// The verb it's closest to, when it reads as a slip of one (`pasue` → `pause`): the closest wins, the list's order
    /// breaks ties.
    #[must_use]
    pub fn close(&self, words: &[String]) -> Option<String> {
        let typed = words.get(self.at)?.to_lowercase();
        self.verbs
            .iter()
            .filter(|v| !v.contains(' ') && !v.starts_with('<'))
            .map(|v| (distance(&typed, v), v))
            // a short verb's last letter dropped counts too (`of` is `off`, never `on`)
            .filter(|(d, v)| *d <= 2 && (near(&typed, v, *d) || (*d == 1 && typed.len() >= 2 && v.starts_with(typed.as_str()))))
            .min_by_key(|(d, _)| *d)
            .map(|(_, v)| v.clone())
    }

    /// What the CLI says (exit 2): the verb it's closest to, the verbs there are, and how to make the words a task anyway.
    pub fn said(&self, words: &[String]) -> String {
        let typed = words.get(self.at).map_or("", String::as_str);
        if self.group == "git" && GIT_OWN.contains(&typed) {
            let all = words.join(" ").replace('"', "\\\"");
            let line = words[self.at..].join(" ");
            return format!(
                "  ✗ wisp git has no {typed}: that's git's own command. Run it with git itself: git {line}\n  wisp git's own: {}\n  To start a task with these words: wisp -- \"{all}\"   (or quote them as one: wisp \"{all}\")",
                self.verbs.join(", ")
            );
        }
        let close = self.close(words).map(|v| format!(" (did you mean {v}?)")).unwrap_or_default();
        let have = if self.verbs.is_empty() {
            format!("  ✗ wisp {g} has no {typed}{close}: it takes no verb (wisp {g})", g = self.group)
        } else {
            format!("  ✗ wisp {} has no {typed}{close}: {}", self.group, self.verbs.join(", "))
        };
        let all = words.join(" ").replace('"', "\\\"");
        let hint = HINTS.iter().find(|(g, _)| *g == self.group).map(|(_, h)| format!("\n  {h}")).unwrap_or_default();
        format!("{have}{hint}\n  To start a task with these words: wisp -- \"{all}\"   (or quote them as one: wisp \"{all}\")")
    }
}

/// Setup audit F30 and B1, then audit I2: a plugin's name and a next word that's none of its verbs is a slip, never a
/// task, whatever follows (`wisp goal sett abc never`, `wisp later cancle x`): a slip never launches a Full-auto task.
/// The words a prompt may start with ([`PROMPT_WORDS`]: `wisp ship the docs`) stay a prompt; `wisp -- …` or one quoted
/// argument is always the task.
fn plugin_slip(first: &str, rest: &[String], catalogue: &[Vec<String>]) -> bool {
    let verbs: Vec<&str> = catalogue.iter().filter(|c| c.len() > 1 && c[0] == first).map(|c| c[1].as_str()).collect();
    let Some(word) = rest.first() else { return false };
    // one quoted argument (`wisp ship "the docs"`) is the task, as after every command word
    !prompt_line(first, rest) && !verbs.is_empty() && !verbs.contains(&word.as_str()) && !word.contains(char::is_whitespace)
}

/// A word a prompt may start with, followed by more than one word (`wisp ship the docs`): a prompt. One word after it is
/// still a slip (`wisp ship foo`).
fn prompt_line(first: &str, rest: &[String]) -> bool {
    PROMPT_WORDS.contains(&first) && rest.iter().filter(|w| !flag_shaped(w)).count() > 1
}

/// git's own commands that `wisp git` doesn't have (audit I IS4): after `wisp git` one of them is a command line for git,
/// refused with a pointer to plain git, never read as a prompt (`wisp git push origin main` launched a Full-auto task).
/// The ones wisp git has (`checkout`, `clone`, `commit`, `revert`, `status`) are its own verbs.
pub const GIT_OWN: &[&str] = &[
    "add",
    "am",
    "apply",
    "archive",
    "bisect",
    "blame",
    "branch",
    "bundle",
    "cherry",
    "cherry-pick",
    "clean",
    "config",
    "describe",
    "diff",
    "difftool",
    "fetch",
    "format-patch",
    "fsck",
    "gc",
    "grep",
    "init",
    "log",
    "ls-files",
    "ls-remote",
    "merge",
    "mergetool",
    "mv",
    "notes",
    "pull",
    "push",
    "range-diff",
    "rebase",
    "reflog",
    "remote",
    "reset",
    "restore",
    "rev-parse",
    "rm",
    "shortlog",
    "show",
    "sparse-checkout",
    "stash",
    "submodule",
    "switch",
    "tag",
    "worktree",
];

/// Command words a prompt may naturally start with (owner's decision on audit I2, 2026-11-20): `wisp review the auth
/// module`, `wisp plugin for prettier`, `wisp crew a reviewer for the PR`, `wisp update the readme`. Under one of these an
/// unknown next word is a slip only when it reads as a slip of one of its verbs ([`slip_of`]) or the line is the word and
/// one more; otherwise the words are a prompt, as before audit I2. The other groups (`computer`, `cloud`, `sandbox`,
/// `team`, `task`, `grid`, `hook`, `events`, `mcp`, `completions`, `sdk`, `recipes`, `machines`, `skill`) and every other
/// plugin's name stay strict: their words rarely start a sentence, and a slip there must never launch a Full-auto task.
const PROMPT_GROUPS: &[&str] = &[
    "review", "plugin", "plugins", "crew", "goal", "factory", "later", "trigger", "triggers", "held", "update", "share", "settings", "git",
    "learn", "budget",
];

/// Whether `typed` reads as a slip of `verb`: within 2 edits (an adjacent swap is one) that leave most of it standing,
/// one edit from 3 letters and two from 5 (`pasue`, `sett`, `stp`, `cancle`; never `the` for `set`, `fix` for `fire` or
/// `rust` for `run`), or the start of it (`of` for `off`, `paus` for `pause`), two letters at least.
fn slip_of(typed: &str, verb: &str) -> bool {
    let typed = typed.to_lowercase();
    let n = typed.chars().count();
    let d = distance(&typed, verb);
    (d <= 2 && 2 * d < n) || (n >= 2 && verb.starts_with(typed.as_str()))
}

/// Under a [`PROMPT_GROUPS`] word, whether the words are a slip rather than a prompt: the word and one more, or a next
/// word that's a slip of one of `verbs` (each a path such as `set` or `session set`: its first word counts). Under any
/// other group: always.
fn slip_line(first: &str, said: &[&String], at: usize, verbs: &[String]) -> bool {
    if !PROMPT_GROUPS.contains(&first) || said.len() <= 2 {
        return true;
    }
    let Some(typed) = said.get(at) else { return true };
    verbs.iter().filter_map(|v| v.split(' ').next()).filter(|v| !v.starts_with('<')).any(|v| slip_of(typed, v))
}

/// `wisp held <id> <action>` with an action that's a slip of one of its three (`wisp held x strat`, audit I2): a slip;
/// one that's none (`wisp held back changes`) leaves the words a prompt ([`PROMPT_GROUPS`]).
fn held_slip(words: &[String]) -> Option<UnknownVerb> {
    let said: Vec<&String> = words.iter().filter(|w| !flag_shaped(w)).collect();
    let [first, _, action] = said.as_slice() else { return None };
    let verbs: Vec<String> = crate::args::HELD_ACTIONS.iter().map(|v| (*v).to_owned()).collect();
    (first.as_str() == "held"
        && !crate::args::HELD_ACTIONS.contains(&action.as_str())
        && !action.contains(char::is_whitespace)
        && slip_line("held", &said, 2, &verbs))
    .then(|| UnknownVerb { group: "held".into(), verbs, at: 2 })
}

/// A line that can only be a task's command but names no task: `wisp held x start` (one of held's own actions after a
/// word that's no task) or `wisp merge x` / `wisp ship x` (the command and one word). The word, said as `no task <word>`
/// (exit 2): `wisp held x start` launched a Full-auto task with that line as its prompt. `wisp held up the release`,
/// `wisp ship the docs to prod` and one quoted argument (`wisp ship "the docs"`) stay prompts.
pub fn no_task(words: &[String], literal: Literal) -> Option<String> {
    if literal != Literal::No {
        return None;
    }
    let said: Vec<&String> = words.iter().filter(|w| !flag_shaped(w)).collect();
    let named = match said.as_slice() {
        [held, w, action] if held.as_str() == "held" && crate::args::HELD_ACTIONS.contains(&action.as_str()) => *w,
        [cmd, w] if matches!(cmd.as_str(), "merge" | "ship") && !w.contains(char::is_whitespace) => *w,
        _ => return None,
    };
    (!crate::args::task_ref(named)).then(|| named.clone())
}

/// QA round 10 #1, then audit I2 (owner's delegate, 2026-11-20, refined by the owner the same day): the words start with
/// a built-in command group (a [`VERB_PATHS`] or [`GROUPS`] word; `catalogue`, each command's path such as
/// `["crew", "create"]`, adds its plugin verbs) or a plugin's name, and the next word is none of its verbs: a command
/// line typed wrong, never a task, however many words follow (`wisp crew pasue Zed` launched one); under a
/// [`PROMPT_GROUPS`] word only when the line is two words or the word is a slip of a verb ([`slip_line`]). None: no group, one of its verbs, a flag (its own
/// checks), one word alone, a word a prompt may start with ([`PROMPT_WORDS`]), or `--` first. A prompt that starts
/// with a group's word is one quoted argument (`wisp "computer start fails"`) or follows `--`.
pub fn unknown_verb(words: &[String], literal: Literal, catalogue: &[Vec<String>]) -> Option<UnknownVerb> {
    if literal == Literal::Task {
        return None;
    }
    if let Some(h) = held_slip(words) {
        return Some(h);
    }
    let (first, rest) = words.split_first()?;
    if rest.first().is_none_or(|w| flag_shaped(w) || w.contains(char::is_whitespace)) {
        return None;
    }
    // `wisp git push origin main`: one of git's own commands is a command line for git, never a task (audit I IS4: it
    // launched a Full-auto task with that line as its prompt), however many words follow
    let git_own = first == "git" && rest.first().is_some_and(|w| GIT_OWN.contains(&w.as_str()));
    let group = is_group(first) && !prompt_line(first, rest);
    let slip = git_own || (!is_group(first) && plugin_slip(first, rest, catalogue));
    if !group && !slip {
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
    // a word a prompt may start with, a far word and more after it: a prompt (owner, audit I2 refined)
    let said: Vec<&String> = words.iter().filter(|w| !flag_shaped(w)).collect();
    if !git_own && !slip_line(first, &said, 1, &verbs) {
        return None;
    }
    Some(UnknownVerb { group: first.clone(), verbs, at: 1 })
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

/// `wisp run <name>` that names nothing (audit H11: it named only recipes, though `wisp run` runs declared tasks and
/// workflows too): each kind it looked for, and the command that lists it. `wisp recipes` lists the declared tasks (a
/// recipe is one with `{params}`), `wisp workflows list` the workflows (wisp.workflows' own verb).
pub fn run_miss(name: &str) -> String {
    format!(
        "no declared task, recipe or workflow named {name}: `wisp recipes` lists the declared tasks and recipes, `wisp workflows list` the workflows"
    )
}

/// A prompt with a `--word` no launch flag takes (audit H2: `wisp "fix it" --mdoe plan` launched in Full auto with the
/// typo in its prompt): the refusal, naming the close flag. Nothing launches; a prompt that says it puts it after `--`.
pub fn launch_stray(prompt: &[String], stray: &str) -> String {
    let typed = stray.trim_start_matches('-');
    let near_ones: Vec<(usize, &str)> = crate::args::LAUNCH_FLAGS
        .iter()
        .map(|f| (distance(typed, f.trim_start_matches('-')), *f))
        .filter(|(d, f)| *d <= 2 && near(typed, f.trim_start_matches('-'), *d))
        .collect();
    // every flag as close as the closest: `--modle` is one slip from --mode and from --model (audit H HS3)
    let best = near_ones.iter().map(|(d, _)| *d).min();
    let tied: Vec<&str> = near_ones.iter().filter(|(d, _)| Some(*d) == best).map(|(_, f)| *f).collect();
    let close = match tied.as_slice() {
        [] => String::new(),
        [one] => format!(": did you mean {one}?"),
        [rest @ .., last] => format!(": did you mean {} or {last}?", rest.join(", ")),
    };
    let later = if close.is_empty() && matches!(typed, "at" | "later" | "when" | "start-at") {
        ". A task can't start later from here: Start later in the app, or wisp later plan --help"
    } else {
        ""
    };
    // the way to say it as words: what came before the flag, then `--` and the rest as typed
    let at = prompt.iter().position(|w| w == stray).unwrap_or(prompt.len());
    let (before, after) = prompt.split_at(at);
    let before = if before.is_empty() { String::new() } else { format!("\"{}\" ", before.join(" ")) };
    format!(
        "wisp takes no {stray}{close}{later}\n    Nothing was launched (wisp --help lists the flags). A task that says it: wisp {before}-- {}",
        after.join(" ")
    )
}

/// `--mode <m>` that names no mode (audit H HS3: "runtime.create: unknown variant `opus`, expected one of …"): the
/// refusal in words, pointing a model at `--model`. None: a mode.
pub fn bad_mode(m: &str) -> Option<String> {
    (!matches!(m, "plan" | "edits" | "auto"))
        .then(|| format!("--mode takes plan, edits or auto, not {m} (a model goes after --model: --model {m}). Nothing was launched"))
}

/// The thinking levels any harness takes (each harness takes some of them; the server checks its own).
pub const EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `--effort banana` (audit I9): launched in Full auto, and the harness got `--effort banana`, which a real one refuses
/// at run time. Refused before launching, in words.
pub fn bad_effort(e: &str) -> Option<String> {
    let e = e.trim();
    (!e.is_empty() && !EFFORTS.contains(&e)).then(|| {
        let mode = if matches!(e, "plan" | "edits" | "auto") { format!(" ({e} is a mode: --mode {e})") } else { String::new() };
        format!("--effort takes minimal, low, medium, high, xhigh or max, not {e}{mode}. Nothing was launched")
    })
}

/// `--model plan` (audit I9): a mode or a thinking level given as a model. Refused before launching, in words.
pub fn bad_model(m: &str) -> Option<String> {
    let m = m.trim();
    if matches!(m, "plan" | "edits" | "auto") {
        return Some(format!("{m} is a mode, not a model: --mode {m}. Nothing was launched"));
    }
    EFFORTS.contains(&m).then(|| format!("{m} is a thinking level, not a model: --effort {m}. Nothing was launched"))
}

/// The first of `--mode`, `--effort` and `--model` given a word it doesn't take, in words (audit I9).
pub fn bad_launch_words(mode: Option<&str>, effort: Option<&str>, model: Option<&str>) -> Option<String> {
    mode.and_then(bad_mode).or_else(|| effort.and_then(bad_effort)).or_else(|| model.and_then(bad_model))
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
    // a group HELP has no line for (`wisp trigger`, `wisp triggers`; audit H HS2): its verbs, not "needs more"
    let verbs: Vec<&str> = VERB_PATHS.iter().filter(|p| p.cmd == word).flat_map(|p| p.verbs.iter().copied()).collect();
    if out.is_empty() && !verbs.is_empty() {
        out = format!("  wisp {word} {} …\n", verbs.join("|"));
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

/// The built-in commands no plugin adds verbs to: their `--help` is their usage, read without the catalogue, so it never
/// starts wisp nor runs the command, whatever words follow (audit I IS10: `wisp import --help` looked for a folder named
/// `--help`; audit H1: `wisp destroy --help` planned a destroy, `wisp workspace new --help` made a workspace `--help`).
/// The other built-ins' `--help` is `wisp help <it>` with their plugin's verbs (audit G GS10), or their verb's own
/// (`wisp crew create --help`). `words_tests` holds every one to a usage of its own and to no plugin of its name.
pub const OWN_HELP: &[&str] = &[
    "add",
    "adopt",
    "apply",
    "attach",
    "blame",
    "btw",
    "desktop",
    "destroy",
    "diff",
    "export",
    "fmt",
    "fresh",
    "graph",
    "held",
    "import",
    "init",
    "list",
    "log",
    "ls",
    "merge",
    "mv",
    "notes",
    "open",
    "output",
    "plan",
    "reply",
    "rerun",
    "resume",
    "run",
    "schema",
    "secret",
    "serve",
    "shell-init",
    "show",
    "stop",
    "tasks",
    "track",
    "tui",
    "untrack",
    "validate",
    "version",
    "wait",
    "workspace",
];

/// What `wisp <built-in> … --help` is, decided before any command runs (audit H1: `wisp destroy --help` planned a
/// destroy, `wisp graph --help` printed the digraph, `wisp workspace new --help` made a workspace named `--help`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskedHelp {
    /// the command's usage, whatever words follow: print it, exit 0
    Usage(String),
    /// one word, or a next word that may be its plugin's verb (`wisp ship merge --help`): the catalogue decides
    /// ([`crate::cli::help_one`], or that verb's own help), the usage when nothing answers
    Catalogue,
}

/// None: no `--help`, after `--` (a task: `wisp -- "explain --help"`), a word that's no built-in (a plugin's: `wisp
/// review --help` is `wisp help review`), `wisp help …` (help already) or `wisp grid …` (wisp Grid's own help).
pub fn asked_help(words: &[String], literal: Literal, help: bool) -> Option<AskedHelp> {
    let first = words.first()?;
    if literal == Literal::Task || !help || !known(first) || matches!(first.as_str(), "help" | "grid") {
        return None;
    }
    // `wisp computer on --help`: the group's usage, as its refusals print it
    if let Some((cmd, _)) = verb_path(words) {
        return Some(AskedHelp::Usage(usage(cmd)));
    }
    if !OWN_HELP.contains(&first.as_str()) && words.get(1).is_none_or(|w| crate::plugin_word(w)) {
        return Some(AskedHelp::Catalogue);
    }
    Some(AskedHelp::Usage(documented(first).unwrap_or_else(|| usage(first))))
}

/// A command's usage, then its plugin's verbs under `more:` (audit M5: `wisp ship --help` listed only the ship plugin's
/// verbs, never `wisp ship <id>` and its `--draft`).
pub fn with_verbs(usage: &str, verbs: &str) -> String {
    format!("{usage}\n\nmore:\n{verbs}")
}

/// `wisp <word>`'s usage when HELP documents it as a command of its own (its lines, never "needs more"), else None.
pub fn documented(word: &str) -> Option<String> {
    if word.starts_with('-') || word.is_empty() {
        return None;
    }
    let u = usage(word);
    (!u.contains(" needs more: ")).then_some(u)
}

/// Why a file couldn't be read, in words (audit D9: "No such file or directory (os error 2)"), as Projects and Plugins
/// say a missing folder ("No folder at …").
pub fn unreadable(path: &std::path::Path, e: &std::io::Error) -> String {
    use std::io::ErrorKind;
    let p = path.display();
    match e.kind() {
        ErrorKind::NotFound => format!("No file at {p}"),
        ErrorKind::IsADirectory => format!("{p} is a folder, not a file"),
        ErrorKind::PermissionDenied => format!("Can't read {p}: permission denied"),
        ErrorKind::InvalidData => format!("{p} isn't text (UTF-8)"),
        _ => format!("Can't read {p}: {}", e.kind()),
    }
}

/// `wisp import`'s v1 home without a `config.json` it can read (audit D9), in words.
pub fn no_v1_home(v1: &std::path::Path, file: &std::path::Path, e: &std::io::Error) -> String {
    let how = "(wisp import [home folder])";
    if !v1.is_dir() {
        format!("No folder at {} {how}", v1.display())
    } else if e.kind() == std::io::ErrorKind::NotFound {
        format!("{} has no config.json: is it a wisp home (~/.wisp)? {how}", v1.display())
    } else {
        format!("{} {how}", unreadable(file, e))
    }
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;
