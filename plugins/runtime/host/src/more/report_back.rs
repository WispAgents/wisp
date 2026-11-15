//! A lead that owes its delegates' reports (v1 runner.js `#awaiting`, `#delegateDone`, `#reportBack`; the rules are
//! `wisp_runtime_core::owed`'s). Looked at when the lead's turn ends (`finish.rs`), when one of its delegates' turns
//! ends, when its own run lets go, and as wisp starts: what ended while the lead's run is off goes to it at once, once;
//! the rest go on with that follow-up's turn.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::model::{QueueItem, Session, SessionState, Turn, TurnStatus};
use wisp_runtime_core::blocks::render::Context;
use wisp_runtime_core::owed::{self, DELEGATIONS, Next, OWED, Open, WAITS_ON};
use wisp_runtime_core::prompt::subagents::{Member, delegation_report};
use wisp_runtime_core::queue::{self, Queue, ReportBack, Run};

use super::{SLOT, slot, str_at, with_slot};
use crate::error::Result;
use crate::runtime::Runtime;

/// How many of the lead's last turns are looked through for the one that owes (held or refused follow-ups between).
const OWED_LOOKBACK: u32 = 8;

/// One delegation the lead owes: its record and how its run stands.
pub(crate) struct OwedRec {
    rec: Value,
    open: Open,
}

/// A delegate's turn: the one its record names, else its task's last (a record from before the turn was known).
async fn delegate_turn(rt: &Runtime, rec: &Value) -> Option<(Session, Option<Turn>)> {
    let child = rt.repo.session(&str_at(rec, "sessionId")).await.ok()?;
    let tid = str_at(rec, "turnId");
    let turn = if tid.is_empty() {
        rt.repo.turns(child.id.as_str(), None, Some(1)).await.ok().and_then(|mut t| t.pop())
    } else {
        rt.repo.turn(&tid).await.ok()
    };
    Some((child, turn))
}

fn records(t: &Turn) -> &[Value] {
    t.ext.get(SLOT).and_then(|x| x.get(DELEGATIONS)).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// The lead's slot without who it waits on; true when it said so.
fn clear_waits(s: &mut Session) -> bool {
    with_slot(&mut s.ext, |x| x.remove(WAITS_ON).is_some())
}

impl Runtime {
    /// The delegations on `t` the lead owes and wisp watches from here (its own children: not a sandbox's, another
    /// machine's or a crew member's job, which it reads with `subagent_results`).
    pub(crate) async fn owed_on(&self, lead: &Session, t: &Turn) -> Vec<OwedRec> {
        let jobs = slot(lead).and_then(|m| m.get("crewJobs")).and_then(Value::as_object);
        let mut out = Vec::new();
        for rec in records(t).iter().filter(|d| owed::is_owed(d) && !owed::is_away(d, jobs)) {
            // gone (deleted) counts as ended: its report says so
            let status = delegate_turn(self, rec).await.and_then(|(_, t)| t).map(|t| t.status);
            let open = Open {
                name: str_at(rec, "name"),
                running: status == Some(TurnStatus::Running),
                stopped: status == Some(TurnStatus::Stopped),
            };
            out.push(OwedRec { rec: rec.clone(), open });
        }
        out
    }

    /// Each owed delegation's report, as the lead reads it (v1 `delegationReport`).
    async fn reports_of(&self, list: &[OwedRec]) -> Vec<String> {
        let mut out = Vec::with_capacity(list.len());
        let name_of = |id: &str| self.harness(id).ok().map(|h| h.info().name.clone());
        for d in list {
            let m = Member {
                name: str_at(&d.rec, "name"),
                agent: str_at(&d.rec, "agent"),
                model: str_at(&d.rec, "model"),
                ..Member::default()
            };
            let (status, report, files) = match delegate_turn(self, &d.rec).await {
                Some((_, Some(t))) => {
                    let report = t.presentation.as_ref().map_or_else(
                        || t.text.clone().unwrap_or_default(),
                        |p| {
                            wisp_runtime_core::blocks::blocks_to_markdown(
                                &p.blocks,
                                &Context { changes: &[], approval: None, summary: None },
                            )
                        },
                    );
                    let files: Vec<String> = t
                        .ext
                        .get(SLOT)
                        .and_then(|x| x.get("changes"))
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|c| str_at(c, "file"))
                        .filter(|f| !f.is_empty())
                        .collect();
                    (Some(t.status), report, files)
                }
                _ => (None, String::new(), Vec::new()),
            };
            out.push(delegation_report(&m, status, &report, &files, &name_of));
        }
        out
    }

    /// The lead's own turn ended (`finish.rs`, under its lock, before its landing). Some: the delegates it still owes
    /// at work, and it stays in Working on them. Some of them ended (v1 `#reportBack` with `ending`): their reports are
    /// queued now (its release starts them; that turn takes the rest over) and marked read on `t`, in the same write as
    /// the turn's end.
    pub(crate) async fn lead_turn_ends(&self, s: &mut Session, t: &mut Turn, stopped: bool) -> Option<Vec<String>> {
        if s.parent_id.is_some() {
            return None;
        }
        if !stopped {
            // one elsewhere (a sandbox, another machine, a crew job) isn't waited on: the lead reads it with
            // `subagent_results`, and it isn't carried into its next turns as still at work
            let jobs = slot(s).and_then(|m| m.get("crewJobs")).and_then(Value::as_object).cloned();
            with_slot(&mut t.ext, |x| {
                if let Some(Value::Array(list)) = x.get_mut(DELEGATIONS) {
                    for d in list.iter_mut().filter(|d| owed::is_owed(d) && owed::is_away(d, jobs.as_ref())) {
                        if let Some(o) = d.as_object_mut() {
                            o.insert(OWED.into(), Value::Bool(false));
                        }
                    }
                }
            });
        }
        let list = self.owed_on(s, t).await;
        let opens: Vec<Open> = list.iter().map(|d| d.open.clone()).collect();
        match owed::next(&opens, false, stopped) {
            Next::Hold(names) => Some(names),
            Next::Nothing => None,
            Next::Report(ended) => {
                let list = pick(list, &ended);
                let reports = self.reports_of(&list).await;
                let ids: Vec<String> = list.iter().map(|d| str_at(&d.rec, "sessionId")).collect();
                with_slot(&mut t.ext, |x| owed::mark_read(x, &ids));
                self.queue_report(s, &reports, Run::Idle, true);
                None
            }
        }
    }

    /// wisp's follow-up with these reports, in the lead's queue: merged into one waiting there, else added. Queued, not
    /// started here: the write that marks them read is the one that queues it, so a restart in between loses nothing.
    fn queue_report(&self, s: &mut Session, reports: &[String], run: Run, ending: bool) {
        let item = QueueItem {
            id: wisp_kernel::ids::uuid7(),
            text: owed::report_text(reports),
            at: self.now(),
            by: Some("wisp".into()),
            reports: true,
            ..QueueItem::default()
        };
        let mut held = s.queue_held;
        let joined = reports.join("\n\n---\n\n");
        if let ReportBack::Start(item) = queue::report_back(&mut Queue { items: &mut s.queue, held: &mut held }, run, ending, item, &joined)
        {
            s.queue.push(item);
        }
    }

    /// The lead's latest turn that still owes a delegation (of its last few, newest first; `skip`: a turn being started):
    /// a follow-up held before it ran, or refused, comes after it without taking its records over.
    async fn owed_turn(&self, id: &str, skip: Option<&wisp_runtime_api::TurnId>) -> Result<Option<Turn>> {
        let mut recent = self.repo.turns(id, None, Some(OWED_LOOKBACK)).await?;
        recent.sort_by_key(|t| std::cmp::Reverse(t.idx));
        Ok(recent.into_iter().filter(|t| skip != Some(&t.id)).find(|t| records(t).iter().any(owed::is_owed)))
    }

    /// The next queued item of an idle lead, claimed under its lock (as `release_run` does, without touching a live
    /// run's entry): what `spawn_start` starts once the lock is let go.
    async fn claim_next(self: &Arc<Self>, s: &mut Session) -> Result<Option<(crate::claims::Claim, Session, QueueItem)>> {
        let id = s.id.0.clone();
        if s.queue_held || self.run_state(&id) != Run::Idle || self.claims.count(&id) > 0 {
            return Ok(None);
        }
        let mut held = s.queue_held;
        let Some(next) = queue::next_after_release(&mut Queue { items: &mut s.queue, held: &mut held }) else { return Ok(None) };
        self.persist(s, Vec::new()).await?;
        Ok(Some((self.claims.claim(&id), s.clone(), next)))
    }

    /// Who a lead that stays in Working on its delegates waits on, on its slot (wisp.later reads it: v1 #795).
    pub(crate) fn note_waits(s: &mut Session, names: &[String]) {
        with_slot(&mut s.ext, |x| {
            x.insert(WAITS_ON.into(), json!(names));
        });
    }

    /// Looks at lead `id` again (one of its delegates' turns ended, its run let go, wisp started): the ones that ended go
    /// to it now (the follow-up's turn takes the rest over); or it still waits; or nothing. A lead that's running, starting
    /// or was stopped is left to its own run's end.
    pub(crate) async fn check_lead(self: &Arc<Self>, id: &str) -> Result<()> {
        let next = {
            let _g = self.task_lock(id).await;
            let mut s = self.repo.session(id).await?;
            if s.parent_id.is_some() {
                return Ok(());
            }
            let Some(last) = self.repo.turns(id, None, Some(1)).await?.pop() else { return Ok(()) };
            // a turn of it running, starting or held before it ran (Start now runs it, and it takes them over)
            let busy = self.run_state(id) != Run::Idle
                || self.claims.count(id) > 0
                || matches!(last.status, TurnStatus::Running | TurnStatus::Held);
            let owed_turn = self.owed_turn(id, None).await?;
            let list = match &owed_turn {
                Some(t) => self.owed_on(&s, t).await,
                None => Vec::new(),
            };
            let opens: Vec<Open> = list.iter().map(|d| d.open.clone()).collect();
            match (owed::next(&opens, busy, last.status == TurnStatus::Stopped), owed_turn) {
                (Next::Hold(names), _) => {
                    let said = slot(&s).and_then(|m| m.get(WAITS_ON)).cloned();
                    if !busy && said != Some(json!(names)) {
                        Self::note_waits(&mut s, &names);
                        self.persist(&mut s, Vec::new()).await?;
                    }
                    None
                }
                (Next::Report(ended), Some(mut t)) => {
                    let list = pick(list, &ended);
                    let reports = self.reports_of(&list).await;
                    let ids: Vec<String> = list.iter().map(|d| str_at(&d.rec, "sessionId")).collect();
                    with_slot(&mut t.ext, |x| owed::mark_read(x, &ids));
                    clear_waits(&mut s);
                    self.queue_report(&mut s, &reports, Run::Idle, false);
                    // a queue the user held keeps it there for them: the lead waits on you, not on wisp
                    (s.state, s.unread) = if s.queue_held { (SessionState::Review, true) } else { (SessionState::Running, false) };
                    self.persist(&mut s, vec![t]).await?;
                    self.claim_next(&mut s).await?
                }
                _ => {
                    // it waited, and nothing is owed any more (a stop, a read): it lands, quietly
                    if !busy && clear_waits(&mut s) {
                        if s.state == SessionState::Running && s.queue.is_empty() {
                            s.state = SessionState::Review;
                        }
                        self.persist(&mut s, Vec::new()).await?;
                    }
                    // their report was queued and nothing started it (wisp stopped in between): it starts now
                    if !busy && s.queue.iter().any(|q| q.reports) { self.claim_next(&mut s).await? } else { None }
                }
            }
        };
        if let Some((claim, s, item)) = next {
            self.spawn_start(s, item, claim);
        }
        Ok(())
    }

    /// `check_lead` on the plugin's scope. A sync step on purpose (see `spawn_start`): a turn's end or a run's release
    /// awaiting it would make their futures contain themselves.
    pub(crate) fn spawn_check_lead(self: &Arc<Self>, id: &str) {
        let (rt, id) = (self.clone(), id.to_owned());
        self.host.spawn(Box::pin(async move {
            if let Err(err) = rt.check_lead(&id).await {
                tracing::warn!(task = %id, %err, "runtime: a lead's delegates, looked at again");
            }
        }));
    }

    /// `subagent_results` handed the lead these reports (`ids`, finished): it has them, and wisp won't send them again.
    pub(crate) async fn mark_reports_read(self: &Arc<Self>, lead: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let _g = self.task_lock(lead).await;
        let mut s = self.repo.session(lead).await?;
        let Some(mut t) = self.owed_turn(lead, None).await? else { return Ok(()) };
        if with_slot(&mut t.ext, |x| owed::mark_read(x, ids)) {
            self.persist(&mut s, vec![t]).await?;
        }
        Ok(())
    }

    /// The user stopped lead `s` (`stop`, under its lock): it's owed nothing (v1: a stopped run owes nothing, and Stop
    /// stops its delegates too). Its last turn, marked, to write with it; and its own children still at work, to stop.
    pub(crate) async fn stop_owed(&self, s: &mut Session) -> Result<(Option<Turn>, Vec<String>)> {
        clear_waits(s);
        if s.parent_id.is_some() {
            return Ok((None, Vec::new()));
        }
        let Some(mut t) = self.owed_turn(s.id.as_str(), None).await? else { return Ok((None, Vec::new())) };
        let kids: Vec<String> =
            self.owed_on(s, &t).await.into_iter().filter(|d| d.open.running).map(|d| str_at(&d.rec, "sessionId")).collect();
        let changed = with_slot(&mut t.ext, |x| {
            let Some(Value::Array(list)) = x.get_mut(DELEGATIONS) else { return false };
            let mut changed = false;
            for d in list.iter_mut().filter(|d| owed::is_owed(d)) {
                if let Some(o) = d.as_object_mut() {
                    o.insert(OWED.into(), Value::Bool(false));
                    changed = true;
                }
            }
            changed
        });
        Ok((changed.then_some(t), kids))
    }

    /// A new turn of lead `s` takes over the delegations its last turn still owes (v1 `#startTurn`); `turn` gets them.
    /// Under the lead's lock, as the turn is written. The records taken, for the still-working note.
    pub(crate) async fn take_over_owed(&self, s: &mut Session, turn: &mut Turn) -> Result<(Option<Turn>, Vec<Value>)> {
        clear_waits(s);
        if s.parent_id.is_some() {
            return Ok((None, Vec::new()));
        }
        let Some(mut prev) = self.owed_turn(s.id.as_str(), Some(&turn.id)).await? else {
            return Ok((None, Vec::new()));
        };
        let taken = match prev.ext.get_mut(SLOT) {
            Some(Value::Object(x)) => owed::take_owed(x),
            _ => Vec::new(),
        };
        if taken.is_empty() {
            return Ok((None, Vec::new()));
        }
        with_slot(&mut turn.ext, |x| {
            let list = x.entry(DELEGATIONS).or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(a) = list {
                a.extend(taken.iter().cloned());
            }
        });
        Ok((Some(prev), taken))
    }
}

/// The records `owed::next` picked (indexes into `list`), in order.
fn pick(list: Vec<OwedRec>, at: &[usize]) -> Vec<OwedRec> {
    list.into_iter().enumerate().filter(|(i, _)| at.contains(i)).map(|(_, d)| d).collect()
}

/// Delegates the lead's last turn still owed went on with `turn` (`taken`): its prompt says who, ahead of the user's
/// words (v1 `stillWorkingNote`). Only for a lead with a team, never a compaction.
pub(crate) fn still_working(s: &Session, turn: &Turn, taken: &[Value], prompt: &mut String) {
    let team = s.ext_of(SLOT)["subagents"].as_array().is_some_and(|t| !t.is_empty());
    if !team || taken.is_empty() || turn.by.as_deref() == Some("compact") {
        return;
    }
    let who: Vec<(String, String)> = taken.iter().map(|d| (str_at(d, "name"), str_at(d, "sessionId"))).collect();
    *prompt = format!("{}\n\n{prompt}", wisp_runtime_core::prompt::subagents::still_working_note(&who));
}
