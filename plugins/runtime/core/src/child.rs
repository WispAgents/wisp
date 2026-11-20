//! A task an agent starts gets nothing its own task may not have (review r194 round 2 #2, #4): the one check
//! `runtime.create` runs whenever the caller is an agent's token, however it got there (`wisp_launch`, a raw
//! `/api/cmd`, a plugin that kept the agent's call). Pure: the host reads the parent task and carries out the answer.

use serde_json::Value;
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::model::Mode;

use crate::agent_task::mode_label;

/// What the asking agent's own task has.
#[derive(Debug, Clone, Default)]
pub struct Parent {
    pub id: String,
    pub mode: Mode,
    /// the cap a channel gave it
    pub cap: Option<Mode>,
    pub crew: String,
    pub account: String,
    pub connectors: Vec<String>,
    /// the team it was started with, by name
    pub team: String,
}

/// The tighter of the parent's mode and its cap: no child goes above it.
#[must_use]
pub fn ceiling(p: &Parent) -> Mode {
    crate::cap::tighter(Some(p.mode), p.cap).unwrap_or(p.mode)
}

fn no(what: &str) -> String {
    format!("Refused: a task an agent starts can't {what}. Say so in your report; the user can start it in wisp.")
}

/// `a` held to `p`, before anything is made: refused (one plain line) when it asks for more than `p` has; the parent's
/// cap carried to the child.
///
/// # Errors
/// The first field that asks for more than the parent has.
pub fn hold(a: &mut StartArgs, p: &Parent) -> Result<(), String> {
    let top = ceiling(p);
    if let Some(m) = a.mode.filter(|m| m.rank() > top.rank()) {
        return Err(format!(
            "Refused: {} is above this task's {}: a task it starts can't do more than it may. Use {} or lower.",
            mode_label(m),
            mode_label(top),
            mode_label(top)
        ));
    }
    if let Some(c) = a.connectors.iter().find(|c| !p.connectors.contains(c)) {
        return Err(no(&format!("have a connector its own task doesn't have ({c})")));
    }
    if a.crew.as_deref().is_some_and(|c| !c.is_empty() && c != p.crew) {
        return Err(no("run for a crew member its own task doesn't run for"));
    }
    if a.account.as_deref().is_some_and(|c| !c.is_empty() && c != p.account) {
        return Err(no("use an account its own task doesn't use"));
    }
    match &a.team {
        Some(Value::String(t)) if !t.is_empty() && *t != p.team => return Err(no("have a team its own task doesn't have")),
        Some(Value::Array(rows))
            if rows
                .iter()
                .any(|r| r.get("machine").and_then(Value::as_str).is_some_and(|m| !m.is_empty() && !matches!(m, "this" | "here"))) =>
        {
            return Err(no("send its team to another machine"));
        }
        _ => {}
    }
    if a.confirm_limit {
        return Err(no("go past Ask near the limit"));
    }
    if a.profile.is_some() {
        return Err(no("give a task it starts a profile: name an agent"));
    }
    if !a.ext.is_empty() {
        return Err(no("set a task's ext"));
    }
    if a.worktree_key.as_deref().is_some_and(|k| !k.is_empty()) || a.worktree_tidy {
        return Err(no("pick another task's worktree"));
    }
    if a.parent_id.as_ref().is_some_and(|x| x.as_str() != p.id) {
        return Err(no("start a task under another task"));
    }
    // the parent's cap travels with it (#4): the child is held to it on every turn, as its parent is
    a.cap = crate::cap::tighter(a.cap, p.cap);
    Ok(())
}

/// An agent's follow-up to its own task: never above the task's own mode or cap, never past Ask near the limit.
///
/// # Errors
/// A mode above it, or `confirm_limit`.
pub fn follow_up(mode: Option<Mode>, confirm_limit: bool, p: &Parent) -> Result<(), String> {
    let top = ceiling(p);
    if let Some(m) = mode.filter(|m| m.rank() > top.rank()) {
        return Err(format!(
            "Refused: {} is above this task's {}: an agent can't lift its own task. The user can, in wisp.",
            mode_label(m),
            mode_label(top)
        ));
    }
    if confirm_limit {
        return Err(no("go past Ask near the limit"));
    }
    Ok(())
}

/// An agent setting a task's mode: lower or the same, never higher than it is (review r194 round 3 #1). Only the user
/// raises a mode.
///
/// # Errors
/// A mode above the task's.
pub fn set_mode(asked: Mode, now: Mode) -> Result<(), String> {
    if asked.rank() > now.rank() {
        return Err(format!(
            "Refused: an agent can't raise a task's mode ({} to {}). Say so in your report; the user can, in wisp.",
            mode_label(now),
            mode_label(asked)
        ));
    }
    Ok(())
}

/// The mode the child got from its agent's profile or the user's default, held to the parent's (asked outright above
/// it was refused by [`hold`]).
#[must_use]
pub fn held_mode(resolved: Option<Mode>, p: &Parent) -> Mode {
    crate::cap::at_cap(resolved.unwrap_or(Mode::Auto), Some(ceiling(p)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parent() -> Parent {
        Parent { id: "p".into(), mode: Mode::Plan, connectors: vec!["linear".into()], ..Parent::default() }
    }

    #[test]
    fn a_child_gets_nothing_its_parent_lacks() {
        let try_ = |a: StartArgs| hold(&mut a.clone(), &parent());
        assert!(try_(StartArgs { mode: Some(Mode::Auto), ..StartArgs::default() }).unwrap_err().contains("above this task's Plan"));
        assert!(try_(StartArgs { connectors: vec!["linear".into()], ..StartArgs::default() }).is_ok());
        assert!(try_(StartArgs { connectors: vec!["jira".into()], ..StartArgs::default() }).unwrap_err().contains("(jira)"));
        assert!(try_(StartArgs { crew: Some("ada".into()), ..StartArgs::default() }).is_err());
        assert!(try_(StartArgs { account: Some("work".into()), ..StartArgs::default() }).is_err());
        assert!(try_(StartArgs { team: Some(json!("core")), ..StartArgs::default() }).is_err());
        assert!(try_(StartArgs { team: Some(json!([{ "agent": "codex", "machine": "box" }])), ..StartArgs::default() }).is_err());
        assert!(try_(StartArgs { team: Some(json!([{ "agent": "codex" }])), ..StartArgs::default() }).is_ok());
        assert!(try_(StartArgs { confirm_limit: true, ..StartArgs::default() }).is_err());
        assert_eq!(held_mode(Some(Mode::Auto), &parent()), Mode::Plan);
        assert_eq!(held_mode(None, &parent()), Mode::Plan, "the default Full auto, held to Plan");
        let mut capped = StartArgs::default();
        hold(&mut capped, &Parent { cap: Some(Mode::Edits), mode: Mode::Edits, ..parent() }).unwrap();
        assert_eq!(capped.cap, Some(Mode::Edits), "the cap travels");
    }
}
