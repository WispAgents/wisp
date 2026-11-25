//! Starting a turn (v1 runner.js `#startTurn`): the turn row, `before:run` (which may hold or veto it),
//! the run's inputs (system prompt, env, resume id), the harness's `Launch`, spawned through the proc
//! service. The process is then driven by `drive.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use wisp_proc::{Env, SpawnOpts};
use wisp_runtime_api::events::{TurnHeld, TurnStarted};
use wisp_runtime_api::harness::{HarnessRun, Launch, Stdin};
use wisp_runtime_api::hooks::{BeforeRunCtx, RunKind};
use wisp_runtime_api::model::{Mode, Presentation, QueueItem, Session, SessionState, Turn, TurnStatus};
use wisp_runtime_api::{Harness, TurnId};
use wisp_runtime_core::prompt::{SystemOpts, system_prompt};
use wisp_runtime_core::queue::Run;

use crate::claims::Claim;
use crate::error::{Result, RuntimeError};
use crate::host::Emitted;
use crate::runtime::Runtime;

/// What the drive loop is told from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ctl {
    Stop,
    Interrupt,
    /// the result is in and a follow-up waits: let the lingering CLI go
    Release,
}

/// A task's run while its process lives.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // v1's live/stopping/interrupting sets, one per flag
pub(crate) struct Live {
    /// the harness CLI wisp spawned: the root of the run's process tree (This computer lists it from here, on every OS,
    /// before the CLI says anything: audit I IS12)
    pub pid: u32,
    pub resumable: bool,
    /// the turn has its result; the CLI is only on its way out
    pub finished: bool,
    pub stopping: bool,
    pub interrupting: bool,
    /// the running turn's last step that isn't text (the row's live line)
    pub label: Option<String>,
    /// the account the run is on (what `before:run` picked): its usage reports are that account's
    pub account: Option<String>,
    pub ctl: mpsc::UnboundedSender<Ctl>,
    /// closed once the run's drive let go: its CLI exited, its last output parsed (and the git work that does in
    /// the task's folder, `wisp_harness_kit` pin and snapshots) done. `Runtime::stop_and_wait` waits on it.
    pub ended: tokio::sync::watch::Receiver<()>,
}

impl Live {
    pub fn state(&self) -> Run {
        if self.finished { Run::Lingering } else { Run::Running { resumable: self.resumable } }
    }
}

fn opt_str(v: Option<&Value>, k: &str) -> Option<String> {
    v.and_then(|o| o.get(k)).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// A queued item's own mode and harness, from here on (v1 `#startQueued`). `harness`: a harness's plugin by id.
fn apply_opts(s: &mut Session, item: &QueueItem, harness: &dyn Fn(&str) -> Option<Arc<dyn Harness>>) {
    let opts = item.opts.as_ref();
    if let Some(m) = opts.and_then(|o| o.get("mode")).and_then(|m| serde_json::from_value::<Mode>(m.clone()).ok()) {
        s.mode = m;
    }
    let run = opts.and_then(|o| o.get("run"));
    // a model or thinking the run names, '' its harness's default (QA desktop r3 #1: "Default" kept the task's Opus)
    let named = |k: &str| run.and_then(|r| r.get(k)).and_then(Value::as_str).map(str::to_owned);
    let (model, effort) = (named("model"), named("effort"));
    if let Some(m) = &model {
        s.model.clone_from(m);
    }
    // another harness keeps its own conversation and its own default model unless the run names one (a model is one
    // harness's); the user's thinking level carries over, the new harness taking its nearest (v1 handoff: max → xhigh)
    if let Some(agent) = opt_str(run, "agent") {
        crate::handoff::switch_agent(s, &agent, model.is_some(), effort.is_some(), harness(&agent).as_deref());
    }
    if let Some(e) = effort {
        s.effort = e;
    }
}

/// A follow-up that asked for a new conversation (`FollowUpArgs::fresh`): the task resumes nothing from here on, and
/// the turn's mark (`{fresh: true, after: <the conversation it left>}`) for its `ext["wisp.runtime"]`.
fn fresh_start(s: &mut Session, item: &QueueItem) -> Option<Value> {
    (item.opts.as_ref().and_then(|o| o.get("fresh")) == Some(&Value::Bool(true)))
        .then(|| serde_json::json!({ "fresh": true, "after": s.resume_id.take() }))
}

/// The turn started a new conversation (`fresh_start`'s mark).
pub(crate) fn is_fresh(t: &Turn) -> bool {
    t.ext.get("wisp.runtime").and_then(|r| r.get("fresh")) == Some(&Value::Bool(true))
}

/// The hero title of a turn `before:run` (or its folder) refused: it never started.
pub(crate) const NOT_STARTED: &str = wisp_runtime_core::summary::NOT_STARTED;
/// The turn's `ext["wisp.runtime"]` flag for a CLI that couldn't be spawned: it ended at once, nothing ran.
pub(crate) const SPAWN_FAILED: &str = "spawnFailed";

/// The queue as it is now, over the one `s` was read with before the start's awaits (audit C2): the start took its
/// item out under the task's lock already, and a follow-up queued meanwhile (or a Stop that held them) stays.
fn keep_queue(s: &mut Session, now: &Session) {
    s.queue.clone_from(&now.queue);
    s.queue_held = now.queue_held;
}

fn shown(title: &str, summary: &str, status: &str) -> Presentation {
    let hero = serde_json::json!({ "type": "hero", "title": title, "summary": summary, "status": status });
    Presentation { blocks: vec![hero], v: wisp_runtime_core::blocks::PARSER_VERSION, ..Presentation::default() }
}

impl Runtime {
    /// Starts `item` as the task's next turn. The turn id, also for one held or refused.
    pub async fn start_turn(self: &Arc<Self>, s: Session, item: QueueItem) -> Result<TurnId> {
        // claimed before its first await: a delete from here on finds it, stops it and waits for it (`claims.rs`)
        let claim = self.claims.claim(s.id.as_str());
        self.start_claimed(s, item, claim).await
    }

    /// `start_turn` on a claim its caller took (the next queued follow-up claims under the task's lock, before its
    /// start is spawned: `finish.rs`).
    pub(crate) async fn start_claimed(self: &Arc<Self>, mut s: Session, item: QueueItem, claim: Claim) -> Result<TurnId> {
        // deleted before it ran at all: not even its folder is made ready (a worktree brought back for a deleted task)
        if claim.cancelled() {
            return Err(RuntimeError::NoTask(s.id.0.clone()));
        }
        // a task that asks (or asked) whether the user's harness may lower a guard runs nothing, ever: its held prompt is
        // the question's own text (audit I IS2: Send now or a reply ran it in Full auto)
        crate::more::no_follow_up_on_guard(&s)?;
        let was = s.agent.clone();
        let was_model = s.model.clone();
        let fresh = fresh_start(&mut s, &item);
        apply_opts(&mut s, &item, &|a| self.harness(a).ok());
        // a capped task's turn never runs above its cap, whatever its queued item asked (F-chan-10)
        let capped = crate::capped::cap_of(&s);
        s.mode = wisp_runtime_core::cap::at_cap(s.mode, capped);
        let mut turn = self.next_turn(&s, &item).await?;
        // another harness taking over: the turn says from what to what, and what it carried (ADR-0125); the Timeline
        // draws it as a divider
        crate::handoff::mark_turn(&mut turn, fresh, (&was, &was_model), (&s.agent, &s.model), item.opts.as_ref());
        let ctx = run_ctx(&s, &turn, &item);
        // its folder first (a worktree archiving removed comes back on its branch), before anything reads it
        if let Err(why) = self.folder_ready(&s, Some(&turn.id)).await {
            return self.refused(s, turn, &why, &claim).await;
        }
        if claim.cancelled() {
            return Err(RuntimeError::NoTask(s.id.0.clone()));
        }
        let given = ctx.prompt.clone(); // a hook that changes it (a Make a pane task's guide) is what the harness gets
        let mut ctx = match self.host.before_run(ctx).await {
            Ok(c) => c,
            Err(e) => return self.refused(s, turn, &e.message, &claim).await,
        };
        ctx.once = ctx.once.map(|m| wisp_runtime_core::cap::at_cap(m, capped));
        // the account wisp.accounts picked (an account at its limit or signed out is skipped on a run you start):
        // the turn says which, so its end and its usage reports count on that account, not the task's first pick
        if let Some(a) = ctx.account.clone().filter(|a| !a.is_empty()) {
            turn.account = Some(a);
        }
        // its project's setup failed in its fresh worktree: the turn waits with why (once: Start now runs it)
        if ctx.hold.is_none()
            && let Some(why) = s.ext.get_mut("wisp.runtime").and_then(Value::as_object_mut).and_then(|o| o.remove("setupFailed"))
        {
            ctx.hold = Some(wisp_runtime_api::model::Held {
                by: "wisp.runtime".into(),
                kind: "setup".into(),
                until: None,
                data: serde_json::json!({ "label": "Its setup", "why": why }),
            });
        }
        if let Some(h) = ctx.hold.clone() {
            return self.hold_turn(s, turn, h, &claim).await;
        }
        let harness = match self.harness(&s.agent) {
            Ok(h) => h,
            Err(e) => return self.refused(s, turn, &e.to_string(), &claim).await,
        };
        // one-off extras a command left for this run (approveBlocked's grants, a fork's first run)
        let next = s.ext.get_mut("wisp.runtime").and_then(Value::as_object_mut).and_then(|o| o.remove("next")).unwrap_or(Value::Null);
        // what other plugins keep on the turn before it runs (git: the checkpoint its rollback goes back to)
        self.kept_before(&s, &mut turn).await;
        s.state = SessionState::Running;
        s.unread = false;
        if turn.by.as_deref() != Some("compact") {
            s.archived = false;
            if let Some(o) = s.ext.get_mut("wisp.runtime").and_then(Value::as_object_mut) {
                o.remove("forLater");
            }
        }
        s.updated_at = self.now();
        // from the moment the turn is saved as running until its CLI is in `live`: a run to the queue (B37)
        let _starting = self.mark_starting(s.id.as_str());
        let taken = self.persist_run_start(&mut s, &mut turn, &claim).await?;
        self.host.emit(Emitted::Started(TurnStarted { session_id: s.id.clone(), turn_id: turn.id.clone() }));
        // a task or My Stuff mentioned in the prompt (the turn as saved keeps the user's words)
        let hooked = Box::new(
            // the prompt as `before:run` left it (boxed: in the run's future); the turn keeps the user's
            Turn { prompt: if ctx.prompt == given { turn.prompt.clone() } else { ctx.prompt.clone() }, ..turn.clone() },
        );
        let mut asked = Turn { prompt: self.with_mentions(&s, &hooked, harness.info()).await, ..turn.clone() };
        // what plugins put right before the user's words (a long thread's Notes, ADR-0079)
        self.turn_notes(&s, &turn, harness.info(), &mut asked.prompt).await;
        crate::more::still_working(&s, &turn, &taken, &mut asked.prompt);
        // another harness taking over: what it missed goes first, with the user's rules (v1 handoff.js)
        let asked = Turn { prompt: Box::pin(self.with_handoff(&s, &asked, asked.prompt.clone())).await, ..asked };
        let mut run = self.harness_run(&s, &asked, &ctx, harness.as_ref());
        // the files the turn names by upload id, on disk where the harness can read them (orch #1)
        run.attachments = self.files_for_run(&run.attachments).await;
        // a side discussion runs in the main task's folder, with where it stands before the user's words (orch #5)
        if is_side(&s) {
            self.side_discussion_run(&s, harness.info(), &mut run, &hooked.prompt).await;
        }
        run.grants = next
            .get("grants")
            .and_then(Value::as_array)
            .map(|g| g.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        run.fork = next.get("fork").and_then(Value::as_bool).unwrap_or(false) && harness.info().fork;
        // a resumed conversation's counters are its running total: what its last turn here left comes off
        (run.prior, run.unknown_prior) = self.prior_of(&s, &turn, run.resume_id.as_deref()).await;
        if next.get("once").and_then(Value::as_str) == Some("auto") {
            run.mode = Mode::Auto;
        }
        run.mode = wisp_runtime_core::cap::at_cap(run.mode, capped);
        run.capped = capped.is_some();
        // Plan that runs commands (v1 #794 `planRuns`): yours, not a side discussion's, nor from a channel or trigger
        let outside = run.mode == Mode::Plan && self.started_outside(&s).await;
        run.plan_commands = wisp_runtime_core::plan::runs_commands(run.mode, is_side(&s), outside);
        if turn.idx == 0 && !claim.cancelled() {
            self.record_setup(&s, &run);
        }
        // boxed: every start comes through here, so its callers' futures stay small
        Box::pin(self.launch_turn(&s, turn, &harness, &run, &claim)).await
    }

    /// The turn's CLI spawned, or its turn ended at once when it couldn't be; refused when the task was deleted while
    /// it started (its CLI never spawned, and nothing more is written for it).
    async fn launch_turn(
        self: &Arc<Self>,
        s: &Session,
        turn: Turn,
        h: &Arc<dyn Harness>,
        run: &HarnessRun,
        claim: &Claim,
    ) -> Result<TurnId> {
        match self.spawn_run(s, &turn, h, run, claim).await {
            Ok(true) => {}
            Ok(false) => return Err(RuntimeError::NoTask(s.id.0.clone())),
            Err(e) => {
                let text = e.to_string();
                self.finish_now(s.id.as_str(), turn.id.as_str(), &text).await?;
                // its CLI never ran (gone, its folder unmade): read back as not started (a trial's pair goes as one)
                self.mark_unspawned(s.id.as_str(), turn.id.as_str()).await?;
            }
        }
        Ok(turn.id)
    }

    /// The turn's CLI couldn't be spawned: `ext["wisp.runtime"].spawnFailed`, beside what its end kept there.
    async fn mark_unspawned(self: &Arc<Self>, sid: &str, tid: &str) -> Result<()> {
        let _guard = self.task_lock(sid).await;
        let mut s = self.repo.session(sid).await?;
        let mut t = self.repo.turn(tid).await?;
        let own = t.ext.entry("wisp.runtime".to_owned()).or_insert_with(|| serde_json::json!({}));
        if !own.is_object() {
            *own = serde_json::json!({});
        }
        if let Some(o) = own.as_object_mut() {
            o.insert(SPAWN_FAILED.into(), Value::Bool(true));
        }
        self.persist(&mut s, vec![t]).await.map(drop)
    }

    /// The turn `item` starts, as it's saved (running, from now).
    /// The turn `item` starts: a new one after the task's last, or, for a held turn started (Start now, its reset), that
    /// same turn in its place (`release_held`'s `opts.turn`), so the page updates the held card instead of a new one.
    async fn next_turn(&self, s: &Session, item: &QueueItem) -> Result<Turn> {
        let again = item
            .opts
            .as_ref()
            .and_then(|o| o.get("turn"))
            .and_then(|t| Some((TurnId(t.get("id")?.as_str()?.to_owned()), u32::try_from(t.get("idx")?.as_u64()?).ok()?)));
        if let Some((id, idx)) = again {
            return Ok(Turn { id, ..self.new_turn(s, item, idx) });
        }
        let idx = self.repo.turns(s.id.as_str(), None, Some(1)).await?.last().map_or(0, |t| t.idx + 1);
        let turn = self.new_turn(s, item, idx);
        // a follow-up's turn takes its queue item's id when the client drew it (`runtime.followUp`'s `turn_id`, checked
        // unused there) and no turn has it yet, so the card the client showed at once is this turn (B35)
        if crate::runtime::is_task_id(&item.id) && self.repo.turn(&item.id).await.is_err() {
            return Ok(Turn { id: TurnId(item.id.clone()), ..turn });
        }
        Ok(turn)
    }

    fn new_turn(&self, s: &Session, item: &QueueItem, idx: u32) -> Turn {
        Turn {
            id: TurnId::new(),
            session_id: s.id.clone(),
            idx,
            prompt: item.text.clone(),
            agent: s.agent.clone(),
            model: s.model.clone(),
            effort: s.effort.clone(),
            account: s.account.clone(),
            mode: s.mode,
            attachments: item.attachments.clone(),
            by: item.by.clone(),
            status: TurnStatus::Running,
            started_at: self.now(),
            ..Turn::default()
        }
    }

    /// Parks a turn a `before_run` interceptor held: the task goes to Review with why, and nothing runs.
    async fn hold_turn(
        self: &Arc<Self>,
        mut s: Session,
        mut turn: Turn,
        h: wisp_runtime_api::model::Held,
        claim: &Claim,
    ) -> Result<TurnId> {
        turn.status = TurnStatus::Held;
        turn.held = Some(h.clone());
        s.state = SessionState::Review;
        s.unread = true;
        // a turn asked for it: the task is back in the list, with why it waits (and its folder stays)
        s.archived = false;
        s.updated_at = self.now();
        self.persist_start(&mut s, vec![turn.clone()], claim).await?;
        self.host.emit(Emitted::Task("task.held", crate::host::fact(&s, Some(&turn.id), "held", &format!("{} ({})", h.kind, h.by), None)));
        self.host.emit(Emitted::Held(TurnHeld { session_id: s.id.clone(), turn_id: turn.id.clone(), by: h.by, kind: h.kind }));
        Ok(turn.id)
    }

    /// What `runtime.run@1` contributors keep on a turn just before it runs (at most 3 s each).
    async fn kept_before(&self, s: &Session, turn: &mut Turn) {
        for part in self.host.run_parts() {
            if let Ok(Some((k, v))) = tokio::time::timeout(std::time::Duration::from_secs(3), part.before_turn(s)).await {
                turn.ext.insert(k, v);
            }
        }
    }

    /// Every `runtime.run@1` contributor's `ready` (in order, the first refusal wins), then the folders themselves:
    /// a task whose folder, or one of its extra folders (`dirs`), is gone and that nobody brought back is refused,
    /// never run somewhere else or without it. A turn (`turn`: its id), a /btw and `runtime.quick` (None) all come
    /// through here.
    pub(crate) async fn folder_ready(&self, s: &Session, turn: Option<&TurnId>) -> std::result::Result<(), String> {
        let wait = std::time::Duration::from_millis(wisp_runtime_api::points::READY_MS);
        for part in self.host.run_parts() {
            match tokio::time::timeout(wait, part.ready(s, turn)).await {
                Ok(Ok(())) => {}
                Ok(Err(why)) => return Err(why),
                Err(_) => return Err(format!("This task's folder didn't come back in time ({}).", part.id())),
            }
        }
        folder_there(&s.cwd).await?;
        for dir in &s.dirs {
            extra_there(dir).await?;
        }
        Ok(())
    }

    /// What the task had (ADR-0029), at its first run; its MCP servers probed in the background.
    fn record_setup(&self, s: &Session, run: &HarnessRun) {
        let harness = self.harness(&s.agent).ok();
        let dirs = harness.as_ref().map(|h| h.skill_dirs(&run.cwd)).unwrap_or_default();
        let available = crate::setup::available(s, run, &s.agent, &dirs);
        let servers: Vec<(String, Value)> = run.mcp_servers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let host = self.host.clone();
        crate::setup::record(
            self.repo.store.clone(),
            Arc::new(self.proc.clone()),
            self.probes.clone(),
            self.host.net(),
            harness,
            s.id.0.clone(),
            available,
            servers,
            &move |f| host.spawn(f),
        );
    }

    /// `before:run` vetoed it, its folder can't be made ready, or its harness is gone: the turn says why, and the
    /// task lands (out of Archived, where the refusal would go unseen).
    async fn refused(self: &Arc<Self>, mut s: Session, mut turn: Turn, why: &str, claim: &Claim) -> Result<TurnId> {
        turn.status = TurnStatus::Error;
        turn.is_error = true;
        turn.ended_at = Some(self.now());
        turn.presentation = Some(shown(NOT_STARTED, why, "blocked"));
        s.state = SessionState::Review;
        s.unread = true;
        s.archived = false;
        s.updated_at = self.now();
        self.persist_start(&mut s, vec![turn.clone()], claim).await?;
        Ok(turn.id)
    }

    /// A turn's start written over the task as it is now, under its lock: `s` was read before the start's awaits
    /// (folder, `before:run`, run parts), so another plugin's slot written meanwhile (`runtime.setExt`: a goal
    /// dropped) is kept, never put back as `s` had it. Runtime's own slot is the start's.
    /// A task deleted meanwhile (its claim cancelled, or its row gone) is refused: the write would put its row back.
    /// `persist_start` for a turn that runs: a lead's turn takes over the delegations its last one still owes, in
    /// the same write (`more::report_back`). The records it took.
    async fn persist_run_start(self: &Arc<Self>, s: &mut Session, turn: &mut Turn, claim: &Claim) -> Result<Vec<Value>> {
        let _guard = self.task_lock(s.id.as_str()).await;
        if claim.cancelled() {
            return Err(RuntimeError::NoTask(s.id.0.clone()));
        }
        let now = self.repo.session(s.id.as_str()).await?;
        keep_queue(s, &now);
        carry_others_ext(s, now.ext);
        let (prev, taken) = self.take_over_owed(s, turn).await?;
        self.persist(s, prev.into_iter().chain(std::iter::once(turn.clone())).collect()).await?;
        Ok(taken)
    }

    async fn persist_start(self: &Arc<Self>, s: &mut Session, turns: Vec<Turn>, claim: &Claim) -> Result<()> {
        let _guard = self.task_lock(s.id.as_str()).await;
        if claim.cancelled() {
            return Err(RuntimeError::NoTask(s.id.0.clone()));
        }
        let now = self.repo.session(s.id.as_str()).await?;
        keep_queue(s, &now);
        carry_others_ext(s, now.ext);
        self.persist(s, turns).await.map(drop)
    }

    /// v1 `outsideTask`: the task, or one of its leads (up to `plan::MAX_LEADS`), was started by a trigger or carries
    /// a mode cap (a channel's).
    async fn started_outside(&self, s: &Session) -> bool {
        let mut leads: Vec<Session> = Vec::new();
        let mut up = s.parent_id.clone();
        while let Some(p) = up.take().filter(|_| leads.len() < wisp_runtime_core::plan::MAX_LEADS) {
            let Ok(x) = self.repo.session(p.as_str()).await else { break };
            up.clone_from(&x.parent_id);
            leads.push(x);
        }
        let origins: Vec<wisp_runtime_core::plan::Origin<'_>> = std::iter::once(s)
            .chain(&leads)
            .map(|x| wisp_runtime_core::plan::Origin {
                capped: crate::capped::cap_of(x).is_some(),
                triggered_by: x.ext.get("wisp.runtime").and_then(|o| o.pointer("/spec/lineage/triggeredBy")).and_then(Value::as_str),
                started_by: x.ext.get("wisp.runtime").and_then(|o| o.get("startedBy")).and_then(Value::as_str),
            })
            .collect();
        wisp_runtime_core::plan::outside(&origins)
    }

    /// v1 `teamNote` for the lead's roster (`ext["wisp.runtime"].subagents`), '' without one or a harness that
    /// can't take wisp's tools.
    fn team_note(&self, s: &Session, ask_tool: Option<&str>) -> String {
        let team: Vec<wisp_runtime_core::prompt::subagents::Member> = s
            .ext
            .get("wisp.runtime")
            .and_then(|o| o.get("subagents"))
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect())
            .unwrap_or_default();
        let tool = ask_tool.map(|a| a.rsplit_once("ask_user").map_or_else(|| "delegate".to_owned(), |(pre, _)| format!("{pre}delegate")));
        let name = |id: &str| self.harness(id).ok().map(|h| h.info().name.clone());
        wisp_runtime_core::prompt::subagents::team_note(&team, &name, tool.as_deref())
    }

    fn harness_run(&self, s: &Session, t: &Turn, ctx: &BeforeRunCtx, h: &dyn Harness) -> HarnessRun {
        let info = h.info();
        let parts: Vec<String> = self.host.prompt_parts().iter().map(|p| p.part(s)).filter(|p| !p.is_empty()).collect();
        // v1 runner.js:1379: the app block and wisp_watch unless switched off; blocks (`runtime.blocks`) and Chat mode
        // (`settings.transcript`) are independent: Chat mode drops the end-of-turn report while still allowing inline
        // blocks when `runtime.blocks` is enabled (never for a crew job or a subagent, whose reader expects reports)
        let (apps, watch) = (self.on_unless_off("runtime.apps"), self.on_unless_off("runtime.watch"));
        let user_task = s.parent_id.is_none() && !s.ext.contains_key("wisp.crew");
        let plain = !self.on_unless_off("runtime.blocks") && user_task;
        let chat_mode = self.on_unless_switched_off("settings.transcript") && user_task;
        let ask = s.parent_id.is_some().then(|| wisp_runtime_core::prompt::subagents::SUBAGENT_ASK.to_owned());
        let mut system = system_prompt(&SystemOpts {
            ask_tool: info.ask_tool.clone(),
            ask,
            apps,
            watch,
            blocks: !plain,
            chat_mode,
            ..SystemOpts::default()
        });
        // a lead hears about its team (v1 runner.js:1379 `teamNote`): Delegate's rows, a preset, a crew member's
        // linked delegates (orch #2), or ad-hoc delegation guidance when no team is pre-configured yet; only where
        // wisp's MCP server (and so `delegate`) is offered (never a subagent or read-only side discussion)
        if self.delegates_through_wisp(s, info) {
            system.push_str(&self.team_note(s, info.ask_tool.as_deref()));
        }
        // a side discussion: what it is and what it may do (v1 sideNote)
        system.push_str(&self.side_system(s, info));
        for p in &parts {
            system.push_str("\n\n");
            system.push_str(p);
        }
        let mut prompt = t.prompt.clone();
        // v1 withProtocol: a CLI without a system-prompt flag hears it ahead of the first prompt
        if !info.system_prompt_flag && s.resume_id.is_none() {
            prompt = format!("{}\n\n---\n\n# Task\n\n{prompt}", system.trim());
        }
        let mut env = BTreeMap::from([("WISP_TASK".to_owned(), s.id.0.clone())]);
        for part in self.host.run_parts() {
            env.extend(part.env(s));
        }
        if watch {
            env.insert("WISP_WATCH".into(), "1".into());
        }
        // wisp's own MCP server (ask_user, delegate, wisp_save…), with a token for this task only
        let mut mcp_servers = BTreeMap::new();
        if let Some(acc) = self.host.agent_access() {
            let (url, token) = (acc.url(), acc.token(s.id.as_str()));
            if info.ask_tool.is_some() {
                mcp_servers.insert("wisp".to_owned(), serde_json::json!({ "type": "http", "url": format!("{url}/api/mcp"), "headers": { "x-wisp-token": token, "x-wisp-session": s.id.0 } }));
            }
            env.insert("WISP_URL".into(), url);
            env.insert("WISP_TOKEN".into(), token);
        }
        // a repo that keeps its agent history (ADR-0033): its tools (`wisp mcp`: history, blame, task)
        // (a task with no folder has no repo: `Path::new("")` would find the server's own `.wisp/`, and `$HOME/.wisp`
        // is wisp's own app data directory — never a repo's history folder)
        if let Ok(dir) = wisp_runtime_api::paths::Folder::of(&s.cwd)
            && let Some(repo) = dir.path().ancestors().find(|d| {
                let w = d.join(".wisp");
                w.is_dir()
                    && !w.join("wisp.db").exists()
                    && !w.join("server.json").exists()
                    && !w.join("worktrees").is_dir()
                    && (w.join("wisp.yml").is_file() || w.join("tasks").is_dir())
            })
            && let Ok(exe) = std::env::current_exe()
        {
            mcp_servers.insert("wisp-history".to_owned(), serde_json::json!({ "type": "stdio", "command": exe.display().to_string(), "args": ["mcp", "--repo", repo.display().to_string()] }));
        }
        // the MCP servers other plugins give this run (connectors: a formula's `connectors:`)
        let q = wisp_runtime_api::points::McpQuery {
            mode: ctx.once.unwrap_or(s.mode),
            harness: s.agent.clone(),
            team: s
                .ext
                .get("wisp.runtime")
                .and_then(|o| o.get("subagents"))
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default(),
            turn_id: t.id.0.clone(),
            sub: s.parent_id.is_some(),
        };
        for part in self.host.run_parts() {
            for (k, v) in part.mcp_servers(s, &q) {
                mcp_servers.entry(k).or_insert(v);
            }
        }
        let variant = crate::trial::variant_of(s);
        HarnessRun {
            session_id: s.id.0.clone(),
            turn_id: t.id.0.clone(),
            prompt,
            attachments: t.attachments.clone(),
            cwd: s.cwd.clone(),
            dirs: s.dirs.clone(),
            resume_id: s.resume_id.clone(),
            mode: ctx.once.unwrap_or(s.mode),
            model: s.model.clone(),
            effort: s.effort.clone(),
            system_prompt: system,
            mcp_servers,
            env,
            account_env: ctx.account_env.clone(),
            untrusted: ctx.untrusted,
            // the program `harnesses:` points at (`from: path`)
            bin: s.ext.get("wisp.runtime").and_then(|o| o.pointer("/brief/bin")).and_then(serde_json::Value::as_str).map(str::to_owned),
            probe: true,
            // v1 leanTask: lean when the task asked, or Settings says every run
            quiet: self.lean_task(s),
            // v1 defaults.subagentModel: the model Claude Code's subagents run on ('task' is the task's own); the task's
            // own pick (Try it two ways, `ext["wisp.runtime"].variant`) wins, '' included
            subagent_model: crate::trial::subagents_on(variant.subagent_model.as_deref(), &self.text_setting("runtime.subagentModel")),
            skills: if variant.skills.as_deref() == Some("off") { "off".into() } else { String::new() },
            delegating: self.delegates_through_wisp(s, info),
            ..HarnessRun::default()
        }
    }

    /// A lead that delegates through wisp: told so in its system prompt (`team_note`), and its harness's own subagent
    /// tool off (`HarnessRun::delegating`), one predicate so the two never drift apart. Never a subagent, a side
    /// discussion, a host without wisp's MCP server, or a harness that can't take wisp's tools.
    fn delegates_through_wisp(&self, s: &Session, info: &wisp_runtime_api::harness::HarnessInfo) -> bool {
        s.parent_id.is_none()
            && !is_side(s)
            && self.host.agent_access().is_some()
            && info.ask_tool.as_deref().is_some_and(|t| !t.is_empty())
    }

    /// Where the run's usage counters start (`HarnessRun::prior`, `unknown_prior`): Claude Code and Codex report the
    /// conversation's running total, so a resumed turn is told the totals the conversation had after its last turn
    /// here, or, when none is known (a chat brought in, adopted or restored), that its first result is the baseline.
    async fn prior_of(&self, s: &Session, turn: &Turn, resume: Option<&str>) -> (Option<Value>, bool) {
        if resume.is_none() {
            return (None, false);
        }
        let turns = self.repo.turns(s.id.as_str(), Some(turn.idx), Some(PRIOR_LOOKBACK)).await.unwrap_or_default();
        prior_from(&turns, &s.agent, resume)
    }

    /// `Ok(false)`: its claim was cancelled (the task deleted) before the spawn: nothing spawned.
    async fn spawn_run(self: &Arc<Self>, s: &Session, t: &Turn, h: &Arc<dyn Harness>, run: &HarnessRun, claim: &Claim) -> Result<bool> {
        let launch = h.launch(run)?;
        let tmp = write_files(&format!("wisp-run-{}", t.id), &launch).await?;
        // its launch files (an MCP config with the run's token and a connector's secrets, ADR-0116) go on every way out:
        // the run ended, its drive was dropped (wisp.runtime switched off), or it never spawned
        let files = LaunchDir::of(&launch, &tmp);
        let fill = |x: &str| x.replace("{tmp}", &tmp);
        let mut set: Vec<(String, String)> = run.env.iter().chain(&run.account_env).map(|(k, v)| (k.clone(), v.clone())).collect();
        set.extend(launch.env.iter().map(|(k, v)| (k.clone(), fill(v))));
        let cwd = run_folder(&launch.cwd, &s.cwd, &s.id.0).await?;
        let opts = SpawnOpts {
            cwd: Some(cwd),
            env: Env { clear: false, set, remove: launch.env_remove.clone() },
            no_stdin: launch.stdin == Stdin::Null,
            // the CLI's tree goes with the plugin's scope (switching wisp.runtime off), as its drive does
            scope: self.host.scope(),
            ..SpawnOpts::default()
        };
        let args: Vec<String> = launch.args.iter().map(|a| fill(a)).collect();
        // the spawn and its `Live` entry as one step against a delete's cancel: it lands before (nothing spawns) or
        // after (it finds `Live` and stops the run there)
        let spawned = claim.unless_cancelled(|| -> Result<_> {
            let child = self.proc.spawn(&launch.program, &args, opts)?;
            let pid = child.pid();
            let (handle, rx) = child.split();
            let (ctl, ctl_rx) = mpsc::unbounded_channel();
            let (ended_tx, ended) = tokio::sync::watch::channel(());
            let mut all = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            // a Stop that came while it started: it stops now
            let stopping = self.stops.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(s.id.as_str());
            let resumable = s.resume_id.is_some();
            let interrupting = !stopping
                && resumable
                && self.pending_interrupt.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(s.id.as_str());
            if stopping {
                self.pending_interrupt.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(s.id.as_str());
                let _ = ctl.send(Ctl::Stop);
            } else if interrupting {
                let _ = ctl.send(Ctl::Interrupt);
            }
            let live =
                Live { pid, resumable, finished: false, stopping, interrupting, label: None, account: t.account.clone(), ctl, ended };
            all.insert(s.id.0.clone(), live);
            drop(all);
            // `live` has it now: no longer starting (cleared here, not when the start returns, so a run that ends at
            // once never reads as starting after its end: B37)
            self.starting.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(s.id.as_str());
            Ok((handle, rx, ctl_rx, ended_tx))
        });
        let Some(spawned) = spawned else {
            return Ok(false);
        };
        let (handle, rx, ctl_rx, ended_tx) = spawned?;
        if let Stdin::Bytes(b) = &launch.stdin {
            // a CLI that exited before reading its prompt is driven all the same: `Live` is in, and only its drive
            // takes it out (a turn whose write failed here read as running forever, its follow-ups queued unstarted);
            // its exit ends the turn with what it said, and the next queued follow-up starts
            if let Err(err) = handle.write(b.clone()) {
                tracing::warn!(task = %s.id, %err, "runtime: the prompt didn't reach the CLI");
            }
            handle.close_stdin();
        }
        let rt = self.clone();
        let driver = h.driver(run);
        let (sid, tid) = (s.id.0.clone(), t.id.0.clone());
        // on the plugin's scope: switching wisp.runtime off ends the drive (and its CLI, through the spawn's scope)
        self.host.spawn(Box::pin(async move {
            rt.drive(sid, tid, driver, handle, rx, ctl_rx).await;
            drop(files);
            drop(ended_tx);
        }));
        Ok(true)
    }

    /// Each run going now (finishing ones too: their CLI is still alive) as `(task, pid of the CLI wisp spawned)`.
    pub fn live_pids(&self) -> Vec<(String, u32)> {
        let live = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        live.iter().map(|(id, l)| (id.clone(), l.pid)).collect()
    }

    pub(crate) fn send(&self, id: &str, c: Ctl) -> bool {
        let mut live = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(l) = live.get_mut(id) else {
            return false;
        };
        match c {
            Ctl::Stop => l.stopping = true,
            Ctl::Interrupt => l.interrupting = true,
            Ctl::Release => {}
        }
        l.ctl.send(c).is_ok()
    }

    pub(crate) fn stop_run(&self, id: &str) {
        self.pending_interrupt.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(id);
        let mut live = self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(l) = live.get_mut(id) {
            l.stopping = true;
            let _ = l.ctl.send(Ctl::Stop);
            return;
        }
        // its turn is saved as running and its CLI on its way: the spawn stops it (decided under `live`'s lock, as the
        // spawn's entry goes in, so a Stop lands either here or on `Live`)
        let starting = self.starting.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(id).is_some_and(|n| *n > 0);
        if starting {
            self.stops.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned());
        }
        drop(live);
    }
    /// The task's run, stopped, and once it let go (at most `STOP_WAIT`): nothing of it is still at work in the
    /// task's folder after this. A delete waits on it: a deleted delegate's copy is removed right after (ADR-0060),
    /// and a drive still parsing the CLI's last output ran git there meanwhile (its pin's temporary index), so the
    /// copy's removal failed with "Directory not empty" and the copy stayed.
    /// A run still starting (claimed, not spawned yet: its folder made ready, `before:run`) or a /btw is cancelled
    /// first, then `Live` is read: a start that spawned before the cancel has its entry in by then (`claims.rs`).
    pub(crate) async fn stop_and_wait(&self, id: &str) {
        let mut waits = self.claims.cancel(id);
        waits.extend(self.live.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(id).map(|l| l.ended.clone()));
        self.stop_run(id);
        let all = async {
            for w in &mut waits {
                let _ = w.changed().await;
            }
        };
        if tokio::time::timeout(STOP_WAIT, all).await.is_err() {
            tracing::warn!(task = id, "runtime: a stopped run didn't let go in {} s", STOP_WAIT.as_secs());
        }
    }

    pub(crate) fn interrupt(&self, id: &str) {
        self.send(id, Ctl::Interrupt);
    }
    pub(crate) fn release_linger(&self, id: &str) {
        self.send(id, Ctl::Release);
    }
}

/// Where a harness runs, never the server's own folder: the launch's folder, else the task's; with neither
/// (adopted from a share, made before `specs::folder`, a `runtime.quick` with no folder) one empty folder of
/// its own under the temp dir, the same each turn for `scratch` so a resume finds it. A relative folder is
/// refused: it would resolve against the server's folder.
/// Every ext slot but runtime's own as `now` has it (other plugins write theirs under the task's lock).
/// Runtime's own fields the list sets at any time (`runtime.pin`) come from `now` too: a pin made while the turn
/// started was overwritten by the copy read before the start's awaits (TRY-17 #30).
fn carry_others_ext(s: &mut Session, mut now: wisp_runtime_api::model::Ext) {
    let pinned = now.get(OWN).and_then(|o| o.get(PINNED)).cloned();
    let mut own = s.ext.remove(OWN);
    if let Some(o) = own.as_mut().and_then(Value::as_object_mut) {
        match pinned {
            Some(p) => o.insert(PINNED.to_owned(), p),
            None => o.remove(PINNED),
        };
    } else if let Some(p) = pinned {
        own = Some(serde_json::json!({ PINNED: p }));
    }
    match own {
        Some(own) => now.insert(OWN.to_owned(), own),
        None => now.remove(OWN),
    };
    s.ext = now;
}

const PINNED: &str = "pinned";

const OWN: &str = "wisp.runtime";
/// How long `stop_and_wait` waits for a stopped run (its CLI's kill grace and tree sweep are 2 s each).
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

/// How many turns back `prior_of` looks for the conversation's last counters (turns that left none, a stop before its
/// result, are passed over).
const PRIOR_LOOKBACK: u32 = 20;

/// `(prior, unknown_prior)` for a run on `agent` resuming `resume`, from the task's turns before it (oldest first): the
/// totals of the newest one that left any, when it ran on the same harness in the same conversation. Anything else
/// (another harness's, another conversation's, none at all: a chat brought in) leaves the counters' start unknown.
/// Nothing to resume: a fresh conversation, counted from zero.
pub(crate) fn prior_from(turns: &[Turn], agent: &str, resume: Option<&str>) -> (Option<Value>, bool) {
    let Some(resume) = resume else { return (None, false) };
    let last = turns.iter().rev().filter(|t| t.ended_at.is_some()).find_map(|t| Some((t, t.stats.as_ref()?.totals.as_ref()?)));
    match last {
        Some((t, totals)) if t.agent == agent && t.resume_id.as_deref() == Some(resume) => (Some(totals.clone()), false),
        _ => (None, true),
    }
}

/// A side discussion (`ext["wisp.runtime"].sideOf`, set by `runtime.create` for a task the user starts under another).
pub(crate) fn is_side(s: &Session) -> bool {
    side_main(s).is_some()
}

/// The main task a side discussion is about.
pub(crate) fn side_main(s: &Session) -> Option<&str> {
    s.ext.get("wisp.runtime").and_then(|o| o.pointer("/sideOf/id")).and_then(Value::as_str).filter(|id| !id.is_empty())
}

/// A run's folder of launch files, removed when this goes (ADR-0116: they may hold the run's token and a connector's
/// secrets). Held by the run's drive, so it goes when the run ends, when the drive is dropped (wisp.runtime switched
/// off), or at once when the run never spawned.
pub(crate) struct LaunchDir(Option<std::path::PathBuf>);

fn uses_tmp(launch: &Launch) -> bool {
    !launch.files.is_empty() || launch.args.iter().any(|a| a.contains("{tmp}")) || launch.env.values().any(|v| v.contains("{tmp}"))
}

impl LaunchDir {
    pub(crate) fn of(launch: &Launch, dir: &str) -> Self {
        Self(uses_tmp(launch).then(|| std::path::PathBuf::from(dir)))
    }
}

impl Drop for LaunchDir {
    fn drop(&mut self) {
        // a few small files: removed where the run lets go, whatever runtime is (or isn't) still up
        if let Some(d) = self.0.take() {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

/// Launch folders a crash left (`wisp-run-*`, `wisp-side-*` under the temp folder) older than a week: removed at
/// start (ADR-0116). A newer one may be another wisp's running run on this computer, so it stays.
pub(crate) async fn sweep_launch_dirs() {
    const WEEK: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);
    let Ok(mut rd) = tokio::fs::read_dir(std::env::temp_dir()).await else { return };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("wisp-run-") || name.starts_with("wisp-side-")) {
            continue;
        }
        let Ok(m) = tokio::fs::symlink_metadata(e.path()).await else { continue };
        let old = m.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > WEEK);
        if m.is_dir() && old {
            let _ = tokio::fs::remove_dir_all(e.path()).await;
        }
    }
}

/// Files the launch asked for (an MCP config, a prompt file), in a temp folder of the run's own (`name`, under the
/// OS temp folder). Returns the folder, which the launch's `{tmp}` stands for.
pub(crate) async fn write_files(name: &str, launch: &Launch) -> Result<String> {
    let dir = std::env::temp_dir().join(name);
    if !uses_tmp(launch) {
        return Ok(dir.to_string_lossy().into_owned());
    }
    tokio::fs::create_dir_all(&dir).await.map_err(|e| RuntimeError::Invalid(e.to_string()))?;
    // the files may hold secrets (an MCP config with the run's wisp token and a connector's env, ADR-0116): the folder is
    // this user's only (0700), each file 0600; a folder someone else made there is refused, not written into
    if let Err(e) = private_dir(&dir).await {
        return Err(RuntimeError::Invalid(format!("the run's own folder {} isn't private: {e}", dir.display())));
    }
    for f in &launch.files {
        let wrote = if f.path.contains("..") {
            Err(RuntimeError::Invalid(format!("launch file {} leaves its folder", f.path)))
        } else {
            write_private(&dir.join(&f.path), f.contents.as_bytes()).await.map_err(|e| RuntimeError::Invalid(e.to_string()))
        };
        // nothing it wrote (a prompt, instructions) is left behind when it can't spawn
        if let Err(e) = wrote {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(e);
        }
    }
    Ok(dir.to_string_lossy().into_owned())
}

/// `dir`, made this user's only (0700). A link at its name is refused, and so is a folder another user made there:
/// only a file's owner may change its mode, so the chmod is the ownership check. Not on Unix: the temp folder is the
/// user's own already.
async fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    let m = tokio::fs::symlink_metadata(dir).await?;
    if !m.is_dir() {
        return Err(std::io::Error::other("not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}

/// `bytes` to `path`, readable by this user only (0600 on Unix), made fresh: an earlier file of the name is replaced.
async fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt as _;
    let _ = tokio::fs::remove_file(path).await;
    let mut o = tokio::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    o.mode(0o600);
    let mut f = o.open(path).await?;
    f.write_all(bytes).await?;
    f.flush().await
}

pub(crate) async fn run_folder(launch: &str, task: &str, scratch: &str) -> Result<std::path::PathBuf> {
    let named = if launch.trim().is_empty() { task } else { launch };
    if !named.trim().is_empty() {
        return wisp_runtime_api::paths::Folder::of(named)
            .map(wisp_runtime_api::paths::Folder::into_path)
            .map_err(|e| RuntimeError::Invalid(format!("Not a project folder: {e}")));
    }
    let dir = std::env::temp_dir().join(format!("wisp-scratch-{scratch}"));
    tokio::fs::create_dir_all(&dir).await.map_err(|e| RuntimeError::Invalid(format!("couldn't make a folder for the task: {e}")))?;
    Ok(dir)
}

/// A task with a folder runs only there: Err (the reason, as the turn says it) when it's gone or not a folder.
/// No folder at all, or a relative one, is left to `run_folder` (its own scratch folder, or refused).
pub(crate) async fn folder_there(cwd: &str) -> std::result::Result<(), String> {
    if wisp_runtime_api::paths::Folder::of(cwd).is_err() {
        return Ok(());
    }
    match tokio::fs::metadata(cwd).await {
        Ok(m) if m.is_dir() => Ok(()),
        _ => Err(format!("This task's folder is gone: {cwd}. It runs only there, so nothing was started.")),
    }
}

/// One of a task's extra folders (`dirs`): Err, with its name, when it's gone or not a folder (it's never dropped
/// from the run silently). A relative one is left as it is: the harness is given it as named.
pub(crate) async fn extra_there(dir: &str) -> std::result::Result<(), String> {
    if wisp_runtime_api::paths::Folder::of(dir).is_err() {
        return Ok(());
    }
    match tokio::fs::metadata(dir).await {
        Ok(m) if m.is_dir() => Ok(()),
        _ => Err(format!("One of this task's extra folders is gone: {dir}. It runs only with all of them, so nothing was started.")),
    }
}

/// What `before:run` hooks are handed for this turn: the task, the queued item and what it asked.
fn run_ctx(s: &Session, turn: &Turn, item: &QueueItem) -> BeforeRunCtx {
    BeforeRunCtx {
        session_id: s.id.clone(),
        turn_id: turn.id.clone(),
        agent: s.agent.clone(),
        account: s.account.clone(),
        model: s.model.clone(),
        effort: s.effort.clone(),
        mode: s.mode,
        cwd: s.cwd.clone(),
        prompt: item.text.clone(),
        by: item.by.clone(),
        hold: None,
        account_env: BTreeMap::new(),
        once: None,
        // a project declared without trusting its hooks or MCP servers (crate::kinds), or Run without them on a hold
        untrusted: s
            .ext
            .get("wisp.runtime")
            .and_then(|o| o.pointer("/brief/untrusted"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || item.opts.as_ref().and_then(|o| o.get("untrusted")).and_then(serde_json::Value::as_bool).unwrap_or(false),
        kind: if turn.idx == 0 { RunKind::First } else { RunKind::FollowUp },
        parent_id: s.parent_id.clone(),
        confirm_limit: item.opts.as_ref().and_then(|o| o.get("confirmLimit")).and_then(serde_json::Value::as_bool).unwrap_or(false),
        together: crate::trial::together(s, turn.idx),
        // the hold Start now released, boxed (the context rides in every run's future)
        released: item.opts.as_ref().and_then(|o| o.get("released")).and_then(|v| serde_json::from_value(v.clone()).ok()).map(Box::new),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod folder_tests {
    /// A harness (a turn, a /btw, `runtime.quick`) never spawns with no cwd, which is the server's own folder.
    #[tokio::test]
    async fn a_harness_runs_in_the_launchs_folder_else_the_tasks_else_its_own_scratch_never_the_servers() {
        let at = |p: &str| std::path::PathBuf::from(p);
        assert_eq!(super::run_folder("/launch", "/task", "t1").await.unwrap(), at("/launch"));
        assert_eq!(super::run_folder("", "/task", "t1").await.unwrap(), at("/task"));
        let scratch = super::run_folder(" ", "", "folder-test").await.unwrap();
        assert_eq!(scratch, std::env::temp_dir().join("wisp-scratch-folder-test"));
        assert!(scratch.is_dir(), "made, so the spawn finds it");
        for (launch, task) in [("relative", "/task"), ("", "src"), ("", ".")] {
            let e = super::run_folder(launch, task, "t1").await.unwrap_err();
            assert!(e.to_string().contains("Not a project folder"), "{launch:?} {task:?}: {e}");
        }
        std::fs::remove_dir_all(scratch).ok();
    }

    /// Audit I IS3 / ADR-0116: a launch file may hold the run's wisp token and a connector's secrets (the MCP config):
    /// it's written 0600 in a folder only this user may enter (0700), and a link at the folder's name is refused.
    #[cfg(unix)]
    #[tokio::test]
    async fn launch_files_are_this_users_only_audit_i_is3() {
        use std::os::unix::fs::PermissionsExt as _;
        let launch = wisp_runtime_api::harness::Launch {
            files: vec![wisp_runtime_api::harness::LaunchFile { path: "wisp-mcp.json".into(), contents: "{\"secret\":1}".into() }],
            ..wisp_runtime_api::harness::Launch::default()
        };
        let name = format!("wisp-run-perm-test-{}", std::process::id());
        let dir = std::path::PathBuf::from(super::write_files(&name, &launch).await.unwrap());
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(dir.join("wisp-mcp.json")).unwrap().permissions().mode() & 0o777, 0o600);
        // written again (a retry of the same turn): still 0600, its new contents
        super::write_files(&name, &launch).await.unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("wisp-mcp.json")).unwrap(), "{\"secret\":1}");
        std::fs::remove_dir_all(&dir).ok();
        // a link at the run's name (someone else's folder, or anywhere) is never written through
        let elsewhere = std::env::temp_dir().join(format!("{name}-elsewhere"));
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &dir).unwrap();
        assert!(super::write_files(&name, &launch).await.is_err());
        assert!(!elsewhere.join("wisp-mcp.json").exists());
        std::fs::remove_file(&dir).ok();
        std::fs::remove_dir_all(&elsewhere).ok();
    }

    /// The review of audit I IS3: a run that never spawned (its folder gone, a spawn error) or whose drive was dropped
    /// (wisp.runtime switched off) left its launch folder, with the run's token, behind. It goes with its guard.
    #[tokio::test]
    async fn a_runs_launch_folder_goes_however_the_run_ends_audit_i_is3() {
        let launch = wisp_runtime_api::harness::Launch {
            files: vec![wisp_runtime_api::harness::LaunchFile { path: "wisp-mcp.json".into(), contents: "{}".into() }],
            ..wisp_runtime_api::harness::Launch::default()
        };
        let dir = super::write_files(&format!("wisp-run-drop-test-{}", std::process::id()), &launch).await.unwrap();
        let guard = super::LaunchDir::of(&launch, &dir);
        assert!(std::path::Path::new(&dir).join("wisp-mcp.json").exists());
        let dropped = tokio::spawn(async move {
            let _held = guard;
            std::future::pending::<()>().await;
        });
        dropped.abort();
        let _ = dropped.await;
        assert!(!std::path::Path::new(&dir).exists(), "the folder went with the dropped drive");
        // a launch with no files has no folder to remove
        assert!(super::LaunchDir::of(&wisp_runtime_api::harness::Launch::default(), "/nonexistent").0.is_none());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;
    use wisp_runtime_api::model::Session;

    /// TRY-17 #30 (review): a pin (`runtime.pin`) made while a turn starts was overwritten by the start's write of
    /// runtime's own slot, read before the start's awaits: the pill showed, then went. The start keeps the pin as the
    /// task has it now, and keeps its own fields (a cap) as it read them.
    #[test]
    fn a_pin_made_while_a_turn_starts_survives_the_starts_write() {
        let mut s = Session::default();
        s.ext.insert("wisp.runtime".into(), json!({ "cap": "plan" }));
        let mut now = s.ext.clone();
        now.insert("wisp.runtime".into(), json!({ "cap": "plan", "pinned": true }));
        now.insert("wisp.goal".into(), json!({ "text": "x" }));
        super::carry_others_ext(&mut s, now.clone());
        assert_eq!(s.ext["wisp.runtime"], json!({ "cap": "plan", "pinned": true }));
        assert_eq!(s.ext["wisp.goal"], json!({ "text": "x" }));
        // unpinned meanwhile: it stays unpinned
        let mut s = Session::default();
        s.ext.insert("wisp.runtime".into(), json!({ "pinned": true }));
        let mut now = s.ext.clone();
        now.insert("wisp.runtime".into(), json!({}));
        super::carry_others_ext(&mut s, now);
        assert_eq!(s.ext["wisp.runtime"], json!({}));
    }

    fn item(opts: &serde_json::Value) -> wisp_runtime_api::model::QueueItem {
        serde_json::from_value(json!({ "id": "q1", "text": "go on", "at": 1, "opts": opts })).unwrap_or_else(|e| panic!("item: {e}"))
    }

    fn on_opus() -> Session {
        Session { agent: "claude".into(), model: "opus".into(), effort: "high".into(), ..Session::default() }
    }

    /// QA desktop r3 #1: a follow-up runs on the task's own model unless the reply box picks another, and a pick is what
    /// it says: "Default" is the harness's default (an empty model was dropped, so it kept Opus), and another harness
    /// never takes the old one's model.
    #[test]
    fn a_follow_ups_pick_is_its_run_default_included_and_none_keeps_the_tasks_own() {
        // nothing picked: the task's own
        let mut s = on_opus();
        super::apply_opts(&mut s, &item(&json!({ "mode": "auto" })), &|_| None);
        assert_eq!((s.agent.as_str(), s.model.as_str(), s.effort.as_str()), ("claude", "opus", "high"));
        // Default picked on the same harness: the harness's default model and thinking
        let mut s = on_opus();
        super::apply_opts(&mut s, &item(&json!({ "run": { "agent": "claude", "model": "", "effort": "" } })), &|_| None);
        assert_eq!((s.model.as_str(), s.effort.as_str()), ("", ""));
        // a model picked: that one, the thinking as it was unless named
        let mut s = on_opus();
        super::apply_opts(&mut s, &item(&json!({ "run": { "agent": "claude", "model": "sonnet" } })), &|_| None);
        assert_eq!((s.model.as_str(), s.effort.as_str()), ("sonnet", "high"));
        // another harness named alone: its own default model, not Opus; the thinking level carries (v1 handoff)
        let mut s = on_opus();
        super::apply_opts(&mut s, &item(&json!({ "run": { "agent": "codex" } })), &|_| None);
        assert_eq!((s.agent.as_str(), s.model.as_str(), s.effort.as_str()), ("codex", "", "high"));
        assert!(s.resume_id.is_none());
    }
}
