//! The `wisp` command line (v1 bin/wisp.js): its usage text, flags and which word is a command. Pure:
//! main reads it before deciding whether anything else has to start.
use std::path::{Path, PathBuf};

pub const HELP: &str = "usage:
  wisp                          open wisp
  wisp \"task\"                   launch a task in this folder
  wisp serve                    run wisp here, in this terminal, without opening anything   (--port N  --host ADDR)
  wisp add [folder]             add a project: a folder here, a path or a git link (wisp add a README is a task)
  wisp \"task\" -d                launch it and return with its id (the default outside a terminal; --wait blocks)
  wisp attach <id>              follow a task live: its steps, its answer, its questions; a reply box after
                                (Ctrl-C detaches, a second Ctrl-C stops it)
  wisp ls                       list sessions        (the newest 50; --state running,waiting,review,done|needs  --limit N  --all, -a: all, archived too
                                --watch: follow the list)
  wisp show <id>                state + latest presentation as markdown   (--watch: follow its turn live)
  wisp wait <id>                block until it finishes or asks you something
  wisp reply <id> \"text\"        follow up (answers a pending question)   (--queue  --mode plan|edits|auto
                                --agent ID  --model ID  --effort LEVEL: switch harness / model from here on)
  wisp btw <id> \"question\"      ask on the side without interrupting the run; prints the answer
  wisp stop <id>                stop a run
  wisp held <id> start|without|skip   a turn held before it started: start it (Trust and start), run it without the
                                repo's own programs, or skip this run
  wisp ship <id>                commit, push and open a PR for a worktree session   (--draft)
  wisp merge <id>               squash-merge its PR and sync your local base branch
  wisp run <recipe> [values…]   run a saved recipe; values fill its {params} in order, or key=value
  wisp recipes                  list recipes
  wisp plugin                   list plugins
  wisp plugin new <name>        start a plugin of your own in ~/.wisp/plugins/<name>
  wisp plugin enable|disable|trust|untrust <id>   switch a plugin on or off; trust one you installed (its theme applies)
  wisp plugin install <folder|git URL>  install one, off, and see what it asks for; remove <id> takes it out
  wisp crew                     list your crew
  wisp init                     a .wisp/ folder here: wisp.yml, the AGENTS.md note, its modules fetched and locked
                                (asks first outside a git repo or project; --here: make it anyway)
  wisp plan|destroy             what applying .wisp/ would change; take out everything it declares   (--yes)
  wisp fmt                      format the .wisp/ folder   (--check)
  wisp workspace [list|show|select|new NAME]   the folder's workspaces: one set of variables each
  wisp output [name]|graph|mv FROM TO   the folder's outputs, its references as Graphviz, rename an address
  wisp track <id>               keep a task in this repo's .wisp/tasks, for you to commit   (--level spec|report|full)
  wisp untrack <id>             take a tracked task out again
  wisp log [path]               the tracked tasks, newest first; with a path, the ones that touched it
  wisp blame <file>:<line>      which task wrote that line, and what it was asked
  wisp resume <id>              a new task that carries on from where that one stopped, on today's code
  wisp adopt [<id>|--all]       bring in sessions you ran in a harness outside wisp   (--since 7d  --track)
  wisp notes [<task>] [--set section=text]   a task's Notes (goal, decisions, what failed, state …); --set rewrites one
  wisp fresh [<task>] [--print] [-f brief.md]   a new conversation on the same task, from a brief (--print shows it)
  wisp hook install|uninstall|status   a git hook that adds Wisp-Task: <id> to commits a task's agent makes
  wisp secret set|rm NAME       a secret your formulas reference as ${secret:NAME}: piped in or typed hidden, never an argument
  wisp import [v1 home]         bring a wisp v1 home (~/.wisp) in: its plan first, then on yes   (--dry-run  --yes)
  wisp apply -f wisp.yml        your setup as code: plan, diff, confirm   (--dry-run  --yes  --prune  --trust  --profile P)
  wisp diff|validate -f F       what apply would change; schema and checks   (wisp schema: the editor schema)
  wisp export [kinds…]          what's set up here, as a formula   (--include ui,account  --format yaml|json|toml)
  wisp task spec|export|formula|history <id>   what was asked, to share, the setup it ran with, its family
  wisp run -f x.task.yml        a shared task spec, on your code; its formula: ./wisp.yml, module.x or inline
                                (wisp run <workflow|task> [k=v]: by name, --wait)
  wisp run <task> --formula F   a task a formula declares, by its name, nothing applied (--formula F \"…\": a prompt)
  wisp rerun <id>               the same ask from the same commit   (--from latest  --formula f.yml[,g.yml]
                                --fill-from-snapshot  --steps N  --agent …  --last N --project P  --wait)
  wisp ls --agent|--crew|--workflow|--formula f.yml|--since 7d   tasks by what made them
  wisp history export --since 30d   every task as JSONL, scrubbed
  wisp share <id>               a signed .wisp-task, previewed with its redactions   (--level spec|report|full  --to cloud|file|gist
                                --access anyone|people:a,b|teams:t|password  --expires 7d  --publish)
  wisp share <kind>.<name>      a setup (with what it references) to wisp Cloud   (--to cloud  --access …  --publish)
  wisp shares                   what you've shared: access, views, imports, versions   (--revoke ID  --updates)
  wisp open <file|url|link>     a shared task or setup: its plan, then run, import or apply it   (--run  --their-setup  --compare  --import  --yes)
  wisp cloud signin|status|signout   wisp Cloud (or another grid: wisp cloud signin <url>)
  wisp cloud accounts|switch|remove  each grid account signed in to; switch or sign out of one
  wisp version                  print this wisp's version (also --version, -v)
  wisp update [apply]           is there a newer wisp? apply installs it   (wisp help update: check, view)
  wisp usage                    your limits this period and what each task took   (--agent ID: that harness's, by task)
  wisp machines|machine <verb>  your machines and this one: list, add, remove…   (wisp help machines)
  wisp spell|spells             v1's spells are plugins now: wisp plugin new|install
  wisp computer on|off|status   this computer on the grid, reachable from your other wisps   (on --grid <url>: signs in there first)
  wisp --on <machine> …         run any command on that machine (wisp ls --on studio, wisp \"task\" --on cloud)
  wisp desktop                  open the wisp app window
  wisp mcp                      run the MCP server (stdio) for other agents and editors
  wisp skill [install]          print / install the wisp agent skill   (--agent claude|codex|opencode|all  --project)
  wisp grid dev|init|serve|doctor  wisp Grid: wisp for many people on your server   (wisp grid help)
  wisp <plugin> <verb> …        any command of any plugin, flags from its arguments (wisp crew create --name Ada)
  wisp help [plugin [verb]]     every plugin's commands, a plugin's verbs, a verb's flags
  wisp completions zsh|bash|fish  shell completions from the same catalogue
  wisp shell-init zsh|bash|fish  a hook for your shell's rc file: wisp then knows each command you ran, its folder and
                                its exit code (kept 7 days on this machine; eval \"$(wisp shell-init zsh)\" in ~/.zshrc)
  wisp tasks --touching <path>|--tracked   the tracked tasks (.wisp/tasks) behind a path, or all of them   (--json)
  wisp sdk types ts|py [FILE]   typed commands for the SDKs (a .d.ts or a .pyi), from the same catalogue
  wisp -- ls the repo           a task that starts with a command's word; or quote it as one: wisp \"computer start fails\"
                                (after a command group, computer, cloud, grid, crew, plugin, settings…, one word it
                                doesn't have is an error naming its verbs; two or more are a task: wisp share the results
                                with the team. show, stop, wait, reply, attach and btw take a task's id or last; other
                                words after them are a task: wisp show me where the bug is)

launch options: --mode plan|edits|auto  --agent claude|codex|opencode|cursor|antigravity|grok|copilot  --model ID  --effort LEVEL
                --cwd DIR  --also DIR (repeatable)  --wait  --json  --no-worktree  --compare agent[:model],…: one task per harness
                --try subagents=haiku|sonnet|task  --try skills=off  run it twice on --agent, the second with
                that changed (repeatable or comma separated); Compare says what differed
                --goal NAME  keep going until a goal declared by name (goals: in a formula) is met
                --goal-text \"…\"  the same with what done means in words
                --formula f.yml[,g.yml]  run under these formulas, nothing applied (its agents, skills, connectors…)
                --factory [NAME]  run it through a software factory (spec, build, checks, review, ship); alone: default
                --yes  go ahead although it takes a usage window past your Ask near the limit line
terminal context: wisp \"check the error above\" attaches what just happened in this terminal: its last commands, their
                exit codes, the failing one's output (tmux, WezTerm, kitty, iTerm2, Terminal), the folder and git status,
                secrets masked; never runs a command again   (--context  --no-context  --show-context  --lines N)
options:        --port N  --browser  --no-open  --json  --version";

pub const COMMANDS: [&str; 26] = [
    "attach",
    "help",
    "completions",
    "sdk",
    "add",
    "ls",
    "list",
    "show",
    "wait",
    "reply",
    "btw",
    "held",
    "stop",
    "mcp",
    "skill",
    "ship",
    "merge",
    "run",
    "recipes",
    "plugin",
    "plugins",
    "crew",
    "grid",
    "serve",
    "import",
    "desktop",
];
/// What `wisp held <id> <action>` takes: runtime.held's actions.
pub const HELD_ACTIONS: [&str; 3] = ["start", "without", "skip"];
pub const LS_STATES: [&str; 4] = ["running", "waiting", "review", "done"];
/// wisp-grid's commands (apps/grid's CLI). A test keeps `wisp grid <word>` routing in step.
pub const GRID_SUB: [&str; 9] = ["dev", "init", "serve", "doctor", "rotate-key", "backup", "users", "waitlist", "help"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRun {
    pub agent: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Flags {
    pub open: bool,
    pub browser: bool,
    pub port: u16,
    /// --port was on the command line (not just `WISP_PORT)`: scripted commands then skip discovery
    pub port_given: bool,
    pub host: Option<String>,
    pub mode: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub cwd: Option<PathBuf>,
    pub also: Vec<PathBuf>,
    pub json: bool,
    pub wait: bool,
    pub yes: bool,
    pub queue: Option<bool>,
    pub state: Option<String>,
    pub limit: Option<u32>,
    pub project: bool,
    pub worktree: Option<bool>,
    pub compare: Option<Vec<CompareRun>>,
    pub tries: Vec<String>,
    /// `--goal <name>`: a goal declared by name (`goals:` in a formula, ADR-0110)
    pub goal: Option<String>,
    /// `--goal-text "…"`: what done means, in words (as `/goal …` in the prompt)
    pub goal_text: Option<String>,
    /// `--formula f.yml[,g.yml]`: the launch runs under these formulas, nothing applied (ADR-0111)
    pub formula: Vec<String>,
    /// `--factory [name]`: the task runs through a software factory ('' : the built-in `default`, ADR-0114)
    pub factory: Option<String>,
    pub draft: bool,
    pub days: Option<u32>,
    /// `--on <machine>`: the command runs on that peer (ADR-0041)
    pub on: Option<String>,
    pub all: bool,
    pub grid: Option<String>,
    pub notebook: bool,
    pub team: bool,
    pub kit: bool,
    pub gist: bool,
    pub dry_run: bool,
    /// `-d`: return at once with the id (the default outside a terminal)
    pub detach: bool,
    /// `--watch`: `ls` and `show` follow their topic
    pub watch: bool,
    /// terminal context (ADR-0023): `--context` / `--no-context`; None: when the prompt asks about what's above
    pub context: Option<bool>,
    pub show_context: bool,
    /// `--lines N`: how much of the terminal's output
    pub lines: Option<usize>,
    /// `--help` after words (`wisp crew create --help`)
    pub help: bool,
}

impl Flags {
    pub fn defaults(env_port: Option<&str>) -> Self {
        Self {
            open: true,
            browser: false,
            port: env_port.and_then(|p| p.parse().ok()).unwrap_or(wisp_api::DEFAULT_PORT),
            port_given: false,
            host: None,
            mode: None,
            agent: None,
            model: None,
            effort: None,
            cwd: None,
            also: Vec::new(),
            json: false,
            wait: false,
            yes: false,
            queue: None,
            state: None,
            limit: None,
            project: false,
            worktree: None,
            compare: None,
            tries: Vec::new(),
            goal: None,
            goal_text: None,
            formula: Vec::new(),
            factory: None,
            draft: false,
            days: None,
            on: None,
            all: false,
            grid: None,
            notebook: false,
            team: false,
            kit: true,
            gist: false,
            dry_run: false,
            detach: false,
            watch: false,
            context: None,
            show_context: false,
            lines: None,
            help: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Run,
    Version,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Literal {
    No,
    /// `--` came first: the words are a task even if the first is a command name
    Task,
    Yes,
}

#[derive(Debug, Clone)]
pub struct Parsed {
    pub action: Action,
    pub flags: Flags,
    pub words: Vec<String>,
    /// read by the tests and kept for callers that need to know a task was forced with `--`
    #[allow(dead_code)]
    pub literal: Literal,
    pub command: Option<&'static str>,
    /// the arguments with every `--flag=value` split in two (the grid parses its own from these)
    pub argv: Vec<String>,
}

pub fn expand(p: &str, home: &Path, cwd: &Path) -> PathBuf {
    let p = if p == "~" {
        home.to_path_buf()
    } else if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        home.join(rest)
    } else {
        PathBuf::from(p)
    };
    if p.is_absolute() { p } else { cwd.join(p) }
}

fn compare_list(v: &str) -> Vec<CompareRun> {
    v.split(',')
        .filter(|s| !s.is_empty())
        .map(|x| {
            let mut it = x.splitn(2, ':');
            CompareRun { agent: it.next().unwrap_or("").to_owned(), model: it.next().filter(|m| !m.is_empty()).map(str::to_owned) }
        })
        .collect()
}

/// Generated verbs with an `on` field of their own (crew members' wakes: v1's trigger kinds `done`, `ci`…): after
/// their words, `--on` is theirs, not the machine to run on.
const VERBS_WITH_ON: [[&str; 2]; 2] = [["crew", "create"], ["crew", "update"]];

fn verb_owns_on(words: &[String]) -> bool {
    VERBS_WITH_ON.iter().any(|v| words.len() >= 2 && words[0] == v[0] && words[1] == v[1])
}

/// The runs a `--compare` launch makes: `--agent` (with `--model`) first, then each `--compare` run (v1 launch.js
/// `[body, ...compare]`; audit B #14: `--agent claude --compare codex` launched only codex). Without `--agent` the
/// list is the runs; one already in the list isn't made twice.
pub fn compare_runs(f: &Flags) -> Vec<CompareRun> {
    let list = f.compare.clone().unwrap_or_default();
    let Some(agent) = f.agent.as_deref().filter(|a| !a.is_empty()) else { return list };
    let lead = CompareRun { agent: agent.to_owned(), model: f.model.clone() };
    if list.iter().any(|r| r.agent == lead.agent && (r.model == lead.model || lead.model.is_none())) {
        return list;
    }
    std::iter::once(lead).chain(list).collect()
}

/// v1 session ids are 12 hex characters; v2 ones are UUIDs (with or without dashes).
pub fn session_id(s: &str) -> bool {
    let hex = |s: &str| s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    (s.len() == 12 && hex(s))
        || (s.len() == 32 && hex(s))
        || (s.len() == 36 && s.split('-').map(str::len).eq([8, 4, 4, 4, 12]) && hex(&s.replace('-', "")))
}

/// `wisp add <x>`: a folder (there, or here), something that reads as a path (`./web`, `~/code/site`, `code/site`), or
/// a link to clone. `wisp add a README` is a prompt (QA round 13 #1 registered a project "a").
fn addable(w: &str, home: &Path, cwd: &Path) -> bool {
    let link = ["https:", "http:", "ssh:", "git:", "git@"].iter().any(|p| w.to_ascii_lowercase().starts_with(p));
    link || w == "." || w == ".." || w.starts_with("~/") || w.contains('/') || w.contains('\\') || expand(w, home, cwd).is_dir()
}

/// What names a task on the command line: its id (12 to 32 hex digits, or a UUID), a number, or `last` (the newest).
pub fn task_ref(w: &str) -> bool {
    let hex = (12..=32).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_hexdigit());
    w == "last" || session_id(w) || hex || (!w.is_empty() && w.bytes().all(|b| b.is_ascii_digit()))
}

/// Newer commands are also common first words of a task ("ship the docs", "usage of X"), so they only
/// count when the rest fits; `wisp run` falls back to a task when no recipe has that name. A command that takes a task
/// counts only when its argument names one (QA round 14: `wisp show me where the bug is` is a prompt).
fn fits(cmd: &str, w: &[String], home: &Path, cwd: &Path) -> bool {
    let n = w.len();
    let w1 = w.get(1).map_or("", String::as_str);
    match cmd {
        "grid" => n == 1 || GRID_SUB.contains(&w1),
        "ship" | "merge" => session_id(w1),
        "attach" => n == 2 && task_ref(w1),
        "show" | "stop" | "wait" | "reply" | "btw" => task_ref(w1),
        // `wisp held <id> start` (audit D4): only with a task and one of its three words; `wisp held up the release` is a task
        "held" => n == 3 && task_ref(w1) && HELD_ACTIONS.contains(&w.get(2).map_or("", String::as_str)),
        // groups that route as commands (QA round 15 #1): bare, a flag, one word (a slip, said in main.rs) or their own
        // verb; two or more other words are a prompt (`wisp plugin for prettier`)
        "plugin" | "plugins" | "mcp" | "skill" => n <= 2 || w1.starts_with('-') || crate::words::own_verb(cmd, w1),
        "add" => n == 1 || (n == 2 && addable(w1, home, cwd)),
        // `wisp help`, `wisp help crew create`, `wisp help runtime "queue order"` (the spelling its list prints): help
        // never starts a task, whatever follows (audit B #15 launched one); a task saying it is `wisp -- help …`. Its own
        // arm though the default is the same: a change to the default must not make help a prompt
        #[allow(clippy::match_same_arms)]
        "help" => true,
        "completions" => n == 2 && ["zsh", "bash", "fish"].contains(&w1),
        "sdk" => (n == 3 || n == 4) && w1 == "types" && ["ts", "py", "typescript", "python"].contains(&w.get(2).map_or("", String::as_str)),
        "recipes" | "serve" | "desktop" | "import" => n == 1,
        "run" => n > 1,
        "crew" => n == 1 || (w1 == "ls" && n == 2),
        _ => true,
    }
}

pub fn command_of(words: &[String], literal: Literal, home: &Path, cwd: &Path) -> Option<&'static str> {
    if literal == Literal::Task {
        return None;
    }
    let first = words.first()?;
    let cmd = COMMANDS.iter().find(|c| **c == first.as_str())?;
    fits(cmd, words, home, cwd).then_some(*cmd)
}

/// Whether a word can be a factory's name (letters, digits, - _ .), so `--factory` takes it rather than leaving it a word.
pub fn factory_name(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64 && !v.starts_with('-') && v.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c))
}

/// A flag that needs a value and got none.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0} needs a value — see wisp --help")]
pub struct UsageError(pub String);

/// Sets a flag; Ok(false): not a flag (a word).
fn set(f: &mut Flags, a: &str, value: &mut dyn FnMut() -> Result<String, UsageError>, home: &Path, cwd: &Path) -> Result<bool, UsageError> {
    match a {
        "--no-open" => f.open = false,
        "--browser" | "-b" => f.browser = true,
        "--port" | "-p" => {
            let v = value()?;
            f.port = v.parse().map_err(|_| UsageError(format!("{a} {v}: not a port")))?;
            f.port_given = true;
        }
        "--host" => f.host = Some(value()?),
        "--mode" | "-m" => f.mode = Some(value()?),
        "--agent" | "-a" => f.agent = Some(value()?),
        "--model" => f.model = Some(value()?),
        "--effort" => f.effort = Some(value()?),
        "--cwd" | "-C" => f.cwd = Some(expand(&value()?, home, cwd)),
        "--also" => f.also.push(expand(&value()?, home, cwd)),
        "--state" => f.state = Some(value()?),
        "--limit" => f.limit = value()?.parse().ok(),
        "--json" => f.json = true,
        "--wait" | "-w" => f.wait = true,
        "--yes" | "-y" => f.yes = true,
        "--queue" => f.queue = Some(true),
        "--project" => f.project = true,
        "--no-worktree" => f.worktree = Some(false),
        "--worktree" => f.worktree = Some(true),
        "--compare" => f.compare = Some(compare_list(&value()?)),
        "--try" => f.tries.push(value()?),
        "--goal" => f.goal = Some(value()?),
        "--goal-text" => f.goal_text = Some(value()?),
        "--formula" => f.formula.extend(value()?.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)),
        "--draft" => f.draft = true,
        "--on" => f.on = Some(value()?),
        "--all" => f.all = true,
        "--days" => f.days = value()?.parse().ok(),
        "--grid" => f.grid = Some(value()?),
        "--notebook" => f.notebook = true,
        "--team" => f.team = true,
        "--no-kit" => f.kit = false,
        "--gist" => f.gist = true,
        "--dry-run" => f.dry_run = true,
        "--detach" | "-d" => f.detach = true,
        "--watch" => f.watch = true,
        "--context" => f.context = Some(true),
        "--no-context" => f.context = Some(false),
        "--show-context" => {
            f.show_context = true;
            f.context = Some(true);
        }
        "--lines" => {
            let v = value()?;
            f.lines = Some(v.parse().map_err(|_| UsageError(format!("--lines {v}: a number of lines")))?);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn parse(input: &[String], env_port: Option<&str>, home: &Path, cwd: &Path) -> Result<Parsed, UsageError> {
    let mut argv: Vec<String> = Vec::with_capacity(input.len());
    let mut flags = Flags::defaults(env_port);
    let mut words = Vec::new();
    let mut literal = Literal::No;
    // --state=review is --state review (it used to become a word, and the filter was dropped)
    for a in input {
        match a.split_once('=') {
            Some((k, v))
                if literal == Literal::No
                    && k.starts_with("--")
                    && k.len() > 2
                    && k[2..].bytes().all(|b| b.is_ascii_lowercase() || b == b'-') =>
            {
                argv.push(k.to_owned());
                argv.push(v.to_owned());
            }
            _ => argv.push(a.clone()),
        }
    }
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        if literal != Literal::No {
            words.push(a);
        } else if a == "--" {
            literal = if words.is_empty() { Literal::Task } else { Literal::Yes };
        } else if a == "--version" || a == "-v" {
            return Ok(Parsed { action: Action::Version, flags, words, literal, command: None, argv });
        } else if (a == "--help" || a == "-h") && words.is_empty() {
            return Ok(Parsed { action: Action::Help, flags, words, literal, command: None, argv });
        } else if a == "--help" || a == "-h" {
            // after words: a generated verb's own help (`wisp crew create --help`), else the usage
            flags.help = true;
        } else {
            let mut next = || {
                i += 1;
                argv.get(i).cloned().ok_or_else(|| UsageError(a.clone()))
            };
            // `wisp ls -a` is --all (QA round 15 #6 said "-a needs a value"); `-a` is --agent everywhere else
            if a == "-a" && matches!(words.first().map(String::as_str), Some("ls" | "list")) {
                flags.all = true;
            } else if a == "--factory" {
                // `--factory` alone is `default`: the next word is its name only when it can be one (no spaces, no flag)
                let name = argv.get(i + 1).filter(|v| factory_name(v)).cloned();
                if name.is_some() {
                    i += 1;
                }
                flags.factory = Some(name.unwrap_or_default());
            } else if a == "--on" && verb_owns_on(&words) {
                // `wisp crew create --on done`: the verb's own `--on` (its wakes), read from argv by the verb; the
                // machine to run on goes first (`wisp --on laptop crew create …`) (audit B #9)
                next()?;
            } else if !set(&mut flags, &a, &mut next, home, cwd)? {
                words.push(a);
            }
        }
        i += 1;
    }
    // `-C dir` is where `wisp add <folder>` looks
    let at = flags.cwd.clone().unwrap_or_else(|| cwd.to_owned());
    let command = command_of(&words, literal, home, &at);
    Ok(Parsed { action: Action::Run, flags, words, literal, command, argv })
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
