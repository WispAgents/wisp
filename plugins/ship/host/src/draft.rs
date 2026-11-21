//! The dialog's default commit message and PR text when the caller sends none (v1 `ship.draft`): from the
//! task's last finished answer (its hero, the blocks a PR body keeps), else the task's title; the commit
//! type from its first prompt too.

use wisp_kernel::{Call, Ctx};
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::model::TurnStatus;
use wisp_ship_api::Draft;
use wisp_ship_core::text::{DraftInput, draft, presentation_parts};

fn turns(id: &str, before: Option<u32>, limit: u32) -> rt::TurnsArgs {
    rt::TurnsArgs { id: id.into(), before, limit: Some(limit), masked: None }
}

/// The draft for a task, or an empty one when runtime can't say (the dialog's fields stay blank).
pub async fn draft_for(ctx: &Ctx, call: &Call, session: &str) -> Draft {
    let Ok(s) = ctx.run::<rt::Get>(call, rt::IdArgs { id: session.into() }).await else { return Draft::default() };
    let last = ctx.run::<rt::Turns>(call, turns(session, None, 5)).await.unwrap_or_default();
    let first = ctx.run::<rt::Turns>(call, turns(session, Some(1), 1)).await.unwrap_or_default();
    let answer = last.iter().rev().find(|t| t.status == TurnStatus::Done).or_else(|| last.last());
    let blocks = answer.and_then(|t| t.presentation.as_ref()).map(|p| p.blocks.as_slice()).unwrap_or_default();
    let (hero_title, hero_summary, markdown) = presentation_parts(blocks);
    let first_prompt = first.first().map_or("", |t| t.prompt.as_str());
    draft(&DraftInput { hero_title, hero_summary, session_title: &s.title, first_prompt, markdown: &markdown })
}

/// `ship.write` (TRY-13): the task's own harness and model, asked on the side in its worktree, write the PR's title and
/// description from the diff, the task and its answer, filling the repo's pull request template when it has one.
pub async fn write_for(ctx: &Ctx, call: &Call, session: &str, folder: &str, diff: &str) -> wisp_kernel::Result<Draft> {
    use wisp_kernel::WireError;
    use wisp_ship_core::write::{TEMPLATE_PATHS, prompt, written};
    let s = ctx.run::<rt::Get>(call, rt::IdArgs { id: session.into() }).await?;
    let last = ctx.run::<rt::Turns>(call, turns(session, None, 5)).await.unwrap_or_default();
    let first = ctx.run::<rt::Turns>(call, turns(session, Some(1), 1)).await.unwrap_or_default();
    let answer = last.iter().rev().find(|t| t.status == TurnStatus::Done).or_else(|| last.last());
    let blocks = answer.and_then(|t| t.presentation.as_ref()).map(|p| p.blocks.as_slice()).unwrap_or_default();
    let (hero_title, hero_summary, markdown) = presentation_parts(blocks);
    let answered = [hero_title.unwrap_or_default(), hero_summary.unwrap_or_default(), markdown.as_str()]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut template = None;
    for p in TEMPLATE_PATHS {
        if let Ok(t) = tokio::fs::read_to_string(std::path::Path::new(folder).join(p)).await {
            template = Some(t);
            break;
        }
    }
    let ask = prompt(&s.title, first.first().map_or("", |t| t.prompt.as_str()), &answered, diff, template.as_deref());
    let model = s.model.clone();
    let pick = rt::RunPick { agent: Some(s.agent.clone()), model: (!model.is_empty()).then_some(model), ..rt::RunPick::default() };
    let args = rt::QuickArgs { prompt: ask, cwd: folder.to_owned(), pick: Some(pick), timeout_ms: Some(120_000), json: true, run_id: None };
    let out = ctx.run::<rt::Quick>(call, args).await?;
    let (title, body) = written(out.json.as_ref())
        .ok_or_else(|| WireError::unavailable(format!("{} didn't write the pull request: try again.", s.agent)))?;
    // as every PR wisp drafts: the line that says where it came from, once
    let body =
        if body.contains(wisp_ship_core::text::FOOTER) { body } else { format!("{}\n\n{}", body.trim_end(), wisp_ship_core::text::FOOTER) };
    Ok(Draft { title, body })
}

/// What the task changed since its base, as the prompt carries it (`write::diff_text`); "" when git can't tell.
/// `base_ref`: what the task's worktree branched from (its record); the repo's default base when it has none.
pub async fn diff_of(ship: &crate::ship::Ship, call: &Call, folder: &str, base_ref: &str) -> String {
    use wisp_git_api::Body;
    use wisp_ship_core::write::{DiffFile, diff_text};
    let Ok(repo) = ship.shipper.repo(call, folder).await else { return String::new() };
    let base = if base_ref.is_empty() {
        let Some(b) = repo.base(call, false).await.ok().flatten() else { return String::new() };
        b.reference
    } else {
        base_ref.to_owned()
    };
    // a refusal is logged: the agent would otherwise be told there are no changes
    let c = match repo.changes(call, &base).await {
        Ok(Some(c)) => c,
        Ok(None) => return String::new(),
        Err(e) => {
            tracing::warn!(error = %e, %base, "ship: the diff for the PR's text");
            return String::new();
        }
    };
    let files = match &c.body {
        Body::Inline { files } => files
            .iter()
            .map(|f| DiffFile {
                path: &f.path,
                status: f.status,
                add: f.add,
                del: f.del,
                rows: f.rows.iter().map(|r| (r.op, r.text.as_deref().unwrap_or_default())).collect(),
            })
            .collect::<Vec<_>>(),
        // a diff too large to send inline: its files and counts say what changed
        Body::Blob { .. } => {
            c.stats.iter().map(|f| DiffFile { path: &f.path, status: f.status, add: f.add, del: f.del, rows: Vec::new() }).collect()
        }
    };
    diff_text(&files)
}
