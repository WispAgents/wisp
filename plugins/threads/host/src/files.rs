//! Files: a tracked task's `.wisp/tasks/<id>/notes.md` in its repo (ADR-0033, through `history.store@1`, so the
//! Notes travel with the code and `wisp blame`), and the clean-up when a task is deleted (its rows, and its folder of
//! long command output).

use wisp_history_api::{EXT, STORE};
use wisp_kernel::{Call, Ctx};
use wisp_threads_api::Notes;

use crate::{State, notes, parts};

/// Rewrites the repo's `notes.md` for a task tracked in its repo; nothing for one that isn't. Never fails the write
/// it follows: the store holds the Notes either way.
pub async fn write_repo_file(ctx: &Ctx, call: &Call, task: &str, n: &Notes) {
    let Ok(s) = ctx.run::<wisp_runtime_api::commands::Get>(call, wisp_runtime_api::commands::IdArgs { id: task.into() }).await else {
        return;
    };
    let tracked = s.ext_of(EXT).get("tracked").cloned().unwrap_or_default();
    let (Some(repo), store) = (
        tracked.get("repo").and_then(serde_json::Value::as_str),
        tracked.get("store").and_then(serde_json::Value::as_str).unwrap_or("files"),
    ) else {
        return;
    };
    let Some(st) = ctx.contributions(&STORE).into_iter().find(|x| x.id() == store) else { return };
    let md = notes::md(n, &[], usize::MAX, parts::TOOL);
    let body = format!(
        "# Notes\n\n<!-- wisp keeps this file: the task's Notes (rev {}). Edit them in wisp, `wisp notes {task}` or with wisp_notes. -->\n\n{md}\n",
        n.rev
    );
    if let Err(e) = st.write(repo.into(), vec![(format!("tasks/{}/notes.md", parts::folder_name(task)), body.into_bytes())]).await {
        tracing::warn!(error = %e.message, "threads: notes.md not written");
    }
}

/// A task was deleted: its Notes, their revisions, meta and health go, and its long command output.
pub async fn forget(st: &State, task: &str) {
    let t = task.to_owned();
    if let Err(e) = st.store.write(move |w| crate::store::forget(w, &t)).await {
        tracing::warn!(error = %e, "threads: a deleted task's Notes stayed");
    }
    let name = parts::folder_name(task);
    if !name.is_empty() {
        // gone already, or never made (no long output): nothing to say
        let _ = tokio::fs::remove_dir_all(st.out.join(name)).await;
    }
}
