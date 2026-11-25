//! Ending a turn and landing its task (v1 runner.js `#finishTurn`, `release`), applied in one transaction:
//! the turn's head (status, answer, stats, captured edits), the task's state, then `turn.ended` always and
//! `turn.finished` when someone should hear of it. The decisions are `wisp_runtime_core::land`'s.

use std::sync::Arc;

use serde_json::json;
use wisp_runtime_api::events::{TurnEnded, TurnFinished};
use wisp_runtime_api::harness::Harness;
use wisp_runtime_api::model::{Session, Turn, TurnStatus};
use wisp_runtime_core::activity::{Effect, Head};
use wisp_runtime_core::land::{self, Ending, Facts};
use wisp_runtime_core::queue::{self, Queue};

use crate::drive::Drive;
use crate::error::Result;
use crate::host::Emitted;
use crate::runtime::Runtime;

impl Runtime {
    /// The run's result is in: flush its activity, end the turn, land the task.
    pub(crate) async fn finish(self: &Arc<Self>, d: &mut Drive, e: Effect) {
        d.finished = true;
        d.coal.flush().await;
        // what the turn read, known before its row is written: a title or preview from its answer masks it (ADR-0050)
        self.env_remember(&d.sid, &d.head.env_values);
        if let Some(l) = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_mut(&d.sid) {
            l.finished = true;
        }
        let n_events = u32::try_from(d.seq).unwrap_or(u32::MAX);
        if let Err(err) = self.end_turn(&d.sid, &d.tid, &d.head, e, n_events).await {
            tracing::warn!(task = %d.sid, %err, "a turn's end was not recorded");
        }
    }

    async fn end_turn(self: &Arc<Self>, sid: &str, tid: &str, head: &Head, e: Effect, n_events: u32) -> Result<()> {
        let Effect::Finish { text, is_error, stopped, stats, auth_failed, stop_reason } = e else {
            return Ok(());
        };
        // a lead's delegates elsewhere are asked (left to right) before its lock: never a machine's link under it
        let (far, _guard) = (self.far_ahead(sid).await, self.task_lock(sid).await);
        let mut s = self.repo.session(sid).await?;
        let mut turn = self.repo.turn(tid).await?;
        let (stopping, interrupted) = self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(sid)
            .map_or((false, false), |l| (l.stopping || l.interrupting, l.interrupting));
        let h = self.harness(&turn.agent).ok();
        let harness = h.as_ref().map(|h| h.info().name.clone()).unwrap_or_default();
        let auth_failed = auth_failed || said_signed_out(h.as_deref(), is_error && !stopping, &text);
        let ending = Ending {
            stopped: stopped || stopping,
            interrupted,
            is_error,
            text: &text,
            harness: &harness,
            changes: &head.changes,
            reason: None,
            signed_out: auth_failed,
        };
        let blocks = self.host.blocks();
        let chat_mode = self.on_unless_switched_off("settings.transcript") && s.parent_id.is_none() && !s.ext.contains_key("wisp.crew");
        let (status, mut presentation) = land::end_presentation(&ending, |t| {
            if chat_mode {
                wisp_runtime_core::blocks::presentation_chat(t, &blocks)
            } else {
                wisp_runtime_core::blocks::presentation(t, &blocks)
            }
        });
        // a usage limit first (v1 `#atLimit`), then a refusal (v1 `#refused`, never at a limit)
        let ours = s.parent_id.is_none() && !(stopped || stopping);
        let limited = ours && is_error && self.at_limit(&s, &mut turn, &text, &mut presentation).await;
        if ours && !limited {
            refuse(&mut turn, &mut presentation, &text, &stop_reason, is_error, head.changes.len(), &harness, h.as_deref());
        }
        turn.status = status;
        turn.ended_at = Some(self.now());
        turn.stats = stats.as_ref().map(land::turn_stats);
        turn.text = head.text.clone().or_else(|| (!text.is_empty()).then(|| text.clone()));
        turn.is_error = status == TurnStatus::Error;
        turn.changed_files = u32::try_from(head.changes.len()).ok().filter(|n| *n > 0);
        turn.n_events = n_events;
        turn.resume_id.clone_from(&head.resume_id);
        keep_facts(&mut turn, head, auth_failed);
        turn.presentation = Some(presentation);
        self.prepare_done(&mut s, &mut turn, status).await;
        if head.resume_id.is_some() {
            s.resume_id.clone_from(&head.resume_id);
        }
        // v1 runner.js:1812: a question the turn left open goes with it (no stale ask in review); a setup card's agent
        // reads that it was taken back, and the turn says so (audit G GS5: never silently)
        let card = crate::more::open_card_action(&s);
        self.cancel_asks(&mut s);
        // a lead: its delegates still at work hold it in Working; all of them done, their reports are queued now
        let waits_on = self.lead_turn_ends(&mut s, &mut turn, stopped || stopping, &far).await;
        let (mut landing, later_by) = self.landing(&s, &turn, stopped || stopping, interrupted, waits_on.is_some());
        if let Some(names) = waits_on.filter(|_| landing.state == wisp_runtime_api::model::SessionState::Running && !landing.handing) {
            Self::note_waits(&mut s, &names);
        }
        let waits = self.waits_reset(&s, &mut turn, &mut landing);
        self.apply_landing(&mut s, &landing, later_by.as_deref());
        let (s0, t0, l) = (s.clone(), turn.clone(), &landing); // its events ride on the write (ADR-0074)
        self.persist_emitting(&mut s, vec![turn.clone()], |sum| Self::end_events(&s0, &t0, interrupted, l, sum, &text)).await?;
        // a delegate's turn ended (a child, or a crew job a lead handed out): its lead may have waited on it last
        if let Some(lead) = crate::more::lead_of(&s) {
            self.spawn_check_lead(&lead);
        }
        if let (true, Some(l)) = (waits, turn.limit.as_ref()) {
            let (rt, sid, agent, until) = (self.clone(), s.id.0.clone(), turn.agent.clone(), l.until);
            tokio::spawn(async move { rt.schedule_reset(&sid, &agent, until).await });
        }
        if let Some(id) = landing.run_after
            && let Some(step) = self.host.after_steps().into_iter().find(|a| a.id() == id)
        {
            let (s2, t2) = (s.clone(), turn.clone());
            tokio::spawn(async move {
                if let Err(err) = step.run(s2, t2).await {
                    tracing::warn!(%err, "an after-turn step failed");
                }
            });
        }
        if let Some(action) = card {
            let label = format!("The card asking to {action} was taken back: the turn ended before you answered. Nothing changed.");
            self.add_row(sid, tid, crate::more::answer_row(label)).await?;
        }
        if landing.handing {
            self.release_linger(sid);
        }
        Ok(())
    }

    /// Strips plugin-internal blocks (`TurnOutcome::prepare`), applies turn-1 auto-titling from `topic`,
    /// and keeps media attachments referenced by the final blocks.
    async fn prepare_done(self: &Arc<Self>, s: &mut Session, turn: &mut Turn, status: TurnStatus) {
        if status == TurnStatus::Done && turn.by.as_deref() != Some("compact") {
            for o in self.host.outcomes() {
                o.prepare(s, turn);
            }
        }
        if status == TurnStatus::Done
            && turn.idx == 0
            && title_auto(s)
            && let Some(p) = turn.presentation.as_ref()
            && let Some(t) = land::topic_title(p)
        {
            s.title = t;
        }
        if let Some(p) = turn.presentation.as_mut() {
            self.keep_media(s, turn.id.as_str(), &mut p.blocks).await;
            self.keep_docs(s, &mut p.blocks).await;
        }
    }

    /// Writes `landing`'s state, unread, archive, For later and timestamp onto `s`.
    fn apply_landing(&self, s: &mut Session, landing: &land::Landing, later_by: Option<&str>) {
        s.state = landing.state;
        s.unread = landing.unread;
        if landing.archive {
            s.archived = true;
        }
        let now = self.now();
        if landing.for_later {
            let own = s.ext.entry("wisp.runtime".into()).or_insert_with(|| json!({}));
            if !own.is_object() {
                *own = json!({});
            }
            if let Some(o) = own.as_object_mut() {
                o.insert("forLater".into(), json!({ "at": now, "by": later_by.unwrap_or("wisp") }));
            }
        }
        s.updated_at = now;
    }

    fn landing(&self, s: &Session, turn: &Turn, stopped: bool, interrupted: bool, owes: bool) -> (land::Landing, Option<String>) {
        let picked = self
            .host
            .outcomes()
            .into_iter()
            .find_map(|o| o.outcome(s, turn).map(|x| (x, o.id().strip_prefix("wisp.").unwrap_or(o.id()).to_owned())));
        let (outcome, by) = match picked {
            Some((o, by)) => (o, Some(by)),
            None => (wisp_runtime_api::points::Outcome::default(), None),
        };
        let facts = Facts {
            stopped,
            interrupted,
            follow_up_waits: !s.queue.is_empty() && !s.queue_held,
            owes,
            compact: turn.by.as_deref() == Some("compact"),
            sub: s.parent_id.is_some(),
            ..Facts::default()
        };
        let end = wisp_runtime_api::points::TurnEnd { stopped, is_error: turn.is_error };
        let mut steps = self.host.after_steps();
        steps.sort_by_key(|a| a.order());
        (land::land(facts, outcome, || steps.iter().find(|a| a.wants(s, turn, end)).map(|a| a.id().to_owned())), by)
    }

    /// What a turn's end tells, in order: `turn.ended`, `task.finished`/`failed`, `turn.finished` if someone should hear.
    fn end_events(
        s: &Session,
        turn: &Turn,
        interrupted: bool,
        l: &land::Landing,
        summary: &wisp_runtime_api::model::Summary,
        text: &str,
    ) -> Vec<Emitted> {
        let error_text = (turn.status == TurnStatus::Error).then(|| text.to_owned());
        let cost = turn.stats.as_ref().and_then(|x| x.cost_usd);
        let mut out = vec![Emitted::Ended(TurnEnded {
            session_id: s.id.clone(),
            turn_id: turn.id.clone(),
            status: turn.status,
            interrupted,
            error_text,
            cost_usd: cost,
            changed_files: turn.changed_files,
            agent: turn.agent.clone(),
            model: turn.model.clone(),
            account: turn.account.clone().or_else(|| s.account.clone()),
            tokens: turn.stats.as_ref().and_then(|x| {
                // cache reads and writes aside: harnesses count them differently, and Drain watch only compares a
                // harness's runs with its own
                let t = x.input_tokens.unwrap_or(0) + x.output_tokens.unwrap_or(0);
                (t > 0).then_some(t)
            }),
            input_tokens: turn.stats.as_ref().and_then(|x| x.input_tokens),
            output_tokens: turn.stats.as_ref().and_then(|x| x.output_tokens),
            cache_tokens: turn.stats.as_ref().and_then(|x| x.cache_tokens),
            cache_read_tokens: turn.stats.as_ref().and_then(cache_reads),
            first: turn.idx == 0,
            inner: s.parent_id.is_some() || matches!(turn.by.as_deref(), Some("wisp" | "compact")),
        })];
        match turn.status {
            TurnStatus::Done => out.push(Emitted::Task("task.finished", crate::host::fact(s, Some(&turn.id), "done", text, cost))),
            TurnStatus::Error => out.push(Emitted::Task("task.failed", crate::host::fact(s, Some(&turn.id), "error", text, cost))),
            _ => {}
        }
        if l.notify {
            out.push(Emitted::Finished(TurnFinished {
                session_id: s.id.clone(),
                turn_id: turn.id.clone(),
                status: turn.status,
                interrupted,
                summary: summary.clone(),
            }));
        }
        out
    }

    /// A turn that failed before its process ran (the spawn failed).
    pub(crate) async fn finish_now(self: &Arc<Self>, sid: &str, tid: &str, why: &str) -> Result<()> {
        let e = Effect::Finish {
            text: why.to_owned(),
            is_error: true,
            stopped: false,
            stats: None,
            auth_failed: false,
            stop_reason: String::new(),
        };
        self.end_turn(sid, tid, &Head::default(), e, 0).await
    }

    /// A task's last turn failed in its harness's signed-out words but was saved before wisp read them on a failed end
    /// (an older wisp): it's landed again as signed out (plain words, the sign-in card, the row's Signed out), never the
    /// raw API error as the task's line (QA desktop r3; r2 #6 fixed new runs only).
    pub(crate) async fn repair_signed_out(self: &Arc<Self>, id: &str) -> Result<()> {
        let _guard = self.task_lock(id).await;
        let Some(mut t) = self.repo.turns(id, None, Some(1)).await?.pop() else { return Ok(()) };
        let marked = t.ext.get("wisp.runtime").and_then(|r| r.get("authFailed")).and_then(serde_json::Value::as_bool) == Some(true);
        let text = t.text.clone().unwrap_or_default();
        let Ok(h) = self.harness(&t.agent) else { return Ok(()) };
        if t.status != TurnStatus::Error || marked || !said_signed_out(Some(h.as_ref()), true, &text) {
            return Ok(());
        }
        let ending = Ending {
            stopped: false,
            interrupted: false,
            is_error: true,
            text: &text,
            harness: &h.info().name,
            changes: &[],
            reason: None,
            signed_out: true,
        };
        let (_, p) = land::end_presentation(&ending, |_| wisp_runtime_api::model::Presentation::default());
        t.presentation = Some(p);
        keep_in_slot(&mut t, [("authFailed", json!(true))]);
        let mut s = self.repo.session(id).await?;
        self.persist(&mut s, vec![t]).await.map(drop)
    }

    /// A turn a crash or restart left running never finished: it shows stopped (v1 `restartReason`).
    pub(crate) async fn end_unfinished(self: &Arc<Self>, mut s: Session, mut t: Turn) -> Result<()> {
        // its setup card goes with the run that asked (ADR-0113): nobody is left to read the answer
        // (a plain question stays: only the card goes, so only its id is closed)
        if let Some(ask) = crate::more::ask_id(&s).filter(|_| crate::more::open_setup(&s).is_some()) {
            self.asks().remove(&ask);
            self.close_ask(&ask);
        }
        let taken = crate::more::take_setup_card(&mut s).map(|action| {
            format!("wisp restarted while this was running, so its card asking to {action} was taken back: nothing changed. Follow up to pick it back up.")
        });
        let ending = Ending {
            stopped: true,
            interrupted: false,
            is_error: false,
            text: "",
            harness: "",
            changes: &[],
            reason: Some(("Stopped", taken.as_deref().unwrap_or("wisp restarted while this was running. Follow up to pick it back up."))),
            signed_out: false,
        };
        let (status, p) = land::end_presentation(&ending, |_| wisp_runtime_api::model::Presentation::default());
        t.status = status;
        t.presentation = Some(p);
        t.ended_at = Some(t.started_at);
        s.state = wisp_runtime_api::model::SessionState::Review;
        self.persist(&mut s, vec![t]).await?;
        Ok(())
    }

    /// The run let go: the live entry goes, and the next queued follow-up starts (unless a Stop held them).
    pub(crate) async fn release_run(self: &Arc<Self>, sid: &str) {
        let next = {
            let _guard = self.task_lock(sid).await;
            // let go under the task's lock (audit C2): a follow-up deciding in between found the task idle and started
            // a turn of its own beside the queue's next, on the same turn index
            self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(sid);
            self.pending_interrupt.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(sid);
            let Ok(mut s) = self.repo.session(sid).await else {
                return;
            };
            let mut held = s.queue_held;
            let next = queue::next_after_release(&mut Queue { items: &mut s.queue, held: &mut held });
            if next.is_some() && self.persist(&mut s, Vec::new()).await.is_err() {
                return;
            }
            // the next run is a queued follow-up's own: a later implicit follow-up waits behind it (audit I3); an explicit
            // direct one (`queue: Some(false)`, as the reply box sends) still interrupts it (runtime.rs `follow_up`)
            {
                let mut fq = self.from_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if next.is_some() {
                    fq.insert(sid.to_owned());
                } else {
                    fq.remove(sid);
                }
            }
            // claimed under the task's lock, before its start is spawned: a delete (it takes the lock) either came
            // first (no row: nothing starts) or finds this claim and waits for it; claimed in the spawned task, a
            // delete in between waited on nothing and the start's `folder_ready` could bring a worktree back for it
            let waits =
                next.is_none() && s.parent_id.is_none() && s.ext_of("wisp.runtime").get(wisp_runtime_core::owed::WAITS_ON).is_some();
            (next.map(|n| ((self.claims.claim(sid), self.mark_starting(sid)), s, n)), waits)
        };
        match next {
            (Some((claim, s, n)), _) => self.spawn_start(s, n, claim),
            // a lead waiting on its delegates let go: one may have ended while its run was still on
            (None, true) => self.spawn_check_lead(sid),
            (None, false) => {}
        }
    }

    /// Starts a queued turn on its own task, on the plugin's scope. A sync step on purpose: a run's end starting the
    /// next run would otherwise make the run's future contain itself, and its `Send` couldn't be proven.
    /// `claim`: the start's claim and its starting mark, both taken under the task's lock where it was decided.
    pub(crate) fn spawn_start(
        self: &Arc<Self>,
        s: Session,
        n: wisp_runtime_api::model::QueueItem,
        claim: (crate::claims::Claim, crate::runtime::Starting),
    ) {
        let rt = self.clone();
        let sid = s.id.0.clone();
        self.host.spawn(Box::pin(async move {
            let (claim, starting) = claim;
            if let Err(err) = rt.start_claimed(s, n, claim).await {
                drop(starting);
                rt.from_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&sid);
                tracing::warn!(task = %sid, %err, "the next queued follow-up did not start");
                if !matches!(err, crate::error::RuntimeError::NoTask(_)) {
                    let _guard = rt.task_lock(&sid).await;
                    if rt.run_state(&sid) == queue::Run::Idle
                        && let Ok(mut cur) = rt.repo.session(&sid).await
                        && cur.state == wisp_runtime_api::model::SessionState::Running
                    {
                        cur.state = wisp_runtime_api::model::SessionState::Review;
                        cur.unread = true;
                        let _ = rt.persist(&mut cur, Vec::new()).await;
                    }
                }
            }
        }));
    }
}

/// What the turn's end keeps in runtime's slot on it, merged: what the turn already keeps there (its delegations, a
/// checkpoint) stays (replaced, a lead forgot the delegates it owed).
fn keep_in_slot<const N: usize>(turn: &mut Turn, fields: [(&str, serde_json::Value); N]) {
    let own = turn.ext.entry("wisp.runtime".to_owned()).or_insert_with(|| json!({}));
    if !own.is_object() {
        *own = json!({});
    }
    if let Some(o) = own.as_object_mut() {
        for (k, v) in fields {
            o.insert(k.to_owned(), v);
        }
    }
}

/// The first answer may name the task: its title is still the prompt's, and it isn't one of Compare's runs (they're
/// one ask, and one run's answer shouldn't name it: v1 runner.js `!session.groupId`).
fn title_auto(s: &Session) -> bool {
    crate::group::group_of(s).is_none()
        && s.ext.get("wisp.runtime").and_then(|v| v.get("titleAuto")).and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// v1 runner.js `#refused`: a turn the model turned down says so, and the card offers another vendor's harness;
/// never for a subagent's run (its lead reads the answer as it is), nor a stopped one.
#[allow(clippy::too_many_arguments, reason = "the turn's end, as the refusal check reads it")]
fn refuse(
    turn: &mut Turn,
    p: &mut wisp_runtime_api::model::Presentation,
    text: &str,
    stop_reason: &str,
    is_error: bool,
    changes: usize,
    name: &str,
    h: Option<&dyn Harness>,
) {
    let ended = wisp_runtime_core::refused::Ended {
        text,
        stop_reason,
        is_error,
        changes,
        agent: &turn.agent,
        model: &turn.model,
        name,
        home: h.and_then(Harness::vendor),
    };
    if let Some((r, blocks)) = wisp_runtime_core::refused::refused(&ended, &p.blocks) {
        turn.refusal = Some(r);
        p.blocks = blocks;
    }
}

/// What a turn read from the cache: Claude's per-model `cacheRead` when it gives them (its `cache_tokens` adds the
/// writes), else the cached input (Codex's `cache_tokens` is reads alone).
fn cache_reads(st: &wisp_runtime_api::model::TurnStats) -> Option<u64> {
    let reads = st.models.as_ref().filter(|m| !m.is_empty()).and_then(|models| {
        models.values().map(|m| m.get("cacheRead").and_then(serde_json::Value::as_u64)).try_fold(0u64, |a, x| x.map(|x| a + x))
    });
    reads.or(st.cache_tokens)
}

/// A failed end in the harness's own signed-out words, with or without a result line: a CLI that died with a 401 on
/// stderr never said it in one, and its raw API error was the task's line in the list (QA desktop r2 #6).
fn said_signed_out(h: Option<&dyn wisp_runtime_api::Harness>, failed: bool, text: &str) -> bool {
    failed && h.is_some_and(|h| h.auth_failed(text))
}

/// What the turn's slot keeps: its changes, what wisp can allow for Allow & retry (a Full auto run's refusals are its
/// harness's own: v1 #777), whether it failed signed out, and `cli`: what the CLI was this turn (version, catalog), for
/// the page's "since the last turn" line.
fn keep_facts(turn: &mut Turn, head: &Head, auth_failed: bool) {
    let blocked = wisp_runtime_core::activity::allowable(turn.mode, &head.blocked);
    if !head.changes.is_empty() || !blocked.is_empty() || auth_failed || head.cli.is_some() {
        keep_in_slot(
            turn,
            [("changes", json!(head.changes)), ("blocked", json!(blocked)), ("authFailed", json!(auth_failed)), ("cli", json!(head.cli))],
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use wisp_kernel::BoxFuture;
    use wisp_runtime_api::TurnId;
    use wisp_runtime_api::model::{QueueItem, Session};
    use wisp_runtime_api::points::RunPart;

    use crate::{Recorder, Runtime, repo::Repo, schema};

    /// Counts the runs whose folder was made ready.
    #[derive(Default)]
    struct Ready(AtomicUsize);
    impl RunPart for Ready {
        fn id(&self) -> &'static str {
            "test.ready"
        }
        fn ready(&self, _s: &Session, _turn: Option<&TurnId>) -> BoxFuture<'static, Result<(), String>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    /// The next queued follow-up claims its task when its run lets go, under the task's lock and before its start is
    /// spawned: a delete right after finds the claim and the start goes no further, not even its folder made ready
    /// (claimed only once the spawned start ran, the delete waited on nothing and `folder_ready` brought the deleted
    /// task's worktree back).
    #[tokio::test(flavor = "current_thread")]
    async fn the_next_queued_follow_up_is_claimed_before_its_start_is_spawned() {
        let store = wisp_store::Store::open(wisp_store::Location::Memory, Arc::new(wisp_store::SystemClock)).unwrap();
        schema::install(&store).unwrap();
        let ready = Arc::new(Ready::default());
        let host = Arc::new(Recorder { parts: vec![ready.clone()], ..Recorder::default() });
        let proc: wisp_proc::Proc = Arc::new(wisp_proc::sim::SimProc::new());
        let rt = Runtime::new(Repo::new(store), host, proc, Arc::new(wisp_store::SystemClock));
        let mut s = Session {
            id: "t1".into(),
            agent: "fake".into(),
            cwd: std::env::temp_dir().display().to_string(),
            queue: vec![QueueItem { id: "q1".into(), text: "next".into(), ..QueueItem::default() }],
            ..Session::default()
        };
        rt.persist(&mut s, Vec::new()).await.unwrap();
        rt.release_run("t1").await;
        // no await since: the spawned start hasn't run yet
        assert_eq!(rt.claims.count("t1"), 1, "claimed before its start was spawned");
        rt.delete("t1").await.unwrap();
        assert_eq!(rt.claims.count("t1"), 0, "the delete waited for it to let go");
        assert_eq!(ready.0.load(Ordering::SeqCst), 0, "the deleted task's folder was never made ready");
        assert!(rt.repo.session("t1").await.is_err(), "nothing put its row back");
    }
}
