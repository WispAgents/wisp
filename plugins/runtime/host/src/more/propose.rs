//! A side discussion's writes (audit user-side-edits-for-approval). It has its main task's access: its commands and its
//! writes to files the main task never touched run without a card. A write to a file the main task changed (its turns'
//! changes) isn't made: it becomes a proposal on the MAIN task (`ext["wisp.runtime"].proposals`), its old and new text,
//! and the side's CLI hears that it was proposed there. The user applies it on the main task's page (written only while
//! the file still reads as it did, never in part) or discards it; either way the proposal goes, and an applied one is
//! told to the main task's agent before its next turn.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_runtime_api::harness::{FileChangeOf, FileWrite, TextEdit};
use wisp_runtime_api::model::{Session, ToolKind};
use wisp_runtime_api::{Harness, SessionId};
use wisp_runtime_core::prompt::side::SIDE_PROPOSED;

use super::{SLOT, slot, tool_row, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// The main task's pending proposals.
const PROPOSALS: &str = "proposals";
/// The proposals the user applied since the main task's last turn: its agent hears them first (`take_applied`).
const APPLIED: &str = "sideApplied";

/// One proposed write, as the main task keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Proposal {
    pub id: String,
    /// the side discussion it came from
    pub side: String,
    pub side_title: String,
    /// as the page names it: under the main task's folder, else as it is
    pub path: String,
    /// where it is written
    pub file: String,
    /// the file as it read when it was proposed (None: there was none)
    pub before: Option<String>,
    pub after: String,
    pub at: i64,
}

/// `p` without `.` and `..`, made absolute against `cwd` (no disk read: the file may not be there).
fn absolute(p: &str, cwd: &str) -> PathBuf {
    let joined = if Path::new(p).is_absolute() { PathBuf::from(p) } else { Path::new(cwd).join(p) };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `text` with `edits` made in order; None when one's old text isn't there (the CLI refuses that call itself).
fn edited(text: &str, edits: &[TextEdit]) -> Option<String> {
    let mut out = text.to_owned();
    for e in edits {
        if e.old.is_empty() || !out.contains(&e.old) {
            return None;
        }
        out = if e.all { out.replace(&e.old, &e.new) } else { out.replacen(&e.old, &e.new, 1) };
    }
    Some(out)
}

/// The file `before` becomes, for a change wisp can work out.
fn after_of(before: Option<&str>, change: &FileChangeOf) -> Option<String> {
    match change {
        FileChangeOf::Whole { text } => Some(text.clone()),
        FileChangeOf::Edits { edits } => edited(before.unwrap_or_default(), edits),
        FileChangeOf::Opaque => None,
    }
}

fn proposals(s: &Session) -> Vec<Proposal> {
    slot(s)
        .and_then(|m| m.get(PROPOSALS))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| serde_json::from_value(p.clone()).ok()).collect())
        .unwrap_or_default()
}

fn set_proposals(s: &mut Session, list: &[Proposal]) {
    with_slot(&mut s.ext, |m| if list.is_empty() { m.remove(PROPOSALS) } else { m.insert(PROPOSALS.into(), json!(list)) });
}

/// What the main task's agent hears first on its next turn about the proposals the user applied (taken off the task).
pub(crate) fn take_applied(s: &mut Session) -> Option<String> {
    let list = with_slot(&mut s.ext, |m| m.remove(APPLIED))?;
    let lines: Vec<String> = list
        .as_array()?
        .iter()
        .map(|a| {
            format!(
                "- `{}`, from the side discussion “{}”",
                a.get("path").and_then(Value::as_str).unwrap_or_default(),
                a.get("sideTitle").and_then(Value::as_str).unwrap_or_default()
            )
        })
        .collect();
    (!lines.is_empty()).then(|| {
        format!(
            "(wisp: since your last turn the user applied a change a side discussion proposed to these files; they read as changed now, so read them again before you edit them:\n{})\n\n",
            lines.join("\n")
        )
    })
}

impl Runtime {
    /// The files the main task's turns changed, absolute.
    async fn touched(&self, main: &Session) -> BTreeSet<PathBuf> {
        let turns = self.repo.turns(main.id.as_str(), None, None).await.unwrap_or_default();
        turns
            .iter()
            .filter_map(|t| t.ext.get(SLOT)?.get("changes")?.as_array().cloned())
            .flatten()
            .filter_map(|c| c.get("file").and_then(Value::as_str).map(|f| absolute(f, &main.cwd)))
            .collect()
    }

    /// A side run's call through wisp's approve tool (`runtime.approval`): its answer, with no card. Its commands and
    /// its writes to files the main task never touched go through; a write to one it changed is proposed there.
    pub(crate) async fn side_approval(self: &Arc<Self>, s: &Session, h: &dyn Harness, tool: &str, input: &Value) -> Result<Value> {
        let allow = json!({ "behavior": "allow", "updatedInput": input });
        let (Some(main_id), Some(FileWrite { path, change })) = (crate::run::side_main(s), h.file_write(tool, input)) else {
            return Ok(allow);
        };
        let Ok(main) = self.repo.session(main_id).await else { return Ok(allow) };
        let file = absolute(&path, &main.cwd);
        if !self.touched(&main).await.contains(&file) {
            return Ok(allow);
        }
        let shown = file.strip_prefix(&main.cwd).map_or_else(|_| file.display().to_string(), |p| p.display().to_string());
        // a .env file's text never goes in a proposal (the task's row carries it to every client: ADR-0050)
        if wisp_envmask::is_env_file(&shown) {
            return Ok(
                json!({ "behavior": "deny", "message": format!("Not written: the main task changed {shown}, a .env file wisp won't copy into a proposal. Say what to change instead.") }),
            );
        }
        let before = tokio::fs::read_to_string(&file).await.ok();
        let last = self.last_turn(s.id.as_str()).await?;
        let Some(after) = after_of(before.as_deref(), &change) else {
            if matches!(change, FileChangeOf::Opaque) {
                return Ok(
                    json!({ "behavior": "deny", "message": format!("Not written: the main task changed {shown}, and wisp can't propose this kind of change to it. Say what to change instead.") }),
                );
            }
            // its old text isn't there: the CLI refuses the call itself, nothing is written
            return Ok(allow);
        };
        let p = Proposal {
            id: wisp_kernel::ids::uuid7(),
            side: s.id.0.clone(),
            side_title: s.title.clone(),
            path: shown.clone(),
            file: file.display().to_string(),
            before,
            after,
            at: self.now(),
        };
        {
            let _g = self.task_lock(main_id).await;
            let mut m = self.repo.session(main_id).await?;
            let mut list = proposals(&m);
            list.push(p);
            set_proposals(&mut m, &list);
            self.persist(&mut m, Vec::new()).await?;
        }
        if let Some(t) = &last {
            self.add_row(s.id.as_str(), t.id.as_str(), tool_row(ToolKind::Answer, format!("Proposed to the main task: {shown}"))).await?;
        }
        Ok(json!({ "behavior": "deny", "message": SIDE_PROPOSED }))
    }

    /// `runtime.proposal.apply`: the proposal written in the main task's folder, only while the file reads as it did
    /// when it was proposed (else a Conflict, nothing written); then it goes, and the main task's agent hears of it.
    pub async fn apply_proposal(self: &Arc<Self>, id: &SessionId, proposal: &str) -> Result<Value> {
        let p = {
            let _g = self.task_lock(id.as_str()).await;
            let mut s = self.repo.session(id.as_str()).await?;
            let mut list = proposals(&s);
            let Some(at) = list.iter().position(|p| p.id == proposal) else {
                return Err(RuntimeError::NoTurn("That proposal isn't on this task any more.".into()));
            };
            let p = list[at].clone();
            let now = tokio::fs::read_to_string(&p.file).await.ok();
            if now != p.before {
                return Err(RuntimeError::Conflict(format!(
                    "{} changed since the side discussion proposed this, so nothing was written. Discard it, or ask the side discussion again.",
                    p.path
                )));
            }
            if let Some(dir) = Path::new(&p.file).parent() {
                tokio::fs::create_dir_all(dir).await.map_err(|e| RuntimeError::Conflict(format!("Couldn't write {}: {e}", p.path)))?;
            }
            tokio::fs::write(&p.file, &p.after).await.map_err(|e| RuntimeError::Conflict(format!("Couldn't write {}: {e}", p.path)))?;
            list.remove(at);
            set_proposals(&mut s, &list);
            with_slot(&mut s.ext, |m| super::push(m, APPLIED, json!({ "path": p.path, "side": p.side, "sideTitle": p.side_title })));
            self.persist(&mut s, Vec::new()).await?;
            p
        };
        self.side_note(&p.side, &format!("Applied on the main task: {}", p.path)).await;
        Ok(json!({ "applied": p.path }))
    }

    /// `runtime.proposal.discard`: the proposal goes, nothing written; its side discussion says so.
    pub async fn discard_proposal(self: &Arc<Self>, id: &SessionId, proposal: &str) -> Result<Value> {
        let p = {
            let _g = self.task_lock(id.as_str()).await;
            let mut s = self.repo.session(id.as_str()).await?;
            let mut list = proposals(&s);
            let Some(at) = list.iter().position(|p| p.id == proposal) else {
                return Err(RuntimeError::NoTurn("That proposal isn't on this task any more.".into()));
            };
            let p = list.remove(at);
            set_proposals(&mut s, &list);
            self.persist(&mut s, Vec::new()).await?;
            p
        };
        self.side_note(&p.side, &format!("Discarded on the main task: {}", p.path)).await;
        Ok(json!({ "discarded": p.path }))
    }

    /// A row on the side discussion's last turn (it may be gone: nothing then).
    async fn side_note(self: &Arc<Self>, side: &str, text: &str) {
        if let Ok(Some(t)) = self.last_turn(side).await
            && let Err(err) = self.add_row(side, t.id.as_str(), tool_row(ToolKind::Answer, text.to_owned())).await
        {
            tracing::warn!(task = side, %err, "runtime: a side discussion's proposal note");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_made_absolute_against_the_main_tasks_folder() {
        assert_eq!(absolute("src/a.ts", "/w"), PathBuf::from("/w/src/a.ts"));
        assert_eq!(absolute("/w/./src/../a.ts", "/x"), PathBuf::from("/w/a.ts"));
    }

    #[test]
    fn edits_are_made_in_order_and_a_missing_old_text_makes_none() {
        let e = |old: &str, new: &str, all| TextEdit { old: old.into(), new: new.into(), all };
        assert_eq!(edited("a a b", &[e("a", "x", false), e("b", "y", false)]).as_deref(), Some("x a y"));
        assert_eq!(edited("a a", &[e("a", "x", true)]).as_deref(), Some("x x"));
        assert_eq!(edited("a", &[e("z", "x", false)]), None);
        assert_eq!(after_of(None, &FileChangeOf::Whole { text: "new".into() }).as_deref(), Some("new"));
        assert_eq!(after_of(Some("x"), &FileChangeOf::Opaque), None);
    }
}
