//! The Steward (ADR-0132): after the follow loop has read a task's PR, what it does next. It sends the task one
//! follow-up holding the failing checks (their log tails), the review threads it hasn't sent yet and, with `review`,
//! another vendor's high findings. It waits while checks run or the review reads, stops green, or stops with a
//! reason: a limit reached, a failure that came back after a fix, a check that fails on the base too, a PR no
//! longer open. It never re-runs a check and never merges. Pure: the host reads gh and wisp.review and passes in
//! what they said.

use wisp_ship_api::{PrSummary, ShipTask, Steward};

use crate::follow::Busy;

/// Passes per cause per task (the conflict fix's limit).
pub const MAX_PER_CAUSE: u32 = 3;
/// Passes per task, every cause (conflict fixes included).
pub const MAX_PASSES: u32 = 6;
/// Lines kept from the end of each failed job's log...
pub const LOG_LINES: usize = 200;
/// ...and bytes of logs in one follow-up, all jobs together.
pub const LOG_BYTES: usize = 16 * 1024;
/// Thread keys and fingerprints kept per task.
const KEEP: usize = 200;

pub const FIX: &str = "fix";
pub const REVIEW: &str = "review";
pub const OFF: &str = "off";

/// `ship.steward`'s mode as stored: `fix`, `review`, else None (off, or a word it doesn't know).
pub fn mode_of(s: &str) -> Option<&'static str> {
    match s.trim() {
        FIX | "on" | "true" => Some(FIX),
        REVIEW | "fix+review" => Some(REVIEW),
        _ => None,
    }
}

/// A failing check as the host read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Failing {
    pub name: String,
    pub url: String,
    /// the end of its failed job's log ("" when gh can't read it: a status, not an Actions job)
    pub log: String,
    /// the same check fails on the base's latest commit too: not this branch's to fix
    pub on_base: bool,
}

/// An unresolved, not outdated review thread from a person.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenThread {
    pub id: String,
    pub path: String,
    pub line: Option<u32>,
    /// (author, body), in order
    pub comments: Vec<(String, String)>,
    /// the time of its last comment: a new answer makes it news again
    pub last_at: String,
}

impl OpenThread {
    pub fn key(&self) -> String {
        format!("{}@{}", self.id, self.last_at)
    }
}

/// A review finding the task gets back (high only).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighFinding {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub text: String,
}

/// What wisp.review says about the PR's head, with `review`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewRead {
    /// none yet for this head, or one running
    Pending,
    /// read this head: who, and its high findings
    Done { by: String, high: Vec<HighFinding> },
    /// it couldn't review (none of another vendor set up, it failed): why; the Steward goes on without
    Skipped(String),
}

/// What the host read on this look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub failing: Vec<Failing>,
    pub threads: Vec<OpenThread>,
    /// None without `review`
    pub review: Option<ReviewRead>,
}

/// Before reading anything: whether this look is worth gh's time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    Skip,
    /// the PR is gone: stop, with why
    Stop(String),
    Look,
}

pub fn gate(task: &ShipTask, busy: Busy) -> Gate {
    let Some(st) = task.steward.as_ref() else { return Gate::Skip };
    if mode_of(&st.mode).is_none() || matches!(st.step.as_str(), "fixing" | "pushing" | "stopped") {
        return Gate::Skip;
    }
    let Some(pr) = task.pr() else { return Gate::Skip };
    if pr.is_merged() {
        return Gate::Stop("The PR merged.".into());
    }
    if !pr.is_open() {
        return Gate::Stop("The PR was closed.".into());
    }
    if pr.draft {
        return Gate::Stop("The PR is a draft again.".into());
    }
    // the same head already green: nothing new until a push
    if st.step == "green" && st.head == pr.head {
        return Gate::Skip;
    }
    let fixing = task.conflict_fix.as_ref().is_some_and(|f| matches!(f.step.as_str(), "fixing" | "pushing"));
    if busy.live || busy.ask || busy.shipping || busy.archived || busy.starting || fixing || pr.conflict {
        return Gate::Skip;
    }
    // checks still running or not listed yet after a push, or GitHub still working out the merge: one pass carries
    // everything once they settle
    if pr.checks.pending > 0 || pr.waiting_checks || pr.merge_state == "UNKNOWN" {
        return Gate::Skip;
    }
    Gate::Look
}

/// What to do after a look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// nothing to do yet (the review is reading); `why` says what it waits on
    Wait(Option<String>),
    Green {
        reviewer: Option<String>,
    },
    Stop(String),
    Pass(Pass),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    /// checks | threads | review, in that order
    pub causes: Vec<String>,
    pub prompt: String,
    /// the failures it sends, fingerprinted
    pub prints: Vec<String>,
    /// the threads it sends
    pub keys: Vec<String>,
}

/// A failure's fingerprint: its check's name and its error lines, with numbers, hex ids and timestamps taken
/// out, hashed (FNV-1a). The same failure after a fix hashes the same.
pub fn fingerprint(f: &Failing) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |s: &str| {
        for b in s.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&f.name);
    let errors: Vec<String> = f
        .log
        .lines()
        .map(strip_stamp)
        .filter(|l| {
            let low = l.to_ascii_lowercase();
            ["error", "fail", "panic", "assert", "expected"].iter().any(|w| low.contains(w))
        })
        .map(normalize)
        .collect();
    for l in &errors {
        eat("\n");
        eat(l);
    }
    format!("{h:016x}")
}

fn total(st: &Steward, task: &ShipTask) -> u32 {
    st.passes.checks + st.passes.threads + st.passes.review + task.conflict_fix.as_ref().map_or(0, |f| f.tries)
}

/// After a look: a pass, a wait, green, or a stop.
pub fn plan(task: &ShipTask, seen: &Seen) -> Plan {
    let Some(st) = task.steward.as_ref() else { return Plan::Wait(None) };
    let Some(pr) = task.pr() else { return Plan::Wait(None) };
    let fresh: Vec<&Failing> = seen.failing.iter().filter(|f| !f.on_base).collect();
    let prints: Vec<String> = fresh.iter().map(|f| fingerprint(f)).collect();
    if let Some((f, _)) = fresh.iter().zip(&prints).find(|(_, p)| st.prints.contains(p)) {
        return Plan::Stop(format!("{} failed the same way after a fix: it needs you.", f.name));
    }
    let threads: Vec<&OpenThread> = seen.threads.iter().filter(|t| !st.seen.contains(&t.key())).collect();
    let high: &[HighFinding] = match &seen.review {
        Some(ReviewRead::Done { high, .. }) => high,
        _ => &[],
    };
    let mut causes = Vec::new();
    let mut capped = Vec::new();
    for (cause, has, used) in [
        ("checks", !fresh.is_empty() || (pr.checks.fail > 0 && seen.failing.is_empty()), st.passes.checks),
        ("threads", !threads.is_empty(), st.passes.threads),
        ("review", !high.is_empty(), st.passes.review),
    ] {
        if !has {
            continue;
        }
        if used >= MAX_PER_CAUSE { capped.push(cause) } else { causes.push(cause.to_owned()) }
    }
    if !causes.is_empty() && total(st, task) >= MAX_PASSES {
        return Plan::Stop(format!("{MAX_PASSES} passes and the PR still isn't green: it needs you."));
    }
    if causes.is_empty() {
        if let Some(c) = capped.first() {
            let what = match *c {
                "checks" => "the failing checks",
                "threads" => "the review threads",
                _ => "the review's findings",
            };
            return Plan::Stop(format!("Tried {what} {MAX_PER_CAUSE} times: it needs you."));
        }
        if let Some(f) = seen.failing.iter().find(|f| f.on_base) {
            return Plan::Stop(format!("{} fails on `{}` too: not this branch's to fix.", f.name, pr.base));
        }
        return match &seen.review {
            Some(ReviewRead::Pending) => Plan::Wait(Some("Waiting for the review.".into())),
            Some(ReviewRead::Done { by, .. }) => Plan::Green { reviewer: Some(by.clone()) },
            Some(ReviewRead::Skipped(_)) | None => Plan::Green { reviewer: None },
        };
    }
    let prompt = prompt(pr, &causes, &fresh, &threads, high);
    let keys = if causes.iter().any(|c| c == "threads") { threads.iter().map(|t| t.key()).collect() } else { Vec::new() };
    let prints = if causes.iter().any(|c| c == "checks") { prints } else { Vec::new() };
    Plan::Pass(Pass { causes, prompt, prints, keys })
}

/// The follow-up: what's wrong, in order (checks, threads, findings), and how to hand back.
pub fn prompt(pr: &PrSummary, causes: &[String], failing: &[&Failing], threads: &[&OpenThread], high: &[HighFinding]) -> String {
    let has = |c: &str| causes.iter().any(|x| x == c);
    let mut out =
        vec![format!("wisp's Steward is keeping PR #{} ({}) going until it's green. Fix what's below:", pr.number, pr.url), String::new()];
    if has("checks") {
        out.push("## Failing checks".into());
        let mut left = LOG_BYTES;
        for f in failing {
            out.push(format!("### {}{}", f.name, if f.url.is_empty() { String::new() } else { format!(" ({})", f.url) }));
            let log = tail_bytes(&f.log, left);
            left = left.saturating_sub(log.len());
            if log.trim().is_empty() {
                out.push("(no log wisp can read: open the link, or reproduce it locally)".into());
            } else {
                out.push(format!("```\n{log}\n```"));
            }
        }
        if failing.is_empty() {
            out.push("A check failed that wisp can't read the log of: run the repo's checks locally to find it.".into());
        }
        out.push(String::new());
    }
    if has("threads") {
        out.push("## Review comments".into());
        for t in threads {
            let at = t.line.map_or_else(|| t.path.clone(), |l| format!("{}:{l}", t.path));
            out.push(format!("### {at}"));
            for (who, body) in &t.comments {
                let quoted: Vec<String> = body.lines().map(|l| format!("> {l}")).collect();
                out.push(format!("{who}:\n{}", quoted.join("\n")));
            }
        }
        out.push(String::new());
    }
    if has("review") {
        out.push("## High findings from a review by another model".into());
        for f in high {
            let at = match (&f.file, f.line) {
                (Some(p), Some(l)) => format!("{p}:{l}: "),
                (Some(p), None) => format!("{p}: "),
                _ => String::new(),
            };
            out.push(format!("- {at}{}", f.text));
        }
        out.push(String::new());
    }
    out.push("Make the smallest change that fixes each one, and run the checks that are cheap here.".into());
    out.push("Don't push: wisp pushes when you're done. Don't reply to or resolve the threads on GitHub.".into());
    out.push("If a comment is wrong, or a fix needs a decision only the user can make, say so and ask them.".into());
    out.join("\n")
}

/// The record when the Steward is switched on. Back on after a stop is the user asking for another go: the counts
/// and fingerprints start again; the threads already sent stay sent. A mode change while it runs keeps everything.
pub fn started(mode: &str, now: i64, was: Option<&Steward>) -> Steward {
    let again = was.is_none_or(|w| w.step == "stopped" || mode_of(&w.mode).is_none());
    let mut s = was.cloned().unwrap_or_default();
    mode.clone_into(&mut s.mode);
    if again {
        s.step = "watching".into();
        s.passes = wisp_ship_api::StewardPasses::default();
        s.prints.clear();
        s.started_at = now;
    }
    s.why = None;
    s
}

/// A pass goes out: counted, its failures and threads remembered.
pub fn sent(st: &mut Steward, head: &str, p: &Pass) {
    st.step = "fixing".into();
    head.clone_into(&mut st.head);
    st.causes.clone_from(&p.causes);
    st.why = None;
    for c in &p.causes {
        match c.as_str() {
            "checks" => st.passes.checks += 1,
            "threads" => st.passes.threads += 1,
            _ => st.passes.review += 1,
        }
    }
    keep(&mut st.prints, &p.prints);
    keep(&mut st.seen, &p.keys);
}

fn keep(list: &mut Vec<String>, more: &[String]) {
    for m in more {
        if !list.contains(m) {
            list.push(m.clone());
        }
    }
    let over = list.len().saturating_sub(KEEP);
    list.drain(..over);
}

pub fn stop(st: &mut Steward, why: impl Into<String>) {
    st.step = "stopped".into();
    st.why = Some(why.into());
}

/// After a restart: a pass being pushed failed; one being fixed waits for its turn's end as before.
pub fn after_restart(task: &mut ShipTask) {
    if let Some(st) = task.steward.as_mut().filter(|s| s.step == "pushing") {
        stop(st, "wisp restarted while pushing the fix.");
    }
}

mod log;
mod read;

pub use log::log_tail;
use log::{normalize, strip_stamp, tail_bytes};
pub use read::{THREADS_QUERY, base_failed, encode, reviewer_name, threads_from};

#[cfg(test)]
mod tests;
