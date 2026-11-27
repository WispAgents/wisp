//! A PR link in a prompt attaches that PR to the task (ADR-0145). Pasting
//! `https://github.com/acme/api/pull/12` into the composer, a follow-up, `wisp task "…"`, an MCP `wisp_task`, a
//! Telegram or Slack message all reach `turn.started`, so the one hook covers every place a person writes from.
//! It goes through the same `attach` as `wisp_pr`: the PR shows on the row, the strip and the pane, followed and
//! kept current, and its link in the transcript reads as a live chip. Off with `ship.attachPasted`.

use std::sync::Arc;

use wisp_kernel::{Call, Ctx};
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::events::TurnStarted;
use wisp_runtime_api::model::Session;
use wisp_ship_api as api;
use wisp_ship_core::links::pr_links;

use crate::ship::Ship;
use crate::track;

/// Attach the PRs a person links in a prompt; on unless switched off.
pub const ATTACH_PASTED: &str = "ship.attachPasted";

/// The setting as stored: on unless it is `false`.
#[must_use]
pub fn attach_pasted(v: Option<&serde_json::Value>) -> bool {
    !matches!(v, Some(serde_json::Value::Bool(false)))
}

/// Who wrote the turn's prompt, when it wasn't a person: a goal check or a compaction never pastes a link.
fn by_wisp(by: Option<&str>) -> bool {
    matches!(by, Some("wisp" | "compact"))
}

/// The links not on the task yet: its own repos' PRs and the attached ones are skipped (no gh call, no flicker).
#[must_use]
pub fn new_links(task: Option<&api::ShipTask>, links: Vec<String>) -> Vec<String> {
    let Some(t) = task else { return links };
    let has = |u: &str| t.prs.iter().any(|x| x.pr.url == u) || t.repos.iter().any(|r| r.pr.as_ref().is_some_and(|p| p.url == u));
    links.into_iter().filter(|u| !has(u)).collect()
}

/// A task the ship doesn't follow yet (no worktree, or just restarted) gets what its slot kept, else one whose PRs
/// are the attached ones, gh running in its folder; never a blank one over the slot. Shared with `wisp_pr`.
pub(crate) fn ensure_followed(ship: &Ship, t: &Session) {
    let id = t.id.to_string();
    if ship.tasks.get(&id).is_some() {
        return;
    }
    let kept = wisp_ship_core::slot::sync(wisp_ship_core::slot::restore(&id, t.ext_of(api::PLUGIN)), &crate::wire::facts_of(t));
    let child = t.parent_id.is_some();
    ship.tasks.put(kept.unwrap_or_else(|| api::ShipTask { session: id.clone(), cwd: t.cwd.clone(), child, ..api::ShipTask::default() }));
}

/// On `turn.started`: read the turn's prompt and attach each new PR it links. A link gh can't read is left as text.
pub(crate) async fn turn_started(ctx: &Ctx, ship: &Arc<Ship>, e: &TurnStarted) {
    if !attach_pasted(ctx.setting(ATTACH_PASTED).as_ref()) {
        return;
    }
    let call = ctx.root_call();
    let id = e.session_id.as_str();
    let args = rt::TurnsArgs { id: e.session_id.clone(), before: None, limit: Some(2), masked: None };
    let Ok(turns) = ctx.run::<rt::Turns>(&call, args).await else { return };
    let Some(turn) = turns.iter().find(|t| t.id == e.turn_id) else { return };
    if by_wisp(turn.by.as_deref()) {
        return;
    }
    if new_links(ship.tasks.get(id).as_ref(), pr_links(&turn.prompt)).is_empty() {
        return;
    }
    let Ok(session) = ctx.run::<rt::Get>(&call, rt::IdArgs { id: id.into() }).await else { return };
    ensure_followed(ship, &session);
    let attached = attach_links(ship, &call, id, &turn.prompt).await;
    track(ctx, "pr_pasted", serde_json::json!({ "attached": attached }));
}

/// Attaches each new PR `text` links to a followed task, one gh read each; how many attached.
pub async fn attach_links(ship: &Ship, call: &Call, id: &str, text: &str) -> u32 {
    let mut attached = 0;
    for url in new_links(ship.tasks.get(id).as_ref(), pr_links(text)) {
        if ship.attach(call, id, &url, false).await.is_ok() {
            attached += 1;
        }
    }
    attached
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ship_api::{AttachedPr, PrSummary};

    fn pr(url: &str) -> PrSummary {
        PrSummary { url: url.into(), ..PrSummary::default() }
    }

    #[test]
    fn links_already_on_the_task_are_not_attached_again() {
        let task = api::ShipTask {
            prs: vec![AttachedPr { repo: "b".into(), pr: pr("https://github.com/a/b/pull/1") }],
            ..api::ShipTask::default()
        };
        let got = new_links(Some(&task), vec!["https://github.com/a/b/pull/1".into(), "https://github.com/a/b/pull/2".into()]);
        assert_eq!(got, vec!["https://github.com/a/b/pull/2".to_owned()]);
    }

    #[test]
    fn the_setting_is_on_unless_switched_off_and_wisp_own_prompts_never_attach() {
        assert!(attach_pasted(None));
        assert!(attach_pasted(Some(&serde_json::json!(true))));
        assert!(!attach_pasted(Some(&serde_json::json!(false))));
        assert!(by_wisp(Some("wisp")) && by_wisp(Some("compact")));
        assert!(!by_wisp(None) && !by_wisp(Some("channel")) && !by_wisp(Some("crew")));
    }
}
