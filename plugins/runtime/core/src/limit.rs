//! A run at its account's usage limit (v1 runner.js `#atLimit`, `#waitsForReset`): the hero says so, the CLI's
//! words go under it, and `turn.limit` is the card (Wait until, another of your accounts, another harness, extra
//! usage). wisp never moves a task to another account by itself: the providers' terms forbid getting around their
//! limits (design/accounts.md). With Settings → Wait for the reset on, the task carries on by itself after it.

use serde_json::{Value, json};
use wisp_runtime_api::commands::RunPick;
use wisp_runtime_api::model::{LimitAccount, Turn, TurnLimit};

/// The setting: wait for the reset by itself (v1 `defaults.limitWait: 'on'`, off by default).
pub const WAIT: &str = "runtime.limitWait";

/// When it can run again: the budget's reset of a full window when it knows, else what the error said.
pub fn until(reset_at: Option<i64>, account_until: Option<i64>, now: i64, said_ms: i64) -> i64 {
    reset_at.or(account_until).filter(|u| *u > now).unwrap_or(now + said_ms.max(0))
}

/// Who's at the limit, in the hero: the harness, with its account's name when it has one of its own ("Codex (Main)").
/// v1 said the account's label alone ("Main is at its usage limit"), which doesn't say which agent (B26).
pub fn who(harness: &str, account: Option<&str>) -> String {
    match account.map(str::trim).filter(|a| !a.is_empty() && *a != harness && !a.eq_ignore_ascii_case("default")) {
        Some(a) if !harness.is_empty() => format!("{harness} ({a})"),
        Some(a) => a.to_owned(),
        None => harness.to_owned(),
    }
}

/// The card and the hero. `who`: the account's label, else the harness's name.
pub fn at_limit(who: &str, text: &str, limit: TurnLimit) -> (TurnLimit, Vec<Value>) {
    let who = if who.is_empty() { "This harness" } else { who };
    let summary = if limit.others.is_empty() {
        "Wait for the reset, or hand the task to another agent."
    } else {
        "Wait for the reset, continue on another of your accounts, or hand the task to another agent."
    };
    // no time in the hero: the card under it says when, in the page's own time format
    let blocks = vec![
        json!({ "type": "hero", "title": format!("{who} is at its usage limit"), "summary": summary, "status": "blocked" }),
        json!({ "type": "terminal", "output": if text.is_empty() { "No output." } else { text }, "exitCode": 1 }),
    ];
    (limit, blocks)
}

/// It waits for the reset: the hero says it carries on by itself.
pub fn waiting(limit: &mut TurnLimit, blocks: &mut [Value]) {
    limit.waiting = true;
    if let Some(h) = blocks.iter_mut().find(|b| b["type"] == "hero") {
        h["summary"] = json!("It carries on by itself after the reset, in this same conversation.");
        h["status"] = json!("partial");
    }
}

/// The other accounts the card may offer: ready ones only, never this one.
pub fn others(accounts: &Value, this: &str) -> Vec<LimitAccount> {
    let this = if this.is_empty() { "default" } else { this };
    accounts["accounts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["state"] == "ready" && a["id"].as_str() != Some(this))
        .map(|a| LimitAccount {
            id: a["id"].as_str().unwrap_or_default().into(),
            label: a["label"].as_str().unwrap_or_default().into(),
            kind: a["kind"].as_str().unwrap_or("personal").into(),
        })
        .collect()
}

/// The words a CLI uses when the model itself is full, not your plan (router core's `CAPACITY`).
pub fn at_capacity(text: &str) -> bool {
    let e = text.to_ascii_lowercase();
    e.contains("model is at capacity") || e.contains("capacity unavailable")
}

/// A turn that stopped where a fallback takes over: at its usage limit (`turn.limit`) or its model at capacity.
pub fn stopped_at_limit(t: &Turn) -> bool {
    t.limit.is_some() || (t.is_error && t.text.as_deref().is_some_and(at_capacity))
}

/// The task's next fallback (`session.fallbacks`, in order) after a run stopped at a limit, and what the
/// follow-up says. `turns`: the task's turns, the one that just stopped included; `agent`: the task's harness (a
/// fallback that names none keeps it); `here`: whether a harness can run now. A fallback is skipped once a run of
/// this task stopped on it (same harness, and same model when it names one), and the task moves at most once per
/// fallback, so a list where every one is at its limit ends with the card, never a loop. These are harnesses and
/// models you named, never another account (design/accounts.md).
pub fn next_fallback(fallbacks: &[RunPick], turns: &[Turn], agent: &str, here: impl Fn(&str) -> bool) -> Option<(RunPick, String)> {
    let stopped: Vec<&Turn> = turns.iter().filter(|t| stopped_at_limit(t)).collect();
    if stopped.is_empty() || stopped.len() > fallbacks.len() {
        return None;
    }
    let tried = |a: &str, m: Option<&str>| stopped.iter().any(|t| t.agent == a && m.is_none_or(|m| t.model == m));
    let last = stopped.last()?;
    fallbacks.iter().find_map(|f| {
        let a = f.agent.as_deref().filter(|a| !a.is_empty()).unwrap_or(agent);
        let m = f.model.as_deref().filter(|m| !m.is_empty());
        // the same harness with no model of its own is the run that just stopped
        if a.is_empty() || (m.is_none() && a == last.agent) || tried(a, m) || !here(a) {
            return None;
        }
        let to = m.map_or_else(|| a.to_owned(), |m| format!("{a} {m}"));
        let was = if last.model.is_empty() { last.agent.clone() } else { format!("{} {}", last.agent, last.model) };
        let why = if last.limit.is_some() { "hit its usage limit" } else { "is at capacity" };
        let text = format!("{was} {why} · carrying on with {to}, a fallback you set. Carry on from where it stopped.");
        Some((RunPick { agent: Some(a.to_owned()), model: m.map(str::to_owned), effort: f.effort.clone().filter(|e| !e.is_empty()) }, text))
    })
}

/// The hero of a run a fallback carries on.
pub fn moving_on(blocks: &mut [Value], to: &str) {
    if let Some(h) = blocks.iter_mut().find(|b| b["type"] == "hero") {
        h["summary"] = json!(format!("It carries on with {to}, a fallback you set, in this same conversation."));
        h["status"] = json!("partial");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn limited(agent: &str, model: &str) -> Turn {
        Turn { agent: agent.into(), model: model.into(), is_error: true, limit: Some(TurnLimit::default()), ..Turn::default() }
    }
    fn pick(agent: &str, model: &str) -> RunPick {
        RunPick { agent: Some(agent.into()), model: (!model.is_empty()).then(|| model.into()), effort: None }
    }

    #[test]
    fn a_run_at_its_limit_carries_on_with_the_first_fallback_then_the_next_and_never_loops() {
        let fb = [pick("codex", "gpt-5.5"), pick("claude", "sonnet")];
        let all = |_: &str| true;
        let one = [limited("claude", "opus")];
        let (run, text) = next_fallback(&fb, &one, "claude", all).unwrap();
        assert_eq!(run, pick("codex", "gpt-5.5"));
        assert!(text.contains("claude opus hit its usage limit") && text.contains("codex gpt-5.5"), "{text}");
        // the first fallback is at its limit too: the next one, the same harness on another model
        let two = [limited("claude", "opus"), limited("codex", "gpt-5.5")];
        assert_eq!(next_fallback(&fb, &two, "codex", all).unwrap().0, pick("claude", "sonnet"));
        // every one tried: the card, no third move
        let three = [limited("claude", "opus"), limited("codex", "gpt-5.5"), limited("claude", "sonnet")];
        assert!(next_fallback(&fb, &three, "claude", all).is_none());
    }

    #[test]
    fn no_fallback_without_a_limit_or_a_harness_that_can_run() {
        let fb = [pick("codex", ""), pick("cursor", "")];
        let done = [Turn { agent: "claude".into(), is_error: true, text: Some("boom".into()), ..Turn::default() }];
        assert!(next_fallback(&fb, &done, "claude", |_| true).is_none(), "an ordinary error is not a limit");
        let one = [limited("claude", "")];
        assert_eq!(next_fallback(&fb, &one, "claude", |a| a == "cursor").unwrap().0, pick("cursor", ""), "codex isn't here");
        assert!(next_fallback(&[], &one, "claude", |_| true).is_none());
        // a fallback with no harness keeps the task's, on its own model
        let same = [RunPick { agent: None, model: Some("haiku".into()), effort: Some("low".into()) }];
        let (run, _) = next_fallback(&same, &one, "claude", |_| true).unwrap();
        assert_eq!((run.agent.as_deref(), run.model.as_deref(), run.effort.as_deref()), (Some("claude"), Some("haiku"), Some("low")));
        // the harness that just stopped, with no model of its own, is the same run: skipped
        assert!(next_fallback(&[pick("claude", "")], &one, "claude", |_| true).is_none());
    }

    #[test]
    fn a_model_at_capacity_counts_as_a_limit() {
        let cap = Turn {
            agent: "codex".into(),
            model: "gpt-5.5".into(),
            is_error: true,
            text: Some("Selected model is at capacity".into()),
            ..Turn::default()
        };
        let (_, text) = next_fallback(&[pick("codex", "gpt-5.5-mini")], &[cap], "codex", |_| true).unwrap();
        assert!(text.contains("is at capacity"), "{text}");
    }

    #[test]
    fn a_run_at_its_limit_shows_who_until_when_and_what_else_it_can_do() {
        assert_eq!(until(Some(5_000), Some(9_000), 1_000, 60_000), 5_000, "the budget's reset first");
        assert_eq!(until(None, Some(9_000), 1_000, 60_000), 9_000);
        assert_eq!(until(Some(500), None, 1_000, 60_000), 61_000, "a reset already past: what the error said");
        let accs = json!({ "accounts": [{ "id": "default", "label": "Me", "state": "ready", "kind": "personal" }, { "id": "w", "label": "Work", "state": "ready", "kind": "work" }, { "id": "x", "label": "X", "state": "limit" }] });
        let o = others(&accs, "");
        assert_eq!(o.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["w"], "ready ones, never this one");
        let (mut l, mut b) = at_limit("Claude Code", "Claude AI usage limit reached", TurnLimit { others: o, ..TurnLimit::default() });
        assert_eq!(b[0]["title"], "Claude Code is at its usage limit");
        assert!(b[0]["summary"].as_str().unwrap().contains("another of your accounts"));
        assert_eq!((b[1]["type"].as_str(), b[1]["output"].as_str()), (Some("terminal"), Some("Claude AI usage limit reached")));
        waiting(&mut l, &mut b);
        assert!(l.waiting);
        assert_eq!(b[0]["status"], "partial");
        let (_, alone) = at_limit("", "", TurnLimit::default());
        assert_eq!(
            (alone[0]["title"].as_str(), alone[1]["output"].as_str()),
            (Some("This harness is at its usage limit"), Some("No output."))
        );
        assert!(!alone[0]["summary"].as_str().unwrap().contains("accounts"));
    }

    #[test]
    fn the_hero_names_the_harness_with_its_account_never_the_account_alone_b26() {
        assert_eq!(who("Codex", Some("Main")), "Codex (Main)");
        assert_eq!(who("Claude Code", Some("Claude Code")), "Claude Code");
        assert_eq!(who("Claude Code", None), "Claude Code");
        assert_eq!(who("Codex", Some(" ")), "Codex");
        assert_eq!(who("", Some("Main")), "Main");
    }
}
