//! wisp.goal pure logic (goals: rounds, verdicts, judges, stalls), sans-IO. After each finished turn of
//! a task with a goal, its judges each say whether the goal is met; their votes (majority, all, any)
//! decide: met, another round with what they said, or, past the round limit, a stall that asks you.

use serde::{Deserialize, Serialize};

/// Turns a goal may take before it stops and asks, when nothing says.
pub const MAX_ROUNDS: u32 = 3;

/// What one judge said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub judge: String,
    pub met: bool,
    /// why not (empty when met)
    pub why: String,
    /// it couldn't judge (its run failed: no such agent, a sandbox refused): no vote on the work (audit G2)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

impl Verdict {
    /// A judge that couldn't judge, and why.
    pub fn failed(judge: &str, why: String) -> Self {
        Self { judge: judge.to_owned(), met: false, why, failed: true }
    }
}

/// A judge's answer read: `NOT MET: why` (or `not met — why`) is no, `MET` is yes; anything else is no,
/// with the judge's silence as why (an unclear verdict never ends a goal).
pub fn verdict(judge: &str, answer: &str) -> Verdict {
    let up = answer.to_ascii_uppercase();
    if let Some(at) = up.find("NOT MET") {
        let why = answer[at + 7..].trim_start_matches([':', ' ', '-', '—']).lines().next().unwrap_or_default();
        let why = why.split(['"', '`']).next().unwrap_or_default().trim().to_owned();
        return Verdict {
            judge: judge.to_owned(),
            met: false,
            why: if why.is_empty() { "the goal isn't met yet".into() } else { why },
            failed: false,
        };
    }
    if up.contains("MET") {
        return Verdict { judge: judge.to_owned(), met: true, why: String::new(), failed: false };
    }
    Verdict { judge: judge.to_owned(), met: false, why: format!("{judge} didn't say whether it's met"), failed: false }
}

/// The votes counted: `all`, `any`, else a majority (a tie isn't met).
pub fn met(vote: &str, v: &[Verdict]) -> bool {
    let yes = v.iter().filter(|x| x.met).count();
    match vote {
        "all" => !v.is_empty() && yes == v.len(),
        "any" => yes > 0,
        _ => yes * 2 > v.len(),
    }
}

/// What happens after a judged round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Met,
    /// another round, told this
    Again(String),
    /// the rounds are spent: ask the user
    Stall(String),
}

/// After round `round` (1-based) of at most `max`.
pub fn next(vote: &str, v: &[Verdict], round: u32, max: u32) -> Next {
    if met(vote, v) {
        return Next::Met;
    }
    let why: Vec<String> = v.iter().filter(|x| !x.met).map(|x| format!("{}: {}", x.judge, x.why)).collect();
    let why = why.join("\n");
    // no judge said what's missing, one or more couldn't judge at all: the work isn't sent back for that (it would go
    // round to its limit on a judge that never ran), it stops once and says why (audit G2)
    if v.iter().any(|x| x.failed) && v.iter().all(|x| x.met || x.failed) {
        let failed: Vec<&str> = v.iter().filter(|x| x.failed).map(|x| x.why.as_str()).collect();
        return Next::Stall(format!(
            "The goal can't be judged: {}\nThe work isn't sent back for that. Fix the judge (or set the goal's judges), then check it again.",
            failed.join("; ")
        ));
    }
    if round >= max.max(1) {
        let rounds = if round == 1 { "1 round".to_owned() } else { format!("{round} rounds") };
        Next::Stall(format!("The goal isn't met after {rounds}.\n{why}\nKeep going?"))
    } else {
        Next::Again(format!("The goal isn't met yet. What the judges said:\n{why}\nKeep going until it is."))
    }
}

/// The prompt a judge gets.
pub fn judge_prompt(goal: &str, title: &str, answer: &str) -> String {
    format!(
        "You judge whether a task met its goal. Answer with one line: MET, or NOT MET: <what's missing>.\n\nGoal: {goal}\nTask: {title}\nIts last answer:\n{answer}"
    )
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_votes_and_rounds() {
        assert!(verdict("r", "MET").met);
        let no = verdict("r", "Fake: NOT MET: the tests still fail\nmore");
        assert_eq!((no.met, no.why.as_str()), (false, "the tests still fail"));
        assert!(!verdict("r", "looks fine").met, "unclear is no");
        let v = |m: &[bool]| {
            m.iter()
                .enumerate()
                .map(|(i, &met)| Verdict { judge: format!("j{i}"), met, why: if met { String::new() } else { "x".into() }, failed: false })
                .collect::<Vec<_>>()
        };
        assert!(met("majority", &v(&[true, true, false])) && !met("majority", &v(&[true, false])));
        assert!(met("any", &v(&[false, true])) && !met("all", &v(&[false, true])) && !met("all", &[]));
        assert_eq!(next("all", &v(&[true]), 1, 3), Next::Met);
        assert!(matches!(next("all", &v(&[false]), 1, 3), Next::Again(_)));
        assert!(matches!(next("all", &v(&[false]), 3, 3), Next::Stall(s) if s.contains("after 3 rounds")));
    }

    /// Audit G2: a judge that couldn't judge (no harness `critic`) never sends the work back: the goal stops at once,
    /// saying why once, instead of going round to its limit; a judge that did say NOT MET still sends it back. G16:
    /// "after 1 round", not "1 rounds".
    #[test]
    fn a_judge_that_cant_judge_stops_the_goal_once() {
        let broke = Verdict::failed("critic", "critic couldn't judge: no harness critic".into());
        match next("majority", std::slice::from_ref(&broke), 1, 4) {
            Next::Stall(q) => {
                assert!(q.starts_with("The goal can't be judged: critic couldn't judge: no harness critic"), "{q}");
                assert_eq!(q.matches("no harness critic").count(), 1, "said once: {q}");
            }
            other => panic!("sent back: {other:?}"),
        }
        let said = verdict("reviewer", "NOT MET: tests fail");
        assert!(matches!(next("all", &[broke, said], 1, 4), Next::Again(_)), "a real NOT MET still sends it back");
        assert!(matches!(next("all", &[verdict("r", "NOT MET: x")], 1, 1), Next::Stall(s) if s.contains("after 1 round.")));
    }
}

/// A scheduled entry on the task, as `wisp.later` lists it: who set it, whether it's a reminder, when it was set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    pub id: String,
    pub by: String,
    pub remind: bool,
    pub set_at: i64,
}

/// v1 #775: a turn whose agent scheduled its own wake-up (`resume_later`) during it says it's waiting, not done. The
/// goal isn't checked on it but after the wake-up's turn: that wake-up's id. Yours ('user'), wisp's own goal rounds
/// ('wisp') and reminders don't count; nor does one set before this turn started.
#[must_use]
pub fn waits_for_wake(scheduled: &[Scheduled], turn_started_at: i64) -> Option<String> {
    scheduled.iter().find(|l| !l.remind && l.by != "user" && l.by != "wisp" && l.set_at >= turn_started_at).map(|l| l.id.clone())
}

/// What the goal does when the wake-up it waited for went without going (`later.dropped`): you cancelled it, so the
/// round's report stands and is checked now; otherwise (the task marked done, missed while wisp was off) it waits for you.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeGone {
    CheckNow,
    Pause,
}

#[must_use]
pub fn wake_gone(why: &str) -> WakeGone {
    if why == "cancelled" { WakeGone::CheckNow } else { WakeGone::Pause }
}
