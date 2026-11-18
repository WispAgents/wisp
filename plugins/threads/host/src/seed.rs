//! A formula's task starts with Notes (ADR-0097, wisp as code ADR-0026): `notes: { goal, constraints, … }` on a task
//! (`wisp run -f`, a declared task's run, a rerun) is kept in its spec by runtime (`StartArgs::notes`); when the task
//! is made (`session.created`) each section is written as you wrote it (`by: you`), once: a task that has Notes
//! already keeps them.

use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_runtime_api::commands::{IdArgs, SpecOf};
use wisp_threads_api::{By, SeedNotes};

use crate::{State, db, notes};

/// Seeds `task`'s Notes from its spec. How many sections it wrote.
pub async fn seed(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<usize> {
    let spec = ctx.run::<SpecOf>(call, IdArgs { id: task.into() }).await?;
    let Some(v) = spec.notes.filter(|v| !v.is_null()) else { return Ok(0) };
    let seed: SeedNotes = serde_json::from_value(v).map_err(|e| WireError::invalid(format!("the task's notes: {e}")))?;
    let (n, _) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    if n.rev > 0 {
        return Ok(0);
    }
    let mut wrote = 0;
    for (section, text) in seed.sections() {
        let w = notes::update(&st.store, task, section, &text, By::You, crate::now(ctx), 0)
            .await
            .map_err(|e| db(&e))?
            .map_err(WireError::invalid)?;
        wrote += usize::from(w.changed);
    }
    if wrote > 0 {
        let (n, _) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
        crate::files::write_repo_file(ctx, call, task, &n).await;
    }
    Ok(wrote)
}
