//! An agent's questions and live approvals (v1 runner.js `ask`, `requestApproval`, `answer`, `pollAnswer`).

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{AskArgs, PollAnswer};
use wisp_runtime_api::harness::PermissionAsk;
use wisp_runtime_api::model::{Session, SessionState, ToolKind};

use super::{POLL_MS, push, slot, str_at, tool_row, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// What v1 gave a question before it gave up waiting (adapters/common.js `ASK_TIMEOUT_MS`).
pub(crate) const ASK_TIMEOUT_MS: i64 = 30 * 60_000;
/// The answers a live approval offers (v1 `APPROVAL`). Typed text instead is a "no, do this instead".
pub(crate) const APPROVAL_ALLOW: &str = "Allow";
pub(crate) const APPROVAL_ALWAYS: &str = "Always allow in this task";
pub(crate) const APPROVAL_DENY: &str = "Deny";
/// A question asked while a setup card is open (ADR-0113).
pub(crate) const SETUP_OPEN: &str = "A change to the user's setup is waiting on their answer: ask again once they've answered it.";
/// What a setup card's agent reads when the card goes without the user's answer (its turn ended, the task stopped).
pub(crate) const TAKEN_BACK: &str = "The card was taken back before the user answered it (the task stopped or its turn ended): nothing changed. Ask again if it's still needed.";
/// What a second answer to a card already answered (or taken back) gets: nothing runs twice.
pub(crate) const ALREADY: &str = "That card was already answered: nothing ran again.";
/// How long an answer its agent never reads is kept for `pollAnswer` (an ask's own timeout, v1 `ASK_TIMEOUT_MS`).
pub(crate) const ANSWER_KEEP_MS: i64 = ASK_TIMEOUT_MS;
/// How long an answered or taken-back question's id is remembered: far longer than any write in flight.
pub(crate) const CLOSED_KEEP_MS: i64 = 6 * 60 * 60_000;
/// An answer to a question that's no longer the open one.
pub(crate) const NOT_OPEN: &str = "That question was already answered, or a newer one replaced it: nothing was sent.";
/// An answer to a question when none is open now: answered already, or taken back as the task or its factory run
/// stopped (audit H tasks NEW #1: a stopped run's answer read as "already answered").
pub(crate) const NONE_OPEN: &str =
    "That question isn't open any more (it was answered, or taken back when the task or its run stopped): nothing was sent.";
const SIDE_DENIED: &str = "Refused: a side discussion only reads. Ask the main task to do it.";

/// The open question of `row`, as a write is about to store it, settled against the stored one (`stored`): an ask
/// answered or taken back (`closed`) is never stored again (a stale copy still had it), and an open one (`open`, not
/// closed) the copy left out is put back. A task whose turn runs keeps its state (a turn started from an older copy
/// isn't shown Waiting); otherwise it waits on the question again, or stops waiting. True when it changed `row`.
pub(crate) fn reconcile_ask(row: &mut Session, stored: Option<&Value>, open: impl Fn(&str) -> bool, closed: impl Fn(&str) -> bool) -> bool {
    if let Some(id) = ask_id(row) {
        if !closed(&id) {
            return false;
        }
        tracing::warn!(task = %row.id, "a write from an older copy of the task would have brought back an answered question: dropped");
        with_slot(&mut row.ext, |m| m.remove("ask"));
        if row.state == SessionState::Waiting {
            row.state = SessionState::Review;
        }
        return true;
    }
    let Some(ask) = stored.filter(|a| a.get("id").and_then(Value::as_str).is_some_and(|id| open(id) && !closed(id))) else {
        return false;
    };
    tracing::warn!(task = %row.id, "a write from an older copy of the task would have dropped its open question: kept");
    with_slot(&mut row.ext, |m| m.insert("ask".into(), ask.clone()));
    if row.state != SessionState::Running {
        row.state = SessionState::Waiting;
    }
    true
}

/// The open setup card on a task (ADR-0113), if its open question is one.
pub(crate) fn open_setup(s: &Session) -> Option<Value> {
    slot(s)?.get("ask")?.get("setup").cloned()
}

/// Whether the open question is one about a safety guard the user's harness asked for (ADR-0115).
pub(crate) fn open_guard(s: &Session) -> bool {
    slot(s).and_then(|v| v.get("ask")).and_then(|a| a.get("guard")) == Some(&Value::Bool(true))
}

/// What a follow-up or an answer from elsewhere reads on such a task.
pub(crate) const GUARD_ASKS_YOU: &str = "This task asks you whether your harness may change one of wisp's safety guards: answer it with its buttons or wisp reply <id> Allow|Deny, not a follow-up. Nothing was sent.";

/// A follow-up to a task whose question is a guard's: refused (ADR-0115).
pub(crate) fn no_follow_up_on_guard(s: &Session) -> crate::error::Result<()> {
    if open_guard(s) { Err(crate::error::RuntimeError::Conflict(GUARD_ASKS_YOU.into())) } else { Ok(()) }
}

/// The open question's id, from the task itself (the truth across a restart).
pub(crate) fn ask_id(s: &Session) -> Option<String> {
    slot(s)?.get("ask")?.get("id")?.as_str().map(str::to_owned)
}

/// A live approval's answer (v1 `approvalDecision`): a button, a yes/no typed out, or anything
/// else, which denies the call and hands the agent the user's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub allow: bool,
    pub always: bool,
    pub message: Option<String>,
}

pub(crate) fn approval_decision(text: &str) -> Decision {
    let t = text.trim();
    let l = t.to_lowercase();
    let yes = |allow, always| Decision { allow, always, message: None };
    if t == APPROVAL_ALWAYS || l.starts_with("always") || l == "2" {
        return yes(true, true);
    }
    if t == APPROVAL_ALLOW || matches!(l.as_str(), "y" | "yes" | "ok" | "allow" | "approve" | "go ahead" | "1") {
        return yes(true, false);
    }
    if t == APPROVAL_DENY || matches!(l.as_str(), "n" | "no" | "deny" | "reject" | "3") {
        return yes(false, false);
    }
    Decision { allow: false, always: false, message: Some(t.to_owned()) }
}

impl Runtime {
    /// `runtime.ask`: the agent's question goes on its task, which waits on the user (v1 `ask`).
    pub async fn ask(self: &Arc<Self>, a: AskArgs) -> Result<String> {
        let context = a.details.as_ref().and_then(|d| d.get("context")).and_then(Value::as_str).unwrap_or_default().to_owned();
        // `guard`: the user's harness asks about one of wisp's safety guards (ADR-0115): only the user's own hands answer
        // it (`runtime.answer` from the page, the CLI, the phone or a channel's tap), never a follow-up
        let guard = a.details.as_ref().and_then(|d| d.get("guard")) == Some(&Value::Bool(true));
        self.ask_in(&a.id, &a.question, &context, &a.options, guard.then_some(("guard", Value::Bool(true)))).await
    }

    /// Puts a question on task `id`: `state: waiting`, unread, back from the archive, and an
    /// "Asked you: …" (or "Needs your OK to …") row on its turn. A question already open is replaced.
    pub(crate) async fn ask_in(
        self: &Arc<Self>,
        id: &SessionId,
        question: &str,
        context: &str,
        options: &[String],
        permission: Option<(&'static str, Value)>,
    ) -> Result<String> {
        let (tid, label, ask_id) = {
            let _g = self.task_lock(id.as_str()).await;
            let mut s = self.repo.session(id.as_str()).await?;
            // a setup card stays until the user answers it (ADR-0113): nothing asked after it replaces it
            if open_setup(&s).is_some() {
                return Err(RuntimeError::Conflict(SETUP_OPEN.into()));
            }
            if let Some(old) = ask_id(&s) {
                self.asks().remove(&old);
                self.close_ask(&old);
            }
            let ask_id = wisp_kernel::ids::uuid7();
            let question = if question.trim().is_empty() { "The agent has a question.".to_owned() } else { question.trim().to_owned() };
            let options: Vec<String> = options.iter().take(6).cloned().collect();
            // `permission` (a live approval) or `setup` (ADR-0113), each with the `action` its row names
            let label = match permission.as_ref().and_then(|(_, p)| p.get("action")).and_then(Value::as_str) {
                Some(action) => format!("Needs your OK to {action}"),
                None => format!("Asked you: {question}"),
            };
            let mut ask = json!({ "id": ask_id, "question": question, "context": context, "options": options, "askedAt": self.now() });
            if let (Some((key, p)), Some(o)) = (permission, ask.as_object_mut()) {
                o.insert(key.into(), p);
            }
            with_slot(&mut s.ext, |m| m.insert("ask".into(), ask));
            s.state = SessionState::Waiting;
            s.unread = true;
            s.archived = false; // a question needs the user: it comes back to the feed
            self.asks().insert(ask_id.clone(), s.id.0.clone());
            self.persist(&mut s, Vec::new()).await?;
            self.host.emit(crate::host::Emitted::Task("task.asked", crate::host::fact(&s, None, "asked", &question, None)));
            (self.last_turn(id.as_str()).await?.map(|t| t.id), label, ask_id)
        };
        if let Some(t) = tid {
            self.add_row(id.as_str(), t.as_str(), tool_row(ToolKind::Ask, label)).await?;
        }
        Ok(ask_id)
    }

    /// `runtime.answer`: the user's answer to the open question. `ask`: the question it answers (a card's, a channel's
    /// or the phone's button names it): refused when that one isn't the open one any more.
    pub async fn answer(self: &Arc<Self>, id: &SessionId, text: &str, ask: Option<&str>) -> Result<Value> {
        let _g = self.task_lock(id.as_str()).await;
        let s = self.repo.session(id.as_str()).await?;
        if let Some(want) = ask {
            match ask_id(&s) {
                None => return Err(RuntimeError::Conflict(NONE_OPEN.into())),
                Some(open) if open != want => return Err(RuntimeError::Conflict(NOT_OPEN.into())),
                Some(_) => {}
            }
        }
        self.answer_locked(id, text).await
    }

    /// The answer, under the task's lock: kept for `pollAnswer`, the question taken off the task, a
    /// "You answered: …" row. For a live approval, Always adds a grant for this task and a denial is
    /// recorded on the turn (v1 `answer`).
    pub(crate) async fn answer_locked(self: &Arc<Self>, id: &SessionId, text: &str) -> Result<Value> {
        let mut s = self.repo.session(id.as_str()).await?;
        let Some(ask) = with_slot(&mut s.ext, |m| m.remove("ask")) else {
            return Err(RuntimeError::Conflict("Nothing is being asked on this task.".into()));
        };
        let ask_id = ask.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let mut turns = Vec::new();
        let last = self.last_turn(id.as_str()).await?;
        let label = match ask.get("permission") {
            Some(p) => {
                let d = approval_decision(text);
                let (tool, detail, action) = (str_at(p, "tool"), str_at(p, "detail"), str_at(p, "action"));
                if d.always {
                    with_slot(&mut s.ext, |m| push(m, "grants", json!({ "tool": tool, "detail": detail })));
                }
                if !d.allow
                    && let Some(mut t) = last.clone()
                {
                    with_slot(&mut t.ext, |m| push(m, "denied", json!({ "tool": tool, "detail": detail })));
                    turns.push(t);
                }
                if d.allow {
                    format!("You allowed{}: {action}", if d.always { " (always in this task)" } else { "" })
                } else {
                    format!("You denied: {action}{}", d.message.map(|m| format!(" (“{m}”)")).unwrap_or_default())
                }
            }
            None => format!("You answered: {text}"),
        };
        self.asks().remove(&ask_id);
        self.close_ask(&ask_id);
        self.keep_answer(&ask_id, text);
        s.state =
            if self.run_state(id.as_str()) == wisp_runtime_core::queue::Run::Idle { SessionState::Review } else { SessionState::Running };
        self.persist(&mut s, turns).await?;
        if let Some(t) = last {
            self.add_row(id.as_str(), t.id.as_str(), tool_row(ToolKind::Answer, label)).await?;
        }
        Ok(json!({ "answered": true, "askId": ask_id }))
    }

    /// `runtime.unask`: the open question taken back when it is still `question` (its owner's, word for word): the
    /// task stops waiting, as a stop leaves it (`cancel_asks`), and its turn says the question went. Another question
    /// asked since stays. `{ withdrawn }` says which.
    pub async fn unask(self: &Arc<Self>, id: &SessionId, question: &str) -> Result<Value> {
        let _g = self.task_lock(id.as_str()).await;
        let mut s = self.repo.session(id.as_str()).await?;
        let open = slot(&s).and_then(|m| m.get("ask")).and_then(|a| a.get("question")).and_then(Value::as_str).map(str::to_owned);
        if open.is_none_or(|q| q != question.trim()) {
            return Ok(json!({ "withdrawn": false }));
        }
        self.cancel_asks(&mut s);
        self.persist(&mut s, Vec::new()).await?;
        if let Some(t) = self.last_turn(id.as_str()).await? {
            self.add_row(id.as_str(), t.id.as_str(), tool_row(ToolKind::Answer, "The question was taken back".to_owned())).await?;
        }
        Ok(json!({ "withdrawn": true }))
    }

    /// `runtime.pollAnswer`: the answer once (then it's gone), pending while the question is on its
    /// task (also after a restart), else cancelled.
    pub fn poll_answer(&self, ask_id: &str) -> PollAnswer {
        let now = self.now();
        self.answers().retain(|_, (_, at)| now - *at < ANSWER_KEEP_MS);
        if let Some((answer, _)) = self.answers().remove(ask_id) {
            return PollAnswer::Answered { answer };
        }
        if self.asks().contains_key(ask_id) { PollAnswer::Pending } else { PollAnswer::Cancelled }
    }

    /// The task stopped: what its agent asked goes unanswered, and the task stops waiting. A setup card's agent reads
    /// that it was taken back and nothing changed (audit G GS5: a card is never taken back without an answer).
    pub(crate) fn cancel_asks(&self, s: &mut Session) {
        if let Some(ask) = with_slot(&mut s.ext, |m| m.remove("ask")) {
            let id = ask.get("id").and_then(Value::as_str).unwrap_or_default();
            self.asks().remove(id);
            self.close_ask(id);
            if ask.get("setup").is_some() {
                self.keep_answer(id, TAKEN_BACK);
            }
            if s.state == SessionState::Waiting {
                s.state = SessionState::Review;
            }
        }
    }

    /// A write of task `s` made from a copy read before its question went up (no task lock held) would drop the open
    /// question, its card gone while its agent's `pollAnswer` reads pending for good (audit G GS5). Every path that
    /// takes a question back takes it out of `asks` first, so a question `asks` still holds for this task that the
    /// write leaves out is put back from the stored row, with the task still waiting.
    ///
    /// This one runs before the write, so its row (the list's) says the same; `reconcile_ask` runs again inside the
    /// write's transaction, where nothing can come between the read and the write (review r194 of GS5).
    pub(crate) async fn keep_open_ask(&self, s: &mut Session) {
        let open_here = self.asks().values().any(|sid| *sid == s.id.0);
        let has = slot(s).is_some_and(|m| m.get("ask").is_some());
        if !open_here && !has {
            return;
        }
        let stored = self.repo.session(s.id.as_str()).await.ok();
        let stored_ask = stored.as_ref().and_then(|x| slot(x)).and_then(|m| m.get("ask")).cloned();
        reconcile_ask(s, stored_ask.as_ref(), |a| self.asks().contains_key(a), |a| self.closed().contains_key(a));
    }

    /// The question `id` is answered or taken back: no write brings it back and no second answer runs it again.
    pub(crate) fn close_ask(&self, id: &str) {
        let now = self.now();
        let mut c = self.closed();
        c.retain(|_, at| now - *at < CLOSED_KEEP_MS);
        c.insert(id.to_owned(), now);
    }

    /// `text` kept for the agent's `pollAnswer` on question `id`; one never read goes after [`ANSWER_KEEP_MS`].
    pub(crate) fn keep_answer(&self, id: &str, text: &str) {
        let now = self.now();
        let mut a = self.answers();
        a.retain(|_, (_, at)| now - *at < ANSWER_KEEP_MS);
        a.insert(id.to_owned(), (text.to_owned(), now));
    }

    /// Waits for the answer on the kernel's clock, at most `ms`.
    async fn wait_answer(&self, ask_id: &str, ms: i64) -> Option<String> {
        let until = self.now() + ms;
        loop {
            match self.poll_answer(ask_id) {
                PollAnswer::Answered { answer } => return Some(answer),
                PollAnswer::Pending if self.now() < until => self.clock.sleep(POLL_MS).await,
                _ => return None,
            }
        }
    }

    /// `runtime.approval` (v1 `requestApproval`): a live permission prompt. What the user allowed
    /// always in this task goes through at once; else Allow / Always allow in this task / Deny.
    /// Answers Claude Code's permission-prompt-tool shape.
    #[allow(clippy::many_single_char_names)] // s, h, p, a, d: v1 requestApproval's own names
    pub async fn approval(self: &Arc<Self>, a: wisp_runtime_api::commands::ApprovalArgs) -> Result<Value> {
        let s = self.repo.session(a.id.as_str()).await?;
        let h = self.harness(&s.agent)?;
        // a harness that can't say what the call does: the grant covers only this exact input
        let p = h.permission(&a.tool, &a.input, &s.cwd).unwrap_or_else(|| PermissionAsk {
            tool: a.tool.clone(),
            action: format!("use {}", a.tool),
            detail: a.input.to_string(),
            why: None,
        });
        let last = self.last_turn(a.id.as_str()).await?;
        // a side discussion on a harness that can't ask before a write only reads (v1 `sideMode`); on one that can,
        // its writes and commands are asked about as any task's (v1 #794: side discussions run commands)
        if slot(&s).and_then(|m| m.get("sideOf")).is_some() && !h.info().side_gate {
            if let Some(t) = &last {
                self.add_row(
                    a.id.as_str(),
                    t.id.as_str(),
                    tool_row(ToolKind::Answer, format!("Refused (a side discussion only reads): {}", p.action)),
                )
                .await?;
            }
            return Ok(json!({ "behavior": "deny", "message": SIDE_DENIED }));
        }
        let granted = slot(&s)
            .and_then(|m| m.get("grants"))
            .and_then(Value::as_array)
            .is_some_and(|g| g.iter().any(|x| str_at(x, "tool") == p.tool && str_at(x, "detail") == p.detail));
        if granted {
            if let Some(t) = &last {
                self.add_row(
                    a.id.as_str(),
                    t.id.as_str(),
                    tool_row(ToolKind::Answer, format!("Allowed (always in this task): {}", p.action)),
                )
                .await?;
            }
            return Ok(json!({ "behavior": "allow", "updatedInput": a.input }));
        }
        let options = [APPROVAL_ALLOW, APPROVAL_ALWAYS, APPROVAL_DENY].map(str::to_owned);
        let question = format!("Allow {} to {}?", h.info().name, p.action);
        let permission = json!({ "tool": p.tool, "action": p.action, "detail": p.detail, "toolUseId": a.tool_use_id });
        let ask_id =
            self.ask_in(&a.id, &question, p.why.as_deref().unwrap_or_default(), &options, Some(("permission", permission))).await?;
        // v1 internal.js: an approval that asked you (not one an "always" already let through) counts
        self.host.track("approval", json!({ "agent": s.agent }));
        let Some(answer) = self.wait_answer(&ask_id, ASK_TIMEOUT_MS).await else {
            return Ok(json!({ "behavior": "deny", "message": "No answer from the user in time, or the task was stopped." }));
        };
        let d = approval_decision(&answer);
        Ok(if d.allow {
            json!({ "behavior": "allow", "updatedInput": a.input })
        } else {
            json!({ "behavior": "deny", "message": d.message.unwrap_or_else(|| "The user denied this.".into()) })
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{Decision, approval_decision};

    /// F-page-3 (v1 `approvalDecision`): one box answers a live approval; a yes or no typed out is that answer, and any
    /// other words deny the call and go to the agent as what to do instead.
    #[test]
    fn a_reply_typed_under_an_approval_denies_the_call_and_hands_the_agent_the_words() {
        let d = |allow, always, message: Option<&str>| Decision { allow, always, message: message.map(str::to_owned) };
        for yes in ["Allow", "yes", "y", "OK", "go ahead", "1"] {
            assert_eq!(approval_decision(yes), d(true, false, None), "{yes}");
        }
        for always in ["Always allow in this task", "always", "2"] {
            assert_eq!(approval_decision(always), d(true, true, None), "{always}");
        }
        for no in ["Deny", "no", "n", "reject", "3"] {
            assert_eq!(approval_decision(no), d(false, false, None), "{no}");
        }
        assert_eq!(approval_decision("  use pnpm publish --dry-run instead "), d(false, false, Some("use pnpm publish --dry-run instead")));
        // "yes, but…" isn't a plain yes: it denies, with the words
        assert_eq!(approval_decision("yes but only on staging"), d(false, false, Some("yes but only on staging")));
    }
}
