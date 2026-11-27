//! wisp-risk-core: merge risk's pure logic (ADR-0140). Which of a task's open PRs are due a judgment (their head isn't
//! judged yet), who judges, the prompt, reading the judge's answer, the slot's transitions (start, finish) and what the
//! page sees. Sans-IO: the clock comes in as `now`.

use std::fmt::Write as _;

use serde_json::Value;
use wisp_risk_api::{HARNESSES, Level, Pick, RiskView, Slot, Status, Verdict};

/// How long a judge may take.
pub const JUDGE_MS: u64 = 300_000;
/// A verdict still `running` this long after it started was cut short (a restart, a lost run): it reads as failed and
/// is due again.
pub const STALE_MS: i64 = 420_000;
/// The diff the judge gets, at most (characters).
pub const MAX_DIFF: usize = 60_000;
const MAX_REASONS: usize = 8;

/// An open PR of the task, as wisp.ship's slot has it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenPr {
    pub url: String,
    pub repo: String,
    pub number: u64,
    pub head: String,
    pub title: String,
}

/// Every open PR with a known head in wisp.ship's slot: its repos' (Ship it opened them), then the attached ones
/// (`wisp_pr`), each URL once.
pub fn open_prs(ship: &Value) -> Vec<OpenPr> {
    let repos = ship["repos"].as_array().into_iter().flatten().map(|r| (r["name"].as_str().unwrap_or_default(), &r["pr"]));
    let attached = ship["prs"].as_array().into_iter().flatten().map(|x| (x["repo"].as_str().unwrap_or_default(), &x["pr"]));
    let mut out: Vec<OpenPr> = Vec::new();
    for (repo, p) in repos.chain(attached) {
        let (Some(url), Some(head)) = (p["url"].as_str(), p["head"].as_str()) else { continue };
        if p["state"] != "OPEN" || url.is_empty() || head.is_empty() || out.iter().any(|o| o.url == url) {
            continue;
        }
        out.push(OpenPr {
            url: url.to_owned(),
            repo: repo.to_owned(),
            number: p["number"].as_u64().unwrap_or_default(),
            head: head.to_owned(),
            title: p["title"].as_str().unwrap_or_default().to_owned(),
        });
    }
    out
}

/// The slot as kept, or empty when it's unset or unreadable.
pub fn slot_of(v: &Value) -> Slot {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// A judge that started more than [`STALE_MS`] ago and never ended.
pub fn stale(v: &Verdict, now: i64) -> bool {
    v.status == Status::Running && now.saturating_sub(v.at) > STALE_MS
}

/// The open PRs to judge now: those with no verdict of their head, or one cut short. `force` (Judge again) takes
/// heads judged already too, but never one being judged. `only`: just that PR.
pub fn due(open: &[OpenPr], slot: &Slot, now: i64, force: bool, only: Option<&str>) -> Vec<OpenPr> {
    open.iter()
        .filter(|p| only.is_none_or(|u| u == p.url))
        .filter(|p| match slot.prs.iter().find(|v| v.url == p.url) {
            None => true,
            Some(v) if v.head != p.head => true,
            Some(v) if v.status == Status::Running => stale(v, now),
            Some(_) => force,
        })
        .cloned()
        .collect()
}

/// The slot with `due` marked running by `by`, and the verdicts of PRs that are no longer open dropped.
pub fn start(slot: &Slot, open: &[OpenPr], due: &[OpenPr], by: &Pick, now: i64) -> Slot {
    let mut prs: Vec<Verdict> = slot.prs.iter().filter(|v| open.iter().any(|p| p.url == v.url)).cloned().collect();
    for p in due {
        let v = Verdict {
            url: p.url.clone(),
            repo: p.repo.clone(),
            number: p.number,
            head: p.head.clone(),
            status: Status::Running,
            by: Some(by.clone()),
            at: now,
            ..Verdict::default()
        };
        match prs.iter_mut().find(|x| x.url == p.url) {
            Some(x) => *x = v,
            None => prs.push(v),
        }
    }
    Slot { prs }
}

/// What the judge's answer says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub level: Level,
    pub score: Option<u8>,
    pub summary: String,
    pub reasons: Vec<String>,
}

/// How a judgment ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Answered(Answer),
    Failed(String),
}

/// The slot with the run of `url`@`head` ended, or None when that run is no longer the one the slot holds (a newer
/// head started, the PR closed): its answer is dropped.
pub fn finish(slot: &Slot, url: &str, head: &str, outcome: Outcome, partial: bool, now: i64) -> Option<Slot> {
    let mut next = slot.clone();
    let v = next.prs.iter_mut().find(|v| v.url == url && v.head == head && v.status == Status::Running)?;
    v.at = now;
    v.partial = partial;
    match outcome {
        Outcome::Answered(a) => {
            v.status = Status::Done;
            v.level = Some(a.level);
            v.score = a.score;
            v.summary = a.summary;
            v.reasons = a.reasons;
            v.why = None;
        }
        Outcome::Failed(why) => {
            v.status = Status::Failed;
            v.why = Some(why);
        }
    }
    Some(next)
}

/// Every verdict as the page sees it, the task's PRs' order: whether it's of the PR's head now, and a run cut short
/// read as failed.
pub fn view(slot: &Slot, open: &[OpenPr], now: i64) -> Vec<RiskView> {
    let rank = |v: &Verdict| open.iter().position(|p| p.url == v.url).unwrap_or(usize::MAX);
    let mut out: Vec<RiskView> = slot
        .prs
        .iter()
        .map(|v| {
            let mut verdict = v.clone();
            if stale(v, now) {
                verdict.status = Status::Failed;
                verdict.why = Some("The judge didn't finish.".into());
            }
            let current = open.iter().any(|p| p.url == v.url && p.head == v.head);
            RiskView { verdict, current }
        })
        .collect();
    out.sort_by_key(|r| rank(&r.verdict));
    out
}

/// A harness as the runtime reports it (runtime.agents).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub available: bool,
}

/// Every harness that wrote part of the task: its turns', else the task's own; never Auto or wisp.
pub fn authors_of<'a>(task_agent: &'a str, turn_agents: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in turn_agents.into_iter().chain(std::iter::once(task_agent)) {
        if !a.is_empty() && a != "auto" && a != "wisp" && !out.iter().any(|x| x == a) {
            out.push(a.to_owned());
        }
    }
    out
}

/// The setting is on: anything but unset or `off`.
pub fn is_on(setting: &str) -> bool {
    !setting.is_empty() && setting != "off"
}

/// The judge for `setting`, or why none can judge. Never picks an account: the harness runs on the one it's signed
/// in with.
pub fn pick_judge(setting: &str, authors: &[String], agents: &[Agent]) -> Result<Pick, String> {
    let ready = |id: &str| agents.iter().any(|a| a.id == id && a.available);
    let pick = |id: &str| Pick { agent: id.to_owned(), model: String::new() };
    if !is_on(setting) {
        return Err("Merge risk is off.".into());
    }
    if setting == "other" {
        return HARNESSES
            .iter()
            .find(|h| ready(h) && !authors.iter().any(|a| a == *h))
            .map(|h| pick(h))
            .ok_or_else(|| "No other harness than the one that wrote it is installed here.".into());
    }
    if ready(setting) {
        return Ok(pick(setting));
    }
    let name = agents.iter().find(|a| a.id == setting).map_or(setting, |a| if a.name.is_empty() { setting } else { a.name.as_str() });
    Err(format!("{name} isn't installed here."))
}

/// The diff cut to [`MAX_DIFF`] at a line's end, and whether it was cut.
pub fn fit_diff(diff: &str) -> (String, bool) {
    if diff.len() <= MAX_DIFF {
        return (diff.to_owned(), false);
    }
    let mut end = MAX_DIFF;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let cut = diff[..end].rfind('\n').map_or(end, |i| i + 1);
    (diff[..cut].to_owned(), true)
}

/// The first `n` characters, the last one an ellipsis when it was longer (trimmed first).
pub fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// What the judge is told merge risk is, and how to weigh it.
const INTRO: &str = "You judge the merge risk of a pull request another agent wrote. Do not change any file, do not run anything that \
         writes, do not comment on GitHub: read, then answer.\n\n\
         Merge risk is how likely merging this PR as it is breaks something or is hard to undo, and how bad that would be. \
         Weigh: what it touches (auth, payments, data and migrations, concurrency, public APIs and wire formats, build and CI, \
         config and deploys, deletions), how much of it there is, whether its tests cover the change, whether it is easy to \
         revert, and anything in the diff that looks wrong. A docs, test-only or small isolated change is low. A wide or \
         subtle change to shared code is medium. A migration, a security-sensitive path, a breaking contract or data that \
         can be lost is high.\n\n";

/// The one JSON object the judge answers with.
const ANSWER: &str = "Answer with one JSON object and nothing else:\n\
         {\"risk\": \"low\" | \"medium\" | \"high\", \"score\": 0-100, \"summary\": \"one line: why this level\", \
         \"reasons\": [\"what makes it risky or safe, most important first, each one short line naming the file\"]}\n\
         Give at most 5 reasons.";

/// What the judge is asked: the task, the PR, its diff, and the one JSON object to answer with.
pub fn prompt(task_title: &str, task: &str, pr: &OpenPr, diff: &str, partial: bool) -> String {
    let mut p = String::from(INTRO);
    let _ = writeln!(p, "Task: {}", clip(task_title, 200));
    if !task.trim().is_empty() {
        let _ = writeln!(p, "What the user asked: {}", clip(task, 1500));
    }
    let repo = if pr.repo.is_empty() { String::new() } else { format!("{} ", pr.repo) };
    let _ = write!(p, "PR: {repo}#{} {}\n{}\n\n", pr.number, clip(&pr.title, 200), pr.url);
    if partial {
        p.push_str("The diff is long: below is its start only. Say so in a reason when what you can't see could matter.\n\n");
    }
    p.push_str("```diff\n");
    p.push_str(diff);
    if !diff.ends_with('\n') {
        p.push('\n');
    }
    p.push_str("```\n\n");
    p.push_str(ANSWER);
    p
}

fn level_of(s: &str) -> Option<Level> {
    match s.trim().to_ascii_lowercase().as_str() {
        "low" => Some(Level::Low),
        "medium" | "med" | "moderate" => Some(Level::Medium),
        "high" | "critical" => Some(Level::High),
        _ => None,
    }
}

/// The level a score says when the judge gave no level: under 34 low, under 67 medium, else high.
pub fn level_of_score(score: u8) -> Level {
    match score {
        0..34 => Level::Low,
        34..67 => Level::Medium,
        _ => Level::High,
    }
}

/// The JSON object in a text answer (a fenced block, or words around it), when there is one.
fn object_in(text: &str) -> Option<Value> {
    let (a, b) = (text.find('{')?, text.rfind('}')?);
    (a < b).then(|| serde_json::from_str::<Value>(&text[a..=b]).ok()).flatten().filter(Value::is_object)
}

/// The judge's answer read, or None when it isn't a verdict wisp can read (no level and no score).
pub fn read_answer(v: &Value) -> Option<Answer> {
    let o = match v {
        Value::Object(_) => v.clone(),
        Value::String(s) => object_in(s)?,
        _ => return None,
    };
    let score = o["score"]
        .as_f64()
        .or_else(|| o["score"].as_str().and_then(|s| s.trim().parse::<f64>().ok()))
        .filter(|s| s.is_finite())
        // clamped to 0..=100 first, so the cast can't truncate or lose a sign
        .map(|s| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = s.clamp(0.0, 100.0).round() as u8;
            n
        });
    let level = o["risk"].as_str().or_else(|| o["level"].as_str()).and_then(level_of).or_else(|| score.map(level_of_score))?;
    let reasons = o["reasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_str().map(str::to_owned).or_else(|| r["text"].as_str().map(str::to_owned)))
        .map(|r| clip(&r, 300))
        .filter(|r| !r.is_empty())
        .take(MAX_REASONS)
        .collect();
    Some(Answer { level, score, summary: clip(o["summary"].as_str().unwrap_or_default(), 240), reasons })
}
