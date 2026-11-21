//! wisp.threads host (ADR-0079, long threads): what keeps a task's agent on track after hundreds of turns.
//!
//! - **Notes** (`notes.rs`, tables in `store.rs`): the task's state (goal, constraints, decisions, what failed, where
//!   it stands, open questions, key files), each section rewritten, never appended to, every version kept with who
//!   wrote it. `threads.notes`, `threads.notesUpdate`, `threads.revisions`; the agent's `wisp_notes` tool
//!   (`parts.rs`); `wisp notes` in the CLI. A subagent reads its lead's Notes and can't write them. A task tracked in
//!   its repo (ADR-0033) also has them in `.wisp/tasks/<id>/notes.md` (`files.rs`).
//! - **The note** before each follow-up (`parts.rs`, `runtime.prompt@1` `before`): the Notes as they are now, what
//!   you changed since the agent last read them, and a nudge when your message changes something said earlier.
//! - **Health** (`health.rs`): how worn the thread is, worked out at each turn's end from what runtime already has,
//!   on the task's `session:<id>` topic and `threads.health`.
//! - **The offload folder** (`parts.rs`, `runtime.run@1`): each run is told where long command output goes
//!   (`WISP_OFFLOAD_DIR`); Claude Code's `PreToolUse` hook (`wisp hook offload`) acts on it. Removed with the task.
//!
//! resources: data-only (a task's Notes, their versions and its health are the task's own state, written by its agent
//! and you as the work goes; nothing in them is setup to declare in wisp.yml).

mod audit;
mod files;
mod fresh;
mod health;
mod notes;
mod parts;
mod seed;
mod store;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use wisp_kernel::{Call, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::events::{SessionCreated, SessionDeleted, TurnEnded};
use wisp_store::Store;
use wisp_threads_api::{self as api, By};

pub const PLUGIN_ID: &str = api::PLUGIN;

/// The kinds wisp.threads publishes on a task's own topic (`session:<id>`), and the column naming the task in each:
/// what a first look at the task reads beside runtime's task window (`crates/wisp/src/serve.rs`), so the Thread tab
/// has its Notes, versions, health and audits at once, and a later patch of one of them has its row to land on.
pub mod kinds {
    use crate::store;
    pub const NOTES: &str = store::NOTES.kind;
    pub const REVISIONS: &str = store::REVISIONS.kind;
    pub const META: &str = store::META.kind;
    pub const HEALTH: &str = store::HEALTH.kind;
    pub const AUDITS: &str = store::AUDITS.kind;
}

pub(crate) struct State {
    pub store: Store,
    /// `<wisp home>/threads/out`: each task's long command output, under its id
    pub out: PathBuf,
    /// the tasks whose compaction is being audited now
    pub busy: audit::Busy,
    /// what health reads of the last turns of the tasks whose turns ended lately (`health::Window`)
    pub windows: health::Windows,
}

pub(crate) type Slot = Arc<OnceLock<Arc<State>>>;

pub(crate) fn state(slot: &Slot) -> Result<Arc<State>> {
    slot.get().cloned().ok_or_else(|| WireError::unavailable("wisp.threads is starting"))
}

pub(crate) fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

pub(crate) fn db(e: &wisp_store::StoreError) -> WireError {
    WireError::internal(format!("threads: {e}"))
}

/// The task a task's Notes belong to: its own, or its lead's (a subagent works for its lead).
pub(crate) async fn owner(ctx: &Ctx, call: &Call, task: &str) -> Result<(String, Option<String>)> {
    let s = ctx.run::<wisp_runtime_api::commands::Get>(call, wisp_runtime_api::commands::IdArgs { id: task.into() }).await?;
    Ok(match s.parent_id {
        Some(lead) => (lead.0.clone(), Some(lead.0)),
        None => (task.to_owned(), None),
    })
}

async fn read(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<api::Notes> {
    let (whose, lead) = owner(ctx, call, task).await?;
    let (mut n, _) = notes::load(&st.store, &whose).await.map_err(|e| db(&e))?;
    n.lead = lead;
    Ok(n)
}

/// The task's turns now (its last turn's index + 1), read when the Notes are written: no per-turn count is kept
/// (it cost a write at every turn's end, ADR-0100).
async fn turns_now(ctx: &Ctx, call: &Call, task: &str) -> u32 {
    let last = ctx
        .run::<wisp_runtime_api::commands::Turns>(
            call,
            wisp_runtime_api::commands::TurnsArgs { id: task.into(), before: None, limit: Some(1), masked: None },
        )
        .await;
    last.ok().and_then(|t| t.iter().map(|t| t.idx + 1).max()).unwrap_or(0)
}

async fn update(ctx: &Ctx, call: &Call, st: &State, a: api::UpdateArgs) -> Result<api::Notes> {
    let (whose, lead) = owner(ctx, call, &a.id).await?;
    if lead.is_some() {
        return Err(WireError::invalid("A subagent reads its lead’s Notes and can’t change them: say in your report what should change."));
    }
    let by = a.by.unwrap_or(By::You);
    let wrote = notes::update(&st.store, &whose, &a.section, &a.text, by, now(ctx), turns_now(ctx, call, &whose).await)
        .await
        .map_err(|e| db(&e))?
        .map_err(WireError::invalid)?;
    if wrote.changed {
        files::write_repo_file(ctx, call, &whose, &wrote.notes).await;
    }
    Ok(wrote.notes)
}

#[derive(Debug, Default)]
pub struct ThreadsPlugin;

/// What wisp.threads does when runtime says something happened: a turn ended (the compactions a hook counted, health,
/// the audit), a task was made (its spec's Notes), a task went (all it had here).
fn events(s: &mut Setup, slot: &Slot) {
    let sl = slot.clone();
    s.on::<TurnEnded, _, _>(move |ctx, e| {
        let sl = sl.clone();
        async move {
            if let Ok(st) = state(&sl) {
                let call = ctx.root_call();
                // first what a harness's compaction hook counted (Codex), so health and the audit see it
                let marked = audit::take_marked(&ctx, &call, &st, e.session_id.as_str(), e.turn_id.as_str()).await;
                if let Err(err) = health::work_out(&ctx, &call, &st, e.session_id.as_str(), Some(e.turn_id.as_str())).await {
                    tracing::debug!(error = %err.message, task = %e.session_id.0, "threads: no health");
                }
                audit::after_turn(&ctx, &call, &st, e.session_id.as_str(), e.turn_id.as_str(), marked).await;
            }
        }
    });
    let sl = slot.clone();
    s.on::<SessionCreated, _, _>(move |ctx, e| {
        let sl = sl.clone();
        async move {
            if let Ok(st) = state(&sl) {
                let task = e.session.id.0.clone();
                if let Err(err) = seed::seed(&ctx, &ctx.root_call(), &st, &task).await {
                    tracing::warn!(error = %err.message, task = %task, "threads: the task's notes weren't seeded");
                }
            }
        }
    });
    let sl = slot.clone();
    s.on::<SessionDeleted, _, _>(move |_ctx, e| {
        let sl = sl.clone();
        async move {
            if let Ok(st) = state(&sl) {
                st.windows.forget(e.id.as_str());
                files::forget(&st, e.id.as_str()).await;
            }
        }
    });
}

impl Plugin for ThreadsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let slot: Slot = Arc::new(OnceLock::new());
        let sl = slot.clone();
        s.command::<api::NotesCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                read(&ctx, &call, &st, &a.id).await
            }
        });
        let sl = slot.clone();
        s.command::<api::NotesUpdate, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                update(&ctx, &call, &st, a).await
            }
        });
        let sl = slot.clone();
        s.command::<api::RevisionsCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                let (whose, _) = owner(&ctx, &call, &a.id).await?;
                notes::revisions(&st.store, &whose, a.section.as_deref()).await.map_err(|e| db(&e))
            }
        });
        let sl = slot.clone();
        s.command::<api::HealthCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                health::read(&ctx, &call, &st, &a.id).await
            }
        });
        let sl = slot.clone();
        s.command::<api::FreshBriefCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                fresh::make_brief(&ctx, &call, &st, &a.id).await
            }
        });
        let sl = slot.clone();
        s.command::<api::FreshCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                fresh::start(&ctx, &call, &st, a).await
            }
        });
        let sl = slot.clone();
        s.command::<api::AuditsCmd, _, _>(move |ctx, call, a| {
            let sl = sl.clone();
            async move {
                let st = state(&sl)?;
                let (whose, _) = owner(&ctx, &call, &a.id).await?;
                audit::list(&st, &whose, now(&ctx)).await
            }
        });
        parts::register(s, &slot);
        events(s, &slot);
        s.on_start(move |ctx| async move {
            let store = ctx.service::<Store>()?.as_ref().clone();
            store::install(&store).map_err(|e| db(&e))?;
            let home = ctx
                .service::<wisp_kernel::services::Paths>()
                .map_or_else(|_| std::env::temp_dir().join("wisp"), |p| p.home().to_path_buf());
            let _ = slot.set(Arc::new(State {
                store,
                out: home.join("threads").join("out"),
                busy: audit::Busy::default(),
                windows: health::Windows::default(),
            }));
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(ThreadsPlugin.manifest().id, PLUGIN_ID);
    }
}
