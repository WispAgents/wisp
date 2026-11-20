//! wisp-factory-core: a factory run's decisions, without IO (ADR-0114). [`Run`] is a machine: the host feeds it what
//! happened ([`Event`]) and does what it says next ([`Next`]): make or follow up the stations' tasks, ask you, run the
//! checks, ship, merge, or end. It keeps where the run stands as a [`RunView`] for the parent task.
//!
//! The line is spec → build → checks → review → ship, each station the factory has. A failed check, a review the vote
//! doesn't approve, or a build that failed goes back to `loop.back_to` (a follow-up with the findings) as the next
//! round, up to `loop.max`; past it, `then: ask` asks you and `then: stop` ends the run.
//!
//! size-ok: one machine with its prompts and its tests (every edge case of ADR-0114 is a test here)

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_factory_api::{FactorySpec, MAX_ROUNDS, RunState, RunView, StationState, StationView, Waiting};

/// A task's goal as a run reads it (ADR-0110): what done means, its judges (agents), its vote and its round limit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub text: String,
    pub judges: Vec<String>,
    pub vote: String,
    pub max_rounds: Option<u32>,
}

/// What a run starts from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    pub factory: String,
    pub spec: FactorySpec,
    pub prompt: String,
    pub title: String,
    /// the agent the task named ('' : your default)
    pub agent: String,
    /// the run's own mode (plan | edits | auto; '' : the default)
    pub mode: String,
    /// the task's worktree pick (None: the default, a worktree in a project)
    pub worktree: Option<bool>,
    pub goal: Option<Goal>,
    /// the project's own test command ('' : none)
    pub test: String,
    /// an agent's token started it (ADR-0114 rule 7): stations at most at its mode, never a merge by itself
    pub by_agent: bool,
    /// the run is in a sandbox: its checks run there without asking (review r194 factories #3)
    pub sandbox: bool,
}

/// What a check command sees of wisp's environment (review r194 factories #3): everything but wisp's own variables
/// (`WISP_TOKEN`, `WISP_URL`, …), the harnesses' homes, and any token, secret, password, key or credential, by name. The
/// built code runs; nothing it runs can act as wisp or as you.
pub fn check_env(vars: Vec<(String, String)>) -> Vec<(String, String)> {
    const HOMES: [&str; 5] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "COPILOT_HOME", "GROK_HOME", "GEMINI_HOME"];
    // whole words of the name (`GIT_AUTHOR_NAME` keeps its AUTHOR), and the two-word ones
    const WORDS: [&str; 8] = ["TOKEN", "SECRET", "SECRETS", "PASSWORD", "PASSWD", "AUTH", "CREDENTIAL", "CREDENTIALS"];
    const PAIRS: [&str; 4] = ["API_KEY", "APIKEY", "PRIVATE_KEY", "ACCESS_KEY"];
    vars.into_iter()
        .filter(|(k, _)| {
            let up = k.to_ascii_uppercase();
            let words = || up.split('_');
            !up.starts_with("WISP_")
                && !HOMES.contains(&up.as_str())
                && !words().any(|w| WORDS.contains(&w))
                && !PAIRS.iter().any(|p| up.contains(p))
                && !up.ends_with("_KEY")
        })
        .collect()
}

/// One task a station starts or follows up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStep {
    Start {
        station: &'static str,
        /// its place among the station's tasks (the second builder is 1)
        index: usize,
        agent: String,
        title: String,
        prompt: String,
        mode: String,
        worktree: Option<bool>,
        /// it runs in this task's folder (a reviewer reads the build's worktree)
        folder_of: Option<String>,
    },
    FollowUp {
        task: String,
        text: String,
    },
}

/// What the host does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// make or follow up these tasks; then [`Event::Ended`] with how each ended, in this order
    Tasks(Vec<TaskStep>),
    /// ask on the parent task; then [`Event::Answered`]
    /// `context`: what the question is about, shown with it (the spec it asks you to approve, audit G5)
    Ask { question: String, options: Vec<String>, context: String },
    /// run the commands in `task`'s folder, in order, stopping at the first that fails; then [`Event::Checked`]
    Checks { task: String, commands: Vec<String>, minutes: u64 },
    /// `ship.ship` on `task`; then [`Event::Shipped`]
    Ship { task: String, title: String, body: String, draft: bool },
    /// `ship.merge` on `task`; then [`Event::Merged`]
    Merge { task: String },
    /// the run is over
    End { state: RunState, why: String },
}

/// How one task ended.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskEnd {
    pub task: String,
    /// its last turn was done without an error
    pub ok: bool,
    /// its last answer's text (or why it failed)
    pub text: String,
    pub cost: f64,
}

/// How one check command ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckRun {
    pub command: String,
    pub code: i32,
    /// its output's last lines
    pub tail: String,
    pub timed_out: bool,
}

/// What happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// every task of the last [`Next::Tasks`] ended, in its order
    Ended(Vec<TaskEnd>),
    Answered(String),
    Checked(Vec<CheckRun>),
    Shipped {
        ok: bool,
        pr: String,
        why: String,
    },
    /// `local`: what the merge says of your checkout when it couldn't bring it up to date ("Local main was not
    /// updated: …"), '' when it did (audit G20: the run said only "merged")
    Merged {
        ok: bool,
        why: String,
        local: String,
    },
    /// the host couldn't do what it was told (a task it couldn't make): the run ends failed
    Failed(String),
    /// you pressed Stop, or the task went
    Stop(String),
}

/// What a reviewer said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub reviewer: String,
    pub approve: bool,
    /// what to fix (empty when approved)
    pub why: String,
}

/// A reviewer's answer read (review r194 factories #2): only its own final non-empty line, the presentation block left
/// out, trimmed: exactly `APPROVE` is a yes, `CHANGES: <what to fix>` a no with what to fix (any case). Anything else,
/// a negation, a quote, the word mid-line or text the builder left where the reviewer read it, is no verdict, never a yes.
pub fn verdict(reviewer: &str, answer: &str) -> Verdict {
    let text = answer_of(answer);
    let last = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or_default();
    if last.eq_ignore_ascii_case("approve") {
        return Verdict { reviewer: reviewer.to_owned(), approve: true, why: String::new() };
    }
    if let Some(rest) = last.get(..8).filter(|h| h.eq_ignore_ascii_case("changes:")).and_then(|_| last.get(8..)) {
        let why = rest.trim();
        let why = if why.is_empty() { "it asks for changes".to_owned() } else { why.to_owned() };
        return Verdict { reviewer: reviewer.to_owned(), approve: false, why };
    }
    Verdict {
        reviewer: reviewer.to_owned(),
        approve: false,
        why: format!("{reviewer} gave no verdict (its last line wasn't APPROVE or CHANGES: …)"),
    }
}

/// The votes counted as a goal counts its judges' (ADR-0110): `all`, `any`, else a majority (a tie isn't approved).
pub fn approved(vote: &str, v: &[Verdict]) -> bool {
    let yes = v.iter().filter(|x| x.approve).count();
    match vote {
        "all" => !v.is_empty() && yes == v.len(),
        "any" => yes > 0,
        _ => yes * 2 > v.len(),
    }
}

/// An answer to one of the run's questions.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Said {
    Yes,
    Stop,
    Words(String),
}

/// The first option (or yes, ok, 1) is yes; the last (or stop, no, 2) is stop; anything else is your words.
fn said(answer: &str, options: &[String]) -> Said {
    let t = answer.trim();
    let l = t.to_ascii_lowercase();
    if options.first().is_some_and(|o| o.eq_ignore_ascii_case(t))
        || matches!(l.as_str(), "yes" | "y" | "ok" | "1" | "approve" | "go" | "go on")
    {
        return Said::Yes;
    }
    if options.last().is_some_and(|o| o.eq_ignore_ascii_case(t)) || matches!(l.as_str(), "stop" | "no" | "n" | "2" | "cancel") {
        return Said::Stop;
    }
    Said::Words(t.to_owned())
}

/// `s` cut to `n` characters, on a character boundary, with an ellipsis when cut.
pub fn short(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// An answer's words without its `wisp` fenced presentation block (what the app draws as cards, not what a station hands on).
pub fn answer_of(text: &str) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim_start();
        if !inside && t.starts_with("```wisp") {
            inside = true;
            continue;
        }
        if inside {
            if t.starts_with("```") {
                inside = false;
            }
            continue;
        }
        out.push(line);
    }
    out.join("\n").trim().to_owned()
}

/// The first lines of a text that say something, on one line.
fn gist(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```")).take(3).collect();
    short(&lines.join(" · "), n)
}

/// The last `n` lines of `s`.
pub fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

const ORDER: [&str; 3] = ["plan", "edits", "auto"];

/// "1 round", "3 rounds" (audit G15: "after 1 round(s)").
pub fn rounds(n: u32) -> String {
    if n == 1 { "1 round".to_owned() } else { format!("{n} rounds") }
}

/// A PR's URL as people say it: `PR #2` for `…/pull/2` (audit G15: "merged: `https://github.com/acme/repo/pull/2`"); any
/// other link as it is.
pub fn pr_words(url: &str) -> String {
    match url.trim_end_matches('/').rsplit_once("/pull/") {
        Some((_, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => format!("PR #{n}"),
        _ => url.to_owned(),
    }
}
/// How much of the spec goes with "Approve the spec?" (the whole of it is the spec station's output).
const SPEC_SHOWN: usize = 1500;

/// The lower of two modes ('' is the other).
fn lower(a: &str, b: &str) -> String {
    let rank = |m: &str| ORDER.iter().position(|x| *x == m);
    match (rank(a), rank(b)) {
        (Some(x), Some(y)) => ORDER[x.min(y)].to_owned(),
        (Some(_), None) => a.to_owned(),
        (None, _) => b.to_owned(),
    }
}

fn goal_line(g: Option<&Goal>) -> String {
    g.filter(|g| !g.text.trim().is_empty()).map(|g| format!("\n\nIt's done when: {}", g.text.trim())).unwrap_or_default()
}

fn extra(p: &str) -> String {
    if p.trim().is_empty() { String::new() } else { format!("\n\n{}", p.trim()) }
}

/// The spec station's prompt.
pub fn spec_prompt(task: &str, goal: Option<&Goal>, more: &str) -> String {
    format!(
        "You are the spec station of a software factory. Write the spec for the task below: what it must do, its \
         constraints, how we'll know it's done (the checks a reviewer will hold it to) and the plan, short. Don't change \
         any file: the build station builds it once the spec is approved.\n\nTask: {task}{}{}",
        goal_line(goal),
        extra(more)
    )
}

/// A builder's prompt.
pub fn build_prompt(task: &str, spec: &str, goal: Option<&Goal>, more: &str) -> String {
    let spec = if spec.trim().is_empty() { String::new() } else { format!("\n\nThe spec:\n{}", spec.trim()) };
    format!(
        "You are the build station of a software factory. Build the task below{}. Run its tests before you finish.\n\nTask: {task}{spec}{}{}",
        if spec.is_empty() { "" } else { " to its spec" },
        goal_line(goal),
        extra(more)
    )
}

/// A reviewer's prompt.
pub fn review_prompt(task: &str, spec: &str, goal: Option<&Goal>, more: &str) -> String {
    let spec = if spec.trim().is_empty() { String::new() } else { format!("\n\nThe spec:\n{}", spec.trim()) };
    format!(
        "You are a reviewer in a software factory. Review the change in this folder (its diff against where it started) \
         against the task{}: correctness first, then tests, then risks. Don't change any file. End with one line, the \
         last of your answer: APPROVE if it's ready to ship, or CHANGES: <what to fix>.\n\nTask: {task}{spec}{}{}",
        if spec.is_empty() { "" } else { " and its spec" },
        goal_line(goal),
        extra(more)
    )
}

/// Where the run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum At {
    Spec,
    ApproveSpec,
    Build,
    Checks,
    Review,
    Ship,
    ApproveMerge,
    Merge,
    /// past the limit, asking whether to keep going
    Limit,
    /// asking before the checks run code a build made with your approvals
    ApproveChecks,
    Over,
}

/// A run's state: the machine.
/// What a run read back after a restart does first ([`Run::resume`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Resume {
    /// wait on these tasks' turns again, then [`Event::Ended`] as before
    Tasks {
        tasks: Vec<String>,
    },
    /// the question is still on the run's task: read its answer, then [`Event::Answered`]
    Ask {
        ask_id: String,
    },
    /// a step that does no harm twice: do it again
    Redo(Next),
    Over,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub input: Input,
    at: At,
    round: u32,
    max: u32,
    spec_task: Option<String>,
    spec_text: String,
    builds: Vec<String>,
    /// how each builder's last turn ended (true: done)
    built: Vec<bool>,
    /// the builds whose checks are still to run this round (indexes into `builds`)
    to_check: Vec<usize>,
    /// the build that passed its checks, which review and ship take
    chosen: Option<usize>,
    reviewers: Vec<String>,
    review_tasks: Vec<String>,
    /// the build the reviewers read (a new chosen build gets new reviewers)
    reviewed: Option<usize>,
    /// the failure that sent it back, kept for a "keep going" answer
    findings: String,
    failed_station: String,
    /// the spec is being revised after a failure (`back_to: spec`): once approved, the builds follow up
    revising: bool,
    /// the last question's options
    options: Vec<String>,
    pr: String,
    pub view: RunView,
}

impl Run {
    pub fn new(input: Input) -> Self {
        let (_, max, _) = input.spec.loop_rule();
        let max = max.or_else(|| input.goal.as_ref().and_then(|g| g.max_rounds)).unwrap_or(MAX_ROUNDS);
        let stations: Vec<StationView> = input
            .spec
            .station_ids()
            .into_iter()
            .map(|id| StationView { id: id.to_owned(), detail: Value::Null, ..StationView::default() })
            .collect();
        let view = RunView { factory: input.factory.clone(), max, stations, ..RunView::default() };
        Self {
            input,
            at: At::Spec,
            round: 0,
            max,
            spec_task: None,
            spec_text: String::new(),
            builds: Vec::new(),
            built: Vec::new(),
            to_check: Vec::new(),
            chosen: None,
            reviewers: Vec::new(),
            review_tasks: Vec::new(),
            reviewed: None,
            findings: String::new(),
            failed_station: String::new(),
            revising: false,
            options: Vec::new(),
            pr: String::new(),
            view,
        }
    }

    fn station(&mut self, id: &str) -> Option<&mut StationView> {
        self.view.stations.iter_mut().find(|s| s.id == id)
    }

    fn set(&mut self, id: &str, state: StationState) {
        let round = self.round;
        if let Some(s) = self.station(id) {
            s.state = state;
            if matches!(state, StationState::Running | StationState::Asking) {
                s.round = round.max(1);
            }
        }
        if matches!(state, StationState::Running | StationState::Asking) {
            id.clone_into(&mut self.view.station);
        }
        self.view.state = if state == StationState::Asking { RunState::Waiting } else { RunState::Running };
        self.view.round = self.round.max(1);
    }

    fn out(&mut self, id: &str, output: String, detail: Value, why: &str) {
        if let Some(s) = self.station(id) {
            s.output = output;
            s.detail = detail;
            why.clone_into(&mut s.why);
        }
    }

    fn agent_or(&self, a: &str) -> String {
        if a.trim().is_empty() { self.input.agent.clone() } else { a.trim().to_owned() }
    }

    fn end(&mut self, state: RunState, why: &str) -> Next {
        self.at = At::Over;
        self.view.state = state;
        why.clone_into(&mut self.view.why);
        self.view.station.clear();
        self.view.waiting = None;
        Next::End { state, why: why.to_owned() }
    }

    fn ask(&mut self, station: &str, question: String, options: &[&str]) -> Next {
        self.ask_about(station, question, options, String::new())
    }

    /// A question with what it's about (`context`): on the run's task, the phone and channels beside the question.
    fn ask_about(&mut self, station: &str, question: String, options: &[&str], context: String) -> Next {
        self.options = options.iter().map(|o| (*o).to_owned()).collect();
        self.set(station, StationState::Asking);
        self.view.waiting = Some(Waiting {
            station: station.to_owned(),
            question: question.clone(),
            options: self.options.clone(),
            ask_id: String::new(),
            context: context.clone(),
        });
        Next::Ask { question, options: self.options.clone(), context }
    }

    /// The host made a [`TaskStep::Start`]'s task.
    pub fn made(&mut self, station: &str, index: usize, task: &str) {
        let list = match station {
            "spec" => {
                self.spec_task = Some(task.to_owned());
                None
            }
            "build" => Some(&mut self.builds),
            _ => Some(&mut self.review_tasks),
        };
        if let Some(l) = list {
            if l.len() <= index {
                l.resize(index + 1, String::new());
            }
            task.clone_into(&mut l[index]);
        }
        if let Some(s) = self.station(station)
            && !s.tasks.iter().any(|t| t == task)
        {
            s.tasks.push(task.to_owned());
        }
    }

    /// The question's id, once the host asked it (what an answer from the band names).
    pub fn asked(&mut self, ask_id: &str) {
        if let Some(w) = self.view.waiting.as_mut() {
            ask_id.clone_into(&mut w.ask_id);
        }
    }

    /// The budget is spent: no new station starts.
    fn spent(&self) -> Option<String> {
        let b = self.input.spec.budget_usd()?;
        (self.view.cost >= b).then(|| format!("the factory's budget (${b:.2}) is spent: ${:.2} so far", self.view.cost))
    }

    fn tasks(&mut self, steps: Vec<TaskStep>) -> Next {
        if let Some(why) = self.spent() {
            return self.end(RunState::Failed, &why);
        }
        Next::Tasks(steps)
    }

    /// Where the run starts.
    pub fn start(&mut self) -> Next {
        self.round = 1;
        self.view.round = 1;
        match self.input.spec.stations.spec.clone() {
            Some(s) => {
                self.at = At::Spec;
                self.set("spec", StationState::Running);
                let step = TaskStep::Start {
                    station: "spec",
                    index: 0,
                    agent: self.agent_or(&s.agent),
                    title: format!("{} · spec", self.input.title),
                    prompt: spec_prompt(&self.input.prompt, self.input.goal.as_ref(), &s.prompt),
                    mode: "plan".into(),
                    worktree: Some(false),
                    folder_of: None,
                };
                self.tasks(vec![step])
            }
            None => self.build(),
        }
    }

    fn build_mode(&self) -> String {
        let own = self.input.spec.stations.build.as_ref().map(|b| b.mode.clone()).unwrap_or_default();
        if self.input.by_agent {
            // an agent's run: never above its own mode, and no mode said is Plan, never Full auto (review r194 #1)
            let top = if self.input.mode.is_empty() { "plan" } else { self.input.mode.as_str() };
            return lower(if own.is_empty() { top } else { &own }, top);
        }
        if own.is_empty() { self.input.mode.clone() } else { own }
    }

    /// The first round's builds.
    fn build(&mut self) -> Next {
        let b = self.input.spec.stations.build.clone().unwrap_or_default();
        let agents: Vec<String> =
            if b.agents.is_empty() { vec![self.agent_or(&b.agent)] } else { b.agents.iter().map(|a| self.agent_or(a)).collect() };
        self.at = At::Build;
        self.set("build", StationState::Running);
        let worktree = b.worktree.or(self.input.worktree);
        let mode = self.build_mode();
        let prompt = build_prompt(&self.input.prompt, &self.spec_text, self.input.goal.as_ref(), &b.prompt);
        let n = agents.len();
        let steps = agents
            .into_iter()
            .enumerate()
            .map(|(i, agent)| TaskStep::Start {
                station: "build",
                index: i,
                title: if n > 1 { format!("{} · build {}", self.input.title, i + 1) } else { format!("{} · build", self.input.title) },
                agent,
                prompt: prompt.clone(),
                mode: mode.clone(),
                worktree,
                folder_of: None,
            })
            .collect();
        self.tasks(steps)
    }

    /// Another round of the builds, told what failed.
    fn rebuild(&mut self, text: &str) -> Next {
        if self.builds.is_empty() {
            return self.build();
        }
        self.at = At::Build;
        self.set("build", StationState::Running);
        self.chosen = None;
        let steps = self.builds.iter().map(|t| TaskStep::FollowUp { task: t.clone(), text: text.to_owned() }).collect();
        self.tasks(steps)
    }

    fn checks(&mut self) -> Next {
        let ok: Vec<usize> = (0..self.builds.len()).filter(|i| self.built.get(*i).copied().unwrap_or(false)).collect();
        let Some(c) = self.input.spec.stations.checks.clone() else {
            self.chosen = ok.first().copied();
            return self.review();
        };
        let commands =
            if c.run.is_empty() { [self.input.test.clone()].into_iter().filter(|t| !t.trim().is_empty()).collect() } else { c.run };
        if commands.is_empty() {
            self.set("checks", StationState::Skipped);
            self.out("checks", "nothing to run".into(), Value::Null, "no commands: give checks.run, or the project a test command");
            self.chosen = ok.first().copied();
            return self.review();
        }
        self.to_check = ok;
        // the build's commands needed your yes (not Full auto) and nothing walls it in: ask before running what it built
        if !self.input.sandbox && self.effective_build_mode() != "auto" {
            return self.ask_checks(&commands);
        }
        self.check_next(&commands, c.timeout.unwrap_or(wisp_factory_api::CHECK_MINUTES))
    }

    /// The mode the build ran in: its own, else the run's ('' : the default, Full auto).
    fn effective_build_mode(&self) -> String {
        let m = self.build_mode();
        if m.is_empty() { "auto".into() } else { m }
    }

    fn ask_checks(&mut self, commands: &[String]) -> Next {
        self.at = At::ApproveChecks;
        let names: Vec<String> = commands.iter().map(|c| format!("`{c}`")).collect();
        let q = format!("Run {} on the built code?", names.join(" and "));
        self.ask("checks", q, &["Run", "Skip", "Stop"])
    }

    /// The checks' commands as they run now (the project's test when the station names none).
    fn check_commands(&self) -> (Vec<String>, u64) {
        let c = self.input.spec.stations.checks.clone().unwrap_or_default();
        let minutes = c.timeout.unwrap_or(wisp_factory_api::CHECK_MINUTES);
        let commands =
            if c.run.is_empty() { [self.input.test.clone()].into_iter().filter(|t| !t.trim().is_empty()).collect() } else { c.run };
        (commands, minutes)
    }

    fn check_next(&mut self, commands: &[String], minutes: u64) -> Next {
        let Some(&i) = self.to_check.first() else {
            return self.review();
        };
        self.at = At::Checks;
        self.set("checks", StationState::Running);
        Next::Checks { task: self.builds[i].clone(), commands: commands.to_vec(), minutes }
    }

    fn reviewer_agents(&self, r: &wisp_factory_api::ReviewStation) -> Vec<String> {
        if !r.agents.is_empty() {
            return r.agents.iter().map(|a| self.agent_or(a)).collect();
        }
        if !r.agent.is_empty() {
            return vec![self.agent_or(&r.agent)];
        }
        // a declared goal's judges review when the station names none (ADR-0110)
        match self.input.goal.as_ref().filter(|g| !g.judges.is_empty()) {
            Some(g) => g.judges.clone(),
            None => vec![self.input.agent.clone()],
        }
    }

    fn vote(&self) -> String {
        let own = self.input.spec.stations.review.as_ref().map(|r| r.vote.clone()).unwrap_or_default();
        if !own.is_empty() {
            return own;
        }
        self.input.goal.as_ref().map(|g| g.vote.clone()).filter(|v| !v.is_empty()).unwrap_or_else(|| "majority".into())
    }

    fn review(&mut self) -> Next {
        let Some(r) = self.input.spec.stations.review.clone() else { return self.ship() };
        let Some(chosen) = self.chosen else { return self.failure("build", "no build to review") };
        self.at = At::Review;
        self.set("review", StationState::Running);
        let folder = self.builds[chosen].clone();
        if self.reviewed == Some(chosen) && !self.review_tasks.is_empty() {
            let text = format!(
                "The builder changed it (round {}). Review it again in this folder and end with one line: APPROVE, or CHANGES: <what to fix>.",
                self.round
            );
            let steps = self.review_tasks.iter().map(|t| TaskStep::FollowUp { task: t.clone(), text: text.clone() }).collect();
            return self.tasks(steps);
        }
        self.reviewed = Some(chosen);
        self.review_tasks.clear();
        self.reviewers = self.reviewer_agents(&r);
        let prompt = review_prompt(&self.input.prompt, &self.spec_text, self.input.goal.as_ref(), &r.prompt);
        let n = self.reviewers.len();
        let steps = self
            .reviewers
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, agent)| TaskStep::Start {
                station: "review",
                index: i,
                title: if n > 1 { format!("{} · review {}", self.input.title, i + 1) } else { format!("{} · review", self.input.title) },
                agent,
                prompt: prompt.clone(),
                mode: "plan".into(),
                worktree: Some(false),
                folder_of: Some(folder.clone()),
            })
            .collect();
        self.tasks(steps)
    }

    fn merge_rule(&self) -> String {
        let m = self.input.spec.stations.ship.as_ref().map(|s| s.merge.clone()).unwrap_or_default();
        match m.as_str() {
            "" => "human".into(),
            // an agent never skips a person's yes (ADR-0112)
            "auto" if self.input.by_agent => "human".into(),
            _ => m,
        }
    }

    fn ship(&mut self) -> Next {
        let Some(s) = self.input.spec.stations.ship.clone() else { return self.end(RunState::Done, "") };
        let Some(chosen) = self.chosen else { return self.end(RunState::Failed, "no build to ship") };
        if s.pr == Some(false) {
            self.set("ship", StationState::Skipped);
            self.out("ship", "no PR (pr: false)".into(), Value::Null, "");
            return self.end(RunState::Done, "");
        }
        self.at = At::Ship;
        self.set("ship", StationState::Running);
        let mut body = String::new();
        if !self.spec_text.trim().is_empty() {
            let _ = write!(body, "## Spec\n\n{}\n\n", short(&self.spec_text, 4000));
        }
        let checks = self.view.stations.iter().find(|x| x.id == "checks").map(|c| c.output.clone()).unwrap_or_default();
        if !checks.is_empty() {
            let _ = write!(body, "Checks: {checks}\n\n");
        }
        let review = self.view.stations.iter().find(|x| x.id == "review").map(|c| c.output.clone()).unwrap_or_default();
        if !review.is_empty() {
            let _ = write!(body, "Review: {review}\n\n");
        }
        let _ = write!(body, "Made by the wisp factory `{}` in {}.", self.input.factory, rounds(self.round));
        Next::Ship { task: self.builds[chosen].clone(), title: self.input.title.clone(), body, draft: s.draft }
    }

    /// A station failed: back to `loop.back_to` while rounds are left, else ask or stop.
    fn failure(&mut self, station: &'static str, findings: &str) -> Next {
        findings.clone_into(&mut self.findings);
        station.clone_into(&mut self.failed_station);
        if self.round < self.max {
            return self.again("");
        }
        let (_, _, then) = self.input.spec.loop_rule();
        let what = match station {
            "checks" => "Checks still fail",
            "review" => "The review still asks for changes",
            _ => "The build still fails",
        };
        if then == "stop" {
            let why = format!("{what} after {}: {}", rounds(self.round), short(findings, 400));
            return self.end(RunState::Failed, &why);
        }
        self.at = At::Limit;
        let q = format!("{what} after {}: {}. Keep going?", rounds(self.round), short(findings, 300));
        self.ask(station, q, &["One more round", "Stop"])
    }

    /// The next round: back to the spec or the build with what failed (and your words).
    fn again(&mut self, words: &str) -> Next {
        self.round += 1;
        self.view.round = self.round;
        let what = match self.failed_station.as_str() {
            "checks" => "the checks failed",
            "review" => "the review asked for changes",
            _ => "the build failed",
        };
        let mut text = format!("Round {}: {what}.\n{}", self.round, self.findings.trim());
        if !words.trim().is_empty() {
            let _ = write!(text, "\n\nAlso: {}", words.trim());
        }
        let (back, _, _) = self.input.spec.loop_rule();
        if back == "spec"
            && let Some(task) = self.spec_task.clone()
        {
            self.revising = true;
            self.at = At::Spec;
            self.set("spec", StationState::Running);
            let text = format!("{text}\n\nRevise the spec so the next build gets it right. Give the whole spec again.");
            return self.tasks(vec![TaskStep::FollowUp { task, text }]);
        }
        self.rebuild(&format!("{text}\n\nFix it, then run the tests again."))
    }

    /// The spec is approved (or needs no approval): to the build, or after a revision, the builds follow it up.
    fn after_spec(&mut self) -> Next {
        self.set("spec", StationState::Done);
        if self.revising && !self.builds.is_empty() {
            self.revising = false;
            let text = format!("Round {}: the spec changed. Build to it:\n{}", self.round, self.spec_text.trim());
            return self.rebuild(&text);
        }
        self.revising = false;
        self.build()
    }

    /// What happened; the next step.
    #[allow(clippy::too_many_lines)] // one match over where the run is: each arm is a station's result
    pub fn on(&mut self, e: Event) -> Next {
        if self.at == At::Over {
            return Next::End { state: self.view.state, why: self.view.why.clone() };
        }
        match e {
            Event::Stop(why) => {
                let st = self.view.station.clone();
                if let Some(s) = self.station(&st)
                    && matches!(s.state, StationState::Running | StationState::Asking)
                {
                    s.state = StationState::Failed;
                    "stopped".clone_into(&mut s.why);
                }
                self.end(RunState::Stopped, &why)
            }
            Event::Failed(why) => {
                let st = self.view.station.clone();
                if let Some(s) = self.station(&st) {
                    s.state = StationState::Failed;
                    why.clone_into(&mut s.why);
                }
                self.end(RunState::Failed, &why)
            }
            Event::Ended(ends) => {
                self.view.cost += ends.iter().map(|x| x.cost).sum::<f64>();
                match self.at {
                    At::Spec => {
                        let Some(end) = ends.into_iter().next() else { return self.end(RunState::Failed, "the spec station made no task") };
                        if !end.ok {
                            let why = format!("the spec station failed: {}", short(&end.text, 300));
                            self.set("spec", StationState::Failed);
                            self.out("spec", String::new(), Value::Null, &why);
                            return self.end(RunState::Failed, &why);
                        }
                        let text = answer_of(&end.text);
                        self.spec_text = short(&text, 20_000);
                        let (g, t) = (gist(&text, 200), self.spec_text.clone());
                        self.out("spec", g, json!(t), "");
                        let approve = self.input.spec.stations.spec.as_ref().map(|s| s.approve.clone()).unwrap_or_default();
                        if approve == "none" {
                            return self.after_spec();
                        }
                        self.at = At::ApproveSpec;
                        // the spec itself goes with the question (audit G5: `wisp show`, the phone and channels asked
                        // with no spec, and the app had it only behind ⋯ › Open Spec)
                        let about = short(&self.spec_text, SPEC_SHOWN);
                        self.ask_about(
                            "spec",
                            "Approve the spec? Approve to build it, Stop, or say what to change.".into(),
                            &["Approve", "Stop"],
                            about,
                        )
                    }
                    At::Build => {
                        self.built = ends.iter().map(|x| x.ok).collect();
                        let ok = self.built.iter().filter(|x| **x).count();
                        let detail: Vec<Value> =
                            ends.iter().map(|x| json!({ "task": x.task, "ok": x.ok, "text": short(&answer_of(&x.text), 2000) })).collect();
                        let said = ends.iter().find(|x| x.ok).map(|x| gist(&answer_of(&x.text), 200)).unwrap_or_default();
                        if ok == 0 {
                            let why = ends.iter().map(|x| short(&x.text, 300)).collect::<Vec<_>>().join("\n");
                            self.set("build", StationState::Failed);
                            self.out("build", String::new(), json!(detail), &format!("it failed: {}", short(&why, 300)));
                            return self.failure("build", &format!("The build failed: {why}"));
                        }
                        self.set("build", StationState::Done);
                        self.out("build", said, json!(detail), "");
                        self.checks()
                    }
                    At::Review => {
                        let verdicts: Vec<Verdict> = ends
                            .iter()
                            .zip(self.reviewers.iter().chain(std::iter::repeat(&String::new())))
                            .map(|(x, who)| {
                                let name = if who.is_empty() { "the reviewer".to_owned() } else { who.clone() };
                                if x.ok {
                                    verdict(&name, &x.text)
                                } else {
                                    Verdict { reviewer: name, approve: false, why: format!("the review failed: {}", short(&x.text, 200)) }
                                }
                            })
                            .collect();
                        let yes = verdicts.iter().filter(|v| v.approve).count();
                        let output = format!("{yes} of {} approve", verdicts.len());
                        let detail: Vec<Value> =
                            verdicts.iter().map(|v| json!({ "reviewer": v.reviewer, "approve": v.approve, "why": v.why })).collect();
                        if approved(&self.vote(), &verdicts) {
                            self.set("review", StationState::Done);
                            self.out("review", output, json!(detail), "");
                            return self.ship();
                        }
                        let findings: Vec<String> =
                            verdicts.iter().filter(|v| !v.approve).map(|v| format!("{}: {}", v.reviewer, v.why)).collect();
                        let findings = findings.join("\n");
                        self.set("review", StationState::Failed);
                        self.out("review", output, json!(detail), &short(&findings, 300));
                        self.failure("review", &format!("What the review asked for:\n{findings}"))
                    }
                    _ => Next::Tasks(Vec::new()),
                }
            }
            Event::Checked(runs) => {
                let Some(i) = (!self.to_check.is_empty()).then(|| self.to_check.remove(0)) else { return self.review() };
                let passed = runs.iter().filter(|r| r.code == 0 && !r.timed_out).count();
                let failed: Vec<&CheckRun> = runs.iter().filter(|r| r.code != 0 || r.timed_out).collect();
                let detail: Vec<Value> = runs
                    .iter()
                    .map(|r| json!({ "command": r.command, "code": r.code, "timedOut": r.timed_out, "tail": r.tail, "build": i }))
                    .collect();
                if failed.is_empty() {
                    self.chosen = Some(i);
                    self.set("checks", StationState::Done);
                    self.out("checks", format!("{passed} of {} passed", runs.len()), json!(detail), "");
                    return self.review();
                }
                let findings = failed
                    .iter()
                    .map(|r| {
                        let how = if r.timed_out { "ran past its time".to_owned() } else { format!("exited {}", r.code) };
                        // a check that printed nothing: no dangling colon (audit G15 "`exit 1` exited 1:.")
                        if r.tail.trim().is_empty() {
                            format!("`{}` {how} (it printed nothing)", r.command)
                        } else {
                            format!("`{}` {how}:\n{}", r.command, r.tail.trim())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                self.out("checks", format!("{passed} of {} passed", runs.len()), json!(detail), &short(&findings, 300));
                if !self.to_check.is_empty() {
                    // another builder's turn to be checked
                    let c = self.input.spec.stations.checks.clone().unwrap_or_default();
                    let commands = if c.run.is_empty() { vec![self.input.test.clone()] } else { c.run };
                    return self.check_next(&commands, c.timeout.unwrap_or(wisp_factory_api::CHECK_MINUTES));
                }
                self.set("checks", StationState::Failed);
                self.failure("checks", &format!("The checks failed:\n{findings}"))
            }
            Event::Answered(text) => {
                let answer = said(&text, &self.options);
                self.view.waiting = None;
                match self.at {
                    At::ApproveSpec => match answer {
                        Said::Yes => self.after_spec(),
                        Said::Stop => {
                            self.set("spec", StationState::Failed);
                            self.end(RunState::Stopped, "you stopped it at the spec")
                        }
                        Said::Words(w) => {
                            let Some(task) = self.spec_task.clone() else {
                                return self.end(RunState::Failed, "the spec station has no task");
                            };
                            self.at = At::Spec;
                            self.set("spec", StationState::Running);
                            let text = format!("Change the spec: {w}\n\nGive the whole spec again.");
                            self.tasks(vec![TaskStep::FollowUp { task, text }])
                        }
                    },
                    At::Limit => match answer {
                        Said::Yes => {
                            self.max = self.round + 1;
                            self.view.max = self.max;
                            self.again("")
                        }
                        Said::Stop => {
                            let st = self.failed_station.clone();
                            self.set(&st, StationState::Failed);
                            let why = format!("you stopped it after {}", rounds(self.round));
                            self.end(RunState::Stopped, &why)
                        }
                        Said::Words(w) => {
                            self.max = self.round + 1;
                            self.view.max = self.max;
                            self.again(&w)
                        }
                    },
                    At::ApproveChecks => {
                        let (commands, minutes) = self.check_commands();
                        match answer {
                            Said::Yes => self.check_next(&commands, minutes),
                            Said::Stop => {
                                self.set("checks", StationState::Failed);
                                self.end(RunState::Stopped, "you stopped it before the checks")
                            }
                            Said::Words(w) if w.eq_ignore_ascii_case("skip") => {
                                self.set("checks", StationState::Skipped);
                                self.out("checks", "skipped".into(), Value::Null, "you skipped them");
                                self.chosen = self.to_check.first().copied();
                                self.to_check.clear();
                                self.review()
                            }
                            Said::Words(_) => self.ask_checks(&commands),
                        }
                    }
                    At::ApproveMerge => {
                        if answer == Said::Yes {
                            self.at = At::Merge;
                            self.set("ship", StationState::Running);
                            let Some(c) = self.chosen else { return self.end(RunState::Failed, "no build to merge") };
                            Next::Merge { task: self.builds[c].clone() }
                        } else {
                            self.set("ship", StationState::Done);
                            self.end(RunState::Done, "")
                        }
                    }
                    _ => Next::Tasks(Vec::new()),
                }
            }
            Event::Shipped { ok, pr, why } => {
                if !ok {
                    self.set("ship", StationState::Failed);
                    let why = format!("ship failed: {}", short(&why, 300));
                    self.out("ship", String::new(), Value::Null, &why);
                    return self.end(RunState::Failed, &why);
                }
                pr.clone_into(&mut self.pr);
                self.view.pr = Some(pr.clone());
                self.out("ship", format!("{} open", pr_words(&pr)), json!({ "pr": pr }), "");
                match self.merge_rule().as_str() {
                    "auto" => {
                        self.at = At::Merge;
                        let Some(c) = self.chosen else { return self.end(RunState::Failed, "no build to merge") };
                        Next::Merge { task: self.builds[c].clone() }
                    }
                    "none" => {
                        self.set("ship", StationState::Done);
                        self.end(RunState::Done, "")
                    }
                    _ => {
                        self.at = At::ApproveMerge;
                        let q = format!("The PR is open ({pr}). Merge it?");
                        self.ask("ship", q, &["Merge", "Leave it open"])
                    }
                }
            }
            Event::Merged { ok, why, local } => {
                if ok {
                    let pr = self.pr.clone();
                    self.set("ship", StationState::Done);
                    let said =
                        if local.is_empty() { format!("merged {}", pr_words(&pr)) } else { format!("merged {} · {local}", pr_words(&pr)) };
                    self.out("ship", said, json!({ "pr": pr, "merged": true, "local": local }), "");
                    self.end(RunState::Done, "")
                } else {
                    self.set("ship", StationState::Failed);
                    let why = format!("merge failed: {}", short(&why, 300));
                    let pr = self.pr.clone();
                    self.out("ship", format!("{} open", pr_words(&pr)), json!({ "pr": pr }), &why);
                    self.end(RunState::Failed, &why)
                }
            }
        }
    }

    /// Where a run read back after a restart picks up (the owner's delegate, 2026-11-20): the tasks its station waits
    /// on, the question it asks you, or the step to do again.
    pub fn resume(&mut self) -> Resume {
        match self.at {
            At::Over => Resume::Over,
            At::ApproveSpec | At::Limit | At::ApproveMerge | At::ApproveChecks => {
                Resume::Ask { ask_id: self.view.waiting.as_ref().map(|w| w.ask_id.clone()).unwrap_or_default() }
            }
            At::Spec => Resume::Tasks { tasks: self.spec_task.iter().cloned().collect() },
            At::Build => Resume::Tasks { tasks: self.builds.clone() },
            At::Review => Resume::Tasks { tasks: self.review_tasks.clone() },
            At::Checks => {
                let (commands, minutes) = self.check_commands();
                match self.to_check.first().and_then(|i| self.builds.get(*i)) {
                    Some(task) => Resume::Redo(Next::Checks { task: task.clone(), commands, minutes }),
                    None => Resume::Redo(self.review()),
                }
            }
            At::Ship => Resume::Redo(self.ship()),
            At::Merge => match self.chosen.and_then(|c| self.builds.get(c)) {
                Some(task) => Resume::Redo(Next::Merge { task: task.clone() }),
                None => Resume::Redo(self.end(RunState::Failed, "no build to merge")),
            },
        }
    }

    /// The run's total with `more` (what its running station's tasks have cost so far) past its budget: why, to stop
    /// the station there (review r194 factories #6).
    pub fn over_budget(&self, more: f64) -> Option<String> {
        let b = self.input.spec.budget_usd()?;
        let total = self.view.cost + more;
        (total >= b).then(|| format!("the factory's budget (${b:.2}) is spent: ${total:.2} so far"))
    }

    /// The run's view, for its parent task.
    pub fn view(&self) -> &RunView {
        &self.view
    }
}

/// The list row's line (`runtime.summary`'s `factory`): the factory, where it is, the round of the limit, each station's
/// state, round, tasks and short output, and what it waits on you for. A station's whole detail stays on the task.
pub fn summary_of(v: &RunView) -> Value {
    let stations: Vec<Value> = v
        .stations
        .iter()
        .map(|s| json!({ "id": s.id, "state": s.state, "round": s.round, "tasks": s.tasks, "output": short(&s.output, 160), "why": short(&s.why, 160) }))
        .collect();
    let mut out = json!({ "name": v.factory, "state": v.state, "station": v.station, "round": v.round, "max": v.max, "stations": stations, "why": short(&v.why, 300) });
    if let Some(w) = &v.waiting {
        out["waiting"] = json!({ "station": w.station, "question": short(&w.question, 300), "options": w.options, "askId": w.ask_id });
    }
    if let Some(pr) = &v.pr {
        out["pr"] = json!(pr);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests;
