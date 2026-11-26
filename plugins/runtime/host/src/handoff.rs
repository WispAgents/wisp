//! A follow-up on another harness (v1 handoff.js, F-comp-27). Each harness keeps its own conversation, so the one
//! taking over hasn't seen the turns that ran elsewhere: they go ahead of the prompt, opening with *What still holds*
//! (the user's "don't touch / only change / leave … alone" lines). The task keeps its thinking level, as the new
//! harness names it (Claude Code `max` goes to Codex as `xhigh`), unless the switch picked one; and the old harness's
//! model doesn't go with it.

//!
//! Continue on… (ADR-0125): the follow-up may carry the latest turn only (`summary`) or every turn it missed (`full`, as
//! before); the turn is marked with the switch (`ext["wisp.runtime"].switch`: from, to, context) and the preview sheet
//! reads the same note this builds (`runtime.handoffPreview`).

use serde_json::{Value, json};
use wisp_runtime_api::Harness;
use wisp_runtime_api::commands::{HandoffChoice, HandoffContext, HandoffPreviewView};
use wisp_runtime_api::model::{Session, Turn};
use wisp_runtime_core::prompt::handoff::{Handoff, PastTurn, Render, constraints_of, handoff_note, map_effort, missed_turns};

use crate::runtime::Runtime;

/// The thinking levels `h` offers for `model` (the model's own, else the harness's; none when its id holds the level).
pub(crate) fn levels_of<'a>(h: &'a dyn Harness, model: &str) -> &'a [String] {
    let info = h.info();
    info.models.iter().find(|m| m.id == model).and_then(|m| m.efforts.as_deref()).unwrap_or(info.efforts.as_slice())
}

/// `s` handed to another harness (`new`, `h` its plugin), by a follow-up's pick or `runtime.setAgent`, both through
/// here: it resumes nothing of the old one's conversation (each harness keeps its own, and the hand-off note tells the
/// new one what it missed), its model is the new one's default unless picked, and its effort the closest level the new
/// one has unless picked (`picked_*`; kept as it is when the harness isn't known). The same harness again: nothing
/// changes, its conversation goes on.
pub(crate) fn switch_agent(s: &mut Session, new: &str, picked_model: bool, picked_effort: bool, h: Option<&dyn Harness>) {
    if s.agent == new {
        return;
    }
    new.clone_into(&mut s.agent);
    s.resume_id = None;
    if !picked_model {
        s.model.clear();
    }
    if !picked_effort && let Some(h) = h {
        s.effort = map_effort(&s.effort, levels_of(h, &s.model));
    }
}

/// What a queued follow-up asked its hand-off to carry (`opts.handoff`); a switch that didn't say carries it all.
pub(crate) fn context_of(opts: Option<&Value>) -> HandoffContext {
    opts.and_then(|o| o.get("handoff")).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// The turn's mark for a switch to another harness: what it came from, what it went to, what it carried.
pub(crate) fn switch_mark(from: &str, from_model: &str, to: &str, to_model: &str, context: HandoffContext) -> Value {
    json!({ "from": { "agent": from, "model": from_model }, "to": { "agent": to, "model": to_model }, "context": context.as_str() })
}

/// A starting turn's `ext["wisp.runtime"]`: a fresh conversation's mark (`fresh`), and the switch when it runs on
/// another harness than the task's last (`from` → `to`, as agent and model), with what the follow-up asked it to carry.
pub(crate) fn mark_turn(turn: &mut Turn, fresh: Option<Value>, from: (&str, &str), to: (&str, &str), opts: Option<&Value>) {
    let switched = (from.0 != to.0).then(|| switch_mark(from.0, from.1, to.0, to.1, context_of(opts)));
    if fresh.is_none() && switched.is_none() {
        return;
    }
    let mut m = match fresh {
        Some(Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    if let Some(sw) = switched {
        m.insert("switch".into(), sw);
    }
    turn.ext.insert("wisp.runtime".into(), Value::Object(m));
}

/// The context a turn's switch mark carried (a turn with none: everything, as before Continue on…).
fn marked_context(turn: &Turn) -> HandoffContext {
    turn.ext
        .get("wisp.runtime")
        .and_then(|r| r.get("switch"))
        .and_then(|sw| sw.get("context"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// About how many tokens a note is (4 characters a token, rounded up).
fn tokens_of(text: &str) -> u64 {
    u64::try_from(text.chars().count().div_ceil(4)).unwrap_or(u64::MAX)
}

/// The files a turn's report lists (`files` blocks: `files[]` or `items[]`, each a path or `{ path }`).
fn files_in(t: &Turn) -> Vec<String> {
    let path = |v: &Value| v.as_str().map(str::to_owned).or_else(|| v.get("path").and_then(Value::as_str).map(str::to_owned));
    t.presentation
        .iter()
        .flat_map(|p| p.blocks.iter())
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("files"))
        .flat_map(|b| b.get("files").or_else(|| b.get("items")).and_then(Value::as_array).cloned().unwrap_or_default())
        .filter_map(|v| path(&v))
        .collect()
}

/// The latest hero's summary (else its title) among `turns`, newest first.
fn last_summary(turns: &[Turn]) -> String {
    turns
        .iter()
        .rev()
        .flat_map(|t| t.presentation.iter().flat_map(|p| p.blocks.iter()))
        .find(|b| b.get("type").and_then(Value::as_str) == Some("hero"))
        .and_then(|b| b.get("summary").or_else(|| b.get("title")).and_then(Value::as_str))
        .unwrap_or("")
        .to_owned()
}

impl Runtime {
    /// What Continue on… would carry from `s` to `agent` (`runtime.handoffPreview`): both choices built by the note the
    /// hand-off itself writes, the latest summary, the files its reports list and the user's rules that go along.
    pub async fn handoff_preview(&self, id: &str, agent: &str) -> crate::error::Result<HandoffPreviewView> {
        let s = self.repo.session(id).await?;
        let mut all = self.repo.turns(id, None, None).await?;
        all.sort_by_key(|t| t.idx);
        if let Some(i) = all.iter().rposition(crate::run::is_fresh) {
            all.drain(..i);
        }
        let known = self.env_known(id).await?;
        // the turn it would run, last: on `agent`, its conversation not resumed when it's another harness's
        let next = Turn { agent: agent.to_owned(), idx: u32::try_from(all.len()).unwrap_or(u32::MAX), ..Turn::default() };
        let name = |a: &str| self.harness(a).map_or_else(|_| a.to_owned(), |h| h.info().name.clone());
        let mut seq: Vec<&Turn> = all.iter().collect();
        seq.push(&next);
        let past: Vec<PastTurn<'_>> = seq
            .iter()
            .map(|t| PastTurn { env: &known.0, ..PastTurn::new(t, name(if t.agent.is_empty() { &s.agent } else { &t.agent })) })
            .collect();
        let resume = if agent == s.agent { s.resume_id.as_deref() } else { None };
        let base = Handoff { turns: &past, agent, resume_id: resume, ..Handoff::default() };
        let missed = missed_turns(&base);
        let full = handoff_note(&base, Render::default());
        let brief = handoff_note(&Handoff { summary_only: true, ..base }, Render::default());
        let mut files: Vec<String> = Vec::new();
        for t in all.iter().rev() {
            for f in files_in(t) {
                if !files.contains(&f) && files.len() < 20 {
                    files.push(f);
                }
            }
        }
        let n = u32::try_from(missed.len()).unwrap_or(u32::MAX);
        Ok(HandoffPreviewView {
            summary: last_summary(&all),
            files,
            missed: n,
            rules: u32::try_from(constraints_of(&past).len()).unwrap_or(u32::MAX),
            brief: HandoffChoice { turns: n.min(1), tokens: tokens_of(&brief) },
            full: HandoffChoice { turns: n, tokens: tokens_of(&full) },
        })
    }

    /// `prompt` with what the harness taking `turn` missed ahead of it ('' to add when it has seen everything).
    pub(crate) async fn with_handoff(&self, s: &Session, turn: &Turn, prompt: String) -> String {
        if turn.idx == 0 {
            return prompt;
        }
        let Ok(mut all) = self.repo.turns(s.id.as_str(), None, None).await else { return prompt };
        all.retain(|t| t.id != turn.id);
        all.sort_by_key(|t| t.idx);
        all.push(turn.clone());
        // a fresh conversation (Fresh thread) hears nothing from before it: its brief is what it starts from
        if let Some(i) = all.iter().rposition(crate::run::is_fresh) {
            all.drain(..i);
        }
        // what the task's turns read of .env files (ADR-0050): the note quotes none of it, as it goes to another
        // harness and is stored as this turn's prompt. Unknown: no note rather than one that may carry a value.
        let known = match self.env_known(s.id.as_str()).await {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(task = %s.id.0, error = %e, "runtime: the task's .env reads are unknown, so the hand-off note is left out");
                return prompt;
            }
        };
        let name = |id: &str| self.harness(id).map_or_else(|_| id.to_owned(), |h| h.info().name.clone());
        let past: Vec<PastTurn<'_>> = all
            .iter()
            .map(|t| PastTurn { env: &known.0, ..PastTurn::new(t, name(if t.agent.is_empty() { &s.agent } else { &t.agent })) })
            .collect();
        // Continue on…'s summary only: the latest turn the harness missed, not all of them (ADR-0125)
        let summary_only = marked_context(turn) == HandoffContext::Summary;
        let h = Handoff { turns: &past, agent: &s.agent, resume_id: s.resume_id.as_deref(), summary_only, ..Handoff::default() };
        let note = handoff_note(&h, Render::default());
        if note.is_empty() { prompt } else { format!("{note}\n\n{prompt}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_switch_mark_records_from_to_and_the_context() {
        let m = switch_mark("claude", "opus", "codex", "", HandoffContext::Summary);
        assert_eq!(
            m,
            json!({ "from": { "agent": "claude", "model": "opus" }, "to": { "agent": "codex", "model": "" }, "context": "summary" })
        );
        assert_eq!(context_of(Some(&json!({ "handoff": "summary" }))), HandoffContext::Summary);
        assert_eq!(context_of(Some(&json!({ "handoff": "full" }))), HandoffContext::Full);
        assert_eq!(context_of(None), HandoffContext::Full, "a switch that didn't say carries it all, as before");
        let mut t = Turn::default();
        t.ext.insert("wisp.runtime".into(), json!({ "switch": m }));
        assert_eq!(marked_context(&t), HandoffContext::Summary);
        assert_eq!(tokens_of("12345"), 2);
    }
}
