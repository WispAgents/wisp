//! Compacting, rerunning a signed-out turn, adopting a shared or outside session, and forking.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::commands::{FollowUpArgs, FollowedUp, ForkArgs, RerunArgs};
use wisp_runtime_api::model::{Session, SessionState, Turn, TurnStatus};
use wisp_runtime_api::{SessionId, TurnId};

use super::{SLOT, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// What of its task's own slot a fork leaves behind: what belongs to one run or turn (an open question, a retry's
/// extras, a /btw, Allow-always grants, a held queue's marks, side proposals and the applied ones its agent has yet to hear of), or the task's place among others (a side discussion's
/// main task, a compare's group, a crew job, a pin). Everything else is the task's setup and carries over: its spec and
/// the connectors it names, a formula's connector overlay, its subagents, variant, brief, `heldTo`, `startedBy`, cap.
const NOT_FORKED: [&str; 30] = [
    "next",
    "btw",
    "ask",
    "asks",
    "forLater",
    "blocked",
    "grants",
    "approval",
    "rerun",
    "readOnly",
    "sideOf",
    "forkOf",
    "proposals",
    "sideApplied",
    "terminal",
    "setupFailed",
    "spawnFailed",
    "guardTask",
    "overage",
    "trial",
    "group",
    "titleAuto",
    "pinned",
    "watch",
    "member",
    "machine",
    "crewJobs",
    "away",
    "broughtJobs",
    "broughtNote",
];

/// Other plugins' slots that are the task's setup, not a run's state: a fork keeps them as runtime's own is kept.
const FORKED_SLOTS: [&str; 1] = [wisp_router_api::PLUGIN_ID];

/// git's slot on a turn: the checkpoint it kept before the turn ran (`runtime.run@1` `before_turn`).
const GIT: &str = "wisp.git";

/// The fork's ext: the task's own slot without `NOT_FORKED`, and the setup slots of `FORKED_SLOTS`.
fn forked_ext(src: &Session) -> wisp_runtime_api::model::Ext {
    let mut ext = wisp_runtime_api::model::Ext::default();
    if let Some(Value::Object(own)) = src.ext.get(SLOT) {
        let kept: serde_json::Map<String, Value> =
            own.iter().filter(|(k, v)| !NOT_FORKED.contains(&k.as_str()) && !v.is_null()).map(|(k, v)| (k.clone(), v.clone())).collect();
        ext.insert(SLOT.to_owned(), Value::Object(kept));
    }
    for k in FORKED_SLOTS {
        if let Some(v) = src.ext.get(k).filter(|v| !v.is_null()) {
            ext.insert(k.to_owned(), v.clone());
        }
    }
    ext
}

impl Runtime {
    /// `runtime.compact`: `/compact` as the next turn, by wisp (it doesn't count as your follow-up).
    pub async fn compact(self: &Arc<Self>, id: &SessionId) -> Result<FollowedUp> {
        self.follow_up(FollowUpArgs {
            id: id.clone(),
            text: "/compact".into(),
            queue: Some(true),
            by: Some("compact".into()),
            ..FollowUpArgs::default()
        })
        .await
    }

    /// `runtime.rerun` (v1 `rerunSignedOut`): a signed-out turn's request again, once, in the same
    /// conversation, on another account if one is given.
    pub async fn rerun(self: &Arc<Self>, a: RerunArgs) -> Result<FollowedUp> {
        let none = FollowedUp { turn_id: None, queued: None, interrupts: false };
        let t = {
            let _g = self.task_lock(a.id.as_str()).await;
            let mut s = self.repo.session(a.id.as_str()).await?;
            let Some(mut t) = self.last_turn(a.id.as_str()).await?.filter(|t| t.id == a.turn_id) else { return Ok(none) };
            let signed_out = t.ext.get(SLOT).and_then(|x| x.get("authFailed")).and_then(Value::as_bool).unwrap_or(false);
            let again = t.ext.get(SLOT).and_then(|x| x.get("rerun")).is_some();
            if !signed_out || again || t.status != TurnStatus::Error || s.state == SessionState::Running || !s.queue.is_empty() {
                return Ok(none);
            }
            // once only: the mark is saved before anything is sent (two tabs, a phone, a restart)
            with_slot(&mut t.ext, |m| m.insert("rerun".into(), json!(self.now())));
            if let Some(acc) = a.account.filter(|x| !x.is_empty()) {
                s.account = Some(acc);
            }
            self.persist(&mut s, vec![t.clone()]).await?;
            t
        };
        // the run goes through `before:run` like any turn: the guard may hold it (v1's `guard.check`)
        let sent = self
            .follow_up(FollowUpArgs {
                id: a.id.clone(),
                text: t.prompt.clone(),
                attachments: t.attachments.clone(),
                queue: Some(false),
                by: t.by.clone(),
                ..FollowUpArgs::default()
            })
            .await;
        if sent.is_err() {
            // it never went: the card offers it again (v1 runner.js:2036)
            let _g = self.task_lock(a.id.as_str()).await;
            let mut s = self.repo.session(a.id.as_str()).await?;
            let mut t = self.repo.turn(t.id.as_str()).await?;
            with_slot(&mut t.ext, |m| m.remove("rerun"));
            self.persist(&mut s, vec![t]).await?;
        }
        sent
    }

    /// `runtime.adopt`: a read-only task holding someone else's turns (a `full` share); fork it to go on.
    pub async fn adopt(self: &Arc<Self>, a: wisp_runtime_api::spec::AdoptArgs) -> Result<wisp_runtime_api::commands::Created> {
        let now = self.now();
        // a fresh task brought in (Settings → Worktrees) names no harness: it's the one its next follow-up runs on, New
        // task's last pick, else the first, as `runtime.create` picks (audit I IS13: its head read "· Default model" and
        // its box "Follow up with …")
        let agent = if a.agent.trim().is_empty() && a.fresh { self.default_agent() } else { a.agent };
        let mut s = Session {
            id: SessionId::new(),
            title: a.title,
            agent,
            cwd: a.cwd,
            state: SessionState::Done,
            created_at: now,
            updated_at: now,
            ..Session::default()
        };
        for (k, v) in a.ext {
            s.ext.insert(k, v);
        }
        let spec = serde_json::to_value(&a.spec).unwrap_or_default();
        // an outside session stays live: its follow-ups continue it in its CLI (ADR-0035)
        if a.resume_id.is_some() {
            s.resume_id.clone_from(&a.resume_id);
        } else if !a.fresh {
            with_slot(&mut s.ext, |m| m.insert("readOnly".into(), json!(true)));
        }
        with_slot(&mut s.ext, |m| m.insert("spec".into(), spec));
        let copies: Vec<Turn> = a
            .turns
            .into_iter()
            .filter_map(|t| serde_json::from_value::<Turn>(t).ok())
            .map(|t| Turn { id: TurnId::new(), session_id: s.id.clone(), ..t })
            .collect();
        if copies.is_empty() && !a.fresh {
            return Err(RuntimeError::Invalid("a shared task to import has its turns (a full share)".into()));
        }
        let summary = self.persist(&mut s, copies).await?;
        self.host.emit(crate::host::Emitted::Created(wisp_runtime_api::events::SessionCreated { session: (*summary).clone() }));
        Ok(wisp_runtime_api::commands::Created { id: s.id, turn_id: None })
    }

    /// An outside session already adopted (ADR-0035: never twice): its task, with the session's turns past
    /// the ones it has appended. None when no task came from it.
    pub async fn readopt(
        self: &Arc<Self>,
        session: &str,
        a: &wisp_runtime_api::spec::AdoptArgs,
    ) -> Result<Option<wisp_runtime_api::commands::Created>> {
        // adopted from it before, or wisp's own task that ran as it (`wisp open` comes back to it)
        let found = self.repo.sessions_naming(session).await?.into_iter().find(|s| {
            s.ext.get("wisp.adopted").and_then(|v| v.get("session")).and_then(Value::as_str) == Some(session)
                || s.resume_id.as_deref() == Some(session)
        });
        let Some(mut s) = found else { return Ok(None) };
        let have = self.repo.turns(s.id.as_str(), None, None).await?.len();
        let new: Vec<Turn> = a
            .turns
            .iter()
            .skip(have)
            .filter_map(|t| serde_json::from_value::<Turn>(t.clone()).ok())
            .enumerate()
            .map(|(i, t)| Turn { id: TurnId::new(), session_id: s.id.clone(), idx: u32::try_from(have + i).unwrap_or(u32::MAX), ..t })
            .collect();
        if !new.is_empty() {
            s.updated_at = self.now();
            self.persist(&mut s, new).await?;
        }
        Ok(Some(wisp_runtime_api::commands::Created { id: s.id, turn_id: None }))
    }

    /// `runtime.fork`: a new task with this one's turns up to `turn_id`, resuming a copy of its conversation as it was
    /// at that turn, with the task's setup (its connectors, team, variant, lean, limits) and none of its runs' state.
    pub async fn fork(self: &Arc<Self>, a: ForkArgs) -> Result<wisp_runtime_api::commands::Created> {
        let src = self.repo.session(a.id.as_str()).await?;
        let upto = self.turn_of(a.id.as_str(), a.turn_id.as_ref()).await?;
        let all = self.repo.turns(a.id.as_str(), None, None).await?;
        let earlier = all.iter().any(|t| t.idx > upto.idx);
        // the copy goes on from the turn asked, never with what came after it: a harness that shares one conversation
        // across turns ends its copy where that turn did; one that can't is refused, never forked with later context
        let mut next = json!({ "fork": true });
        let mut forked_at = json!({ "id": src.id, "turnId": upto.id });
        if earlier {
            let h = self.harness(&src.agent).ok();
            let at = upto.ext.get(SLOT).and_then(|x| x.get("resumeAt")).and_then(Value::as_str).filter(|a| !a.is_empty());
            let Some(at) = at.filter(|_| h.as_ref().is_some_and(|h| h.info().fork)) else {
                let name = h.map_or_else(|| src.agent.clone(), |h| h.info().name.clone());
                return Err(RuntimeError::Invalid(format!(
                    "{name} can’t go back to an earlier turn’s conversation: fork from the latest turn instead."
                )));
            };
            next["at"] = json!(at);
            // its files as they were then: the checkpoint git kept just before the turn after it (`kept_before`)
            if let Some(cp) = all.iter().find(|t| t.idx == upto.idx + 1).and_then(|t| t.ext.get(GIT)?.get("checkpoint")).cloned() {
                forked_at["checkpoint"] = cp;
            }
        }
        let turns: Vec<Turn> = all.into_iter().filter(|t| t.idx <= upto.idx).collect();
        if a.new_id.as_ref() == Some(&a.id) {
            return Err(RuntimeError::Invalid("a fork is a new task: its id isn't the one it forks".into()));
        }
        // the same fork sent again (after a restart): the copy it made is the answer
        if let Some(id) = a.new_id.clone()
            && self.claimed(&id).await?
        {
            return Ok(wisp_runtime_api::commands::Created { id, turn_id: None });
        }
        let id = self.new_task_id(a.new_id.clone()).await?;
        let now = self.now();
        let default_title =
            if earlier { format!("{} (fork at turn {})", src.title, upto.idx + 1) } else { format!("{} (fork)", src.title) };
        let mut s = Session {
            id,
            title: a.title.filter(|t| !t.trim().is_empty()).unwrap_or(default_title),
            agent: src.agent.clone(),
            model: src.model.clone(),
            effort: src.effort.clone(),
            account: src.account.clone(),
            lean: src.lean.clone(),
            mode: src.mode,
            cwd: src.cwd.clone(),
            dirs: src.dirs.clone(),
            tags: src.tags.clone(),
            state: SessionState::Review,
            resume_id: upto.resume_id.clone().or_else(|| src.resume_id.clone()),
            created_at: now,
            updated_at: now,
            ..Session::default()
        };
        // the task's setup carries over whole (a capped task's fork stays under its cap, v1 capped.js; its connectors and
        // team, so its first run has the same MCP servers), its runs' and turns' state doesn't
        s.ext = forked_ext(&src);
        with_slot(&mut s.ext, |m| m.insert("forkOf".into(), forked_at));
        with_slot(&mut s.ext, |m| m.insert("next".into(), next));
        let copies: Vec<Turn> = turns.into_iter().map(|t| Turn { id: TurnId::new(), session_id: s.id.clone(), ..t }).collect();
        let summary = self.persist(&mut s, copies).await?;
        self.host.emit(crate::host::Emitted::Created(wisp_runtime_api::events::SessionCreated { session: (*summary).clone() }));
        Ok(wisp_runtime_api::commands::Created { id: s.id, turn_id: None })
    }
}
