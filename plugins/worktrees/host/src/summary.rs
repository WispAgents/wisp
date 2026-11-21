//! `runtime.summary@1`: what the list row says of a task's worktree, from its `ext["wisp.worktrees"]` slot.
//! `branch` (the shell's task view and Ship it read it: no branch, no Ship it), `foreign` when it was brought
//! in from another tool (never wisp's to remove; Keep worktree isn't offered), and `project`, the last folder
//! of the repo it was made from (the task's cwd is the worktree's own folder, whose name says nothing).

use std::collections::BTreeMap;

use serde_json::{Value, json};
use wisp_runtime_api::model::{Session, Turn};
use wisp_worktrees_api::EXT;

use crate::runtime_port::project_of;

#[derive(Debug)]
pub struct Summary;

/// The row's fields from the slot; nothing when the task has no worktree.
pub fn fields_of(slot: Option<&Value>, out: &mut BTreeMap<String, Value>) {
    let Some(slot) = slot.filter(|s| s.is_object()) else { return };
    let text = |k: &str| slot.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    if text("path").is_none() {
        return;
    }
    if let Some(b) = text("branch") {
        out.insert("branch".into(), json!(b));
    }
    if slot.get("foreign").is_some_and(|f| f.as_bool() == Some(true) || f.as_str().is_some_and(|s| !s.is_empty())) {
        out.insert("foreign".into(), json!(true));
    }
    if let Some(p) = text("root").map(project_of).filter(|p| !p.is_empty()) {
        out.insert("project".into(), json!(p));
    }
}

impl wisp_runtime_api::points::SummaryField for Summary {
    fn plugin(&self) -> &str {
        EXT
    }
    fn fields(&self, session: &Session, _last: Option<&Turn>, out: &mut BTreeMap<String, Value>) {
        fields_of(session.ext.get(EXT), out);
        // the task it's a side discussion of (runtime's `sideOf`), or its parent
        let side = session.ext.get("wisp.runtime").and_then(|r| r.get("sideOf"));
        let of = side
            .and_then(|o| o.get("id").or(Some(o)))
            .and_then(Value::as_str)
            .or_else(|| session.parent_id.as_ref().map(wisp_runtime_api::SessionId::as_str));
        if !out.contains_key("project")
            && let Some(p) = of.and_then(|parent| borrowed_project(&session.cwd, parent))
        {
            out.insert("project".into(), json!(p));
        }
    }
}

/// A side discussion (`/side`, `/btw`) runs in its parent's worktree with no slot of its own: its project is the one
/// that worktree was made from, read from the folder wisp named `<home>/worktrees/<project>-<parent id>` (audit F
/// orchestration F5: `wisp ls` said "proj-01a1f532312", the folder's own name).
pub fn borrowed_project(cwd: &str, parent: &str) -> Option<String> {
    let parts: Vec<&str> = cwd.split(['/', '\\']).collect();
    let suffix = format!("-{parent}");
    parts.windows(2).find_map(|w| {
        let p = w[1].strip_suffix(&suffix).filter(|_| w[0] == "worktrees")?;
        (!p.is_empty() && !parent.is_empty()).then(|| p.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(slot: &Value) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        fields_of(Some(slot), &mut out);
        out
    }

    /// The row says the worktree's branch and project; a brought-in one is foreign; no worktree says nothing.
    #[test]
    fn the_rows_worktree_fields_are_its_branch_project_and_foreign() {
        let own = of(&json!({ "path": "/h/.wisp/worktrees/app-fix", "branch": "wisp/fix", "root": "/src/app/", "foreign": false }));
        assert_eq!(own.get("branch"), Some(&json!("wisp/fix")));
        assert_eq!(own.get("project"), Some(&json!("app")));
        assert!(!own.contains_key("foreign"));
        let brought = of(&json!({ "path": "/u/conductor/workspaces/app/oslo", "branch": "oslo", "root": "/src/app", "foreign": true }));
        assert_eq!(brought.get("foreign"), Some(&json!(true)));
        assert_eq!(of(&json!({ "path": "/c/x", "branch": "x", "foreign": "conductor" })).get("foreign"), Some(&json!(true)), "v1's mark");
        assert!(!of(&json!({ "path": "/c/x", "branch": "x" })).contains_key("project"), "no root: no project");
        assert!(of(&json!({ "kept": true })).is_empty(), "only Keep in the slot: no worktree");
        assert!(of(&Value::Null).is_empty());
        let mut none = BTreeMap::new();
        fields_of(None, &mut none);
        assert!(none.is_empty());
    }

    /// Audit F orchestration F5: a side discussion in its parent's worktree read its project as that folder's name.
    #[test]
    fn a_side_discussion_in_its_parents_worktree_is_in_that_project_audit_f_orchestration_f5() {
        let p = "01a1f5323120aa00bb11cc22dd33ee44";
        assert_eq!(borrowed_project(&format!("/h/.wisp/worktrees/proj-{p}"), p).as_deref(), Some("proj"));
        assert_eq!(borrowed_project(&format!("/h/.wisp/worktrees/my-app-{p}/web/src"), p).as_deref(), Some("my-app"), "a subfolder");
        assert_eq!(borrowed_project(&format!(r"C:\h\worktrees\proj-{p}"), p).as_deref(), Some("proj"), "Windows");
        assert_eq!(borrowed_project("/src/proj", p), None, "in place: its folder says it");
        assert_eq!(borrowed_project(&format!("/h/worktrees/proj-{p}"), "other"), None, "another task's worktree");
        assert_eq!(borrowed_project(&format!("/h/elsewhere/proj-{p}"), p), None, "not wisp's worktrees folder");
    }
}
