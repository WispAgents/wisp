//! What wisp.threads gives other plugins through their points: the agent's `wisp_notes` tool (`mcp.tool@1`), the
//! note before each follow-up (`runtime.prompt@1` `before`), and the folder long command output goes to
//! (`runtime.run@1`, `WISP_OFFLOAD_DIR`).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Setup};
use wisp_mcp_api::{Audience, TOOL_POINT, ToolContribution, ToolText};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{OFFLOAD_DIR_ENV, PROMPT, PromptPart, RUN, RunPart, TurnNote};
use wisp_threads_api::{self as api, By, SECTIONS};
use wisp_threads_core::note::{LEAD_MAX, NOTE_MAX, NoteIn, lead_note, turn_note};

use crate::{Slot, notes, state};

pub const TOOL: &str = "wisp_notes";

/// How the agent is told to keep its Notes, in front of a task's first turn (v1 protocol.js `LEDGER_TOOL`).
fn first_turn(tool: &str) -> String {
    format!(
        "For work longer than a quick answer, keep this task's Notes with `{tool}`: its goal, constraints, decisions and why, what you tried that failed and why, where it stands, open questions, key files. Write them once you know the goal; update them when something is decided, fails or changes. A section is rewritten whole (never appended to), so when an instruction changes, rewrite that section and the old version is gone. wisp sends them back with each follow-up and after a compaction, so they are what survives when this conversation doesn't."
    )
}

fn tool_def() -> Value {
    let sections = SECTIONS.join(", ");
    json!({
        "name": TOOL,
        "description": "Read or rewrite this task's Notes: the state of the work that must survive a long conversation, compaction or a fresh thread. Sections: goal (what done looks like), constraints (must / must not), decisions (what was chosen, and why), failed (what was tried and didn't work, and why), state (where it stands, what is next), open (open questions), files (key paths). A section is rewritten whole, never appended to: send it as it should be from now on, list sections one item a line. wisp sends the Notes back to you with each follow-up. Call with no arguments to read them. The user can edit them too. A subagent reads its lead's Notes and can't change them.",
        "inputSchema": { "type": "object", "properties": {
            "set": { "type": "object", "description": format!("Sections to rewrite, each the whole new text: {{ \"goal\": \"…\", \"constraints\": \"No Redis\\nKeep the API\" }}. One of: {sections}. \"\" clears one."), "additionalProperties": { "type": "string" } }
        } }
    })
}

/// What the agent reads back.
fn answer(n: &api::Notes, changed: &[String], problems: &[String]) -> String {
    let md = notes::md(n, &[], usize::MAX, TOOL);
    let mut head = String::new();
    if !changed.is_empty() {
        head = format!("Updated {} (rev {}).\n\n", changed.join(", "), n.rev);
    }
    if !problems.is_empty() {
        let _ = write!(head, "Problems: {}\n\n", problems.join("; "));
    }
    let whose = if n.lead.is_some() { " (your lead’s, read-only: report what should change)" } else { "" };
    format!("{head}Task Notes{whose}:\n\n{}", if md.is_empty() { "(empty)" } else { &md })
}

fn tool(_slot: &Slot) -> ToolContribution {
    ToolContribution {
        tool: tool_def(),
        audience: Audience::Agent,
        read_only: false,
        capped_off: false,
        run: Arc::new(move |ctx, call, tc| {
            Box::pin(async move {
                let Some(task) = tc.session.clone() else {
                    return Ok(ToolText { text: None, error: Some("wisp_notes is a task's tool: no task here".into()) });
                };
                let set = tc.args.get("set").and_then(Value::as_object).cloned().unwrap_or_default();
                let (mut changed, mut problems) = (Vec::new(), Vec::new());
                for (section, text) in &set {
                    let text = match text {
                        Value::String(s) => s.clone(),
                        Value::Array(a) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"),
                        Value::Null => String::new(),
                        v => v.to_string(),
                    };
                    let before = ctx.run::<api::NotesCmd>(&call, api::TaskArgs { id: task.clone() }).await?;
                    match ctx
                        .run::<api::NotesUpdate>(
                            &call,
                            api::UpdateArgs { id: task.clone(), section: section.clone(), text, by: Some(By::Agent) },
                        )
                        .await
                    {
                        Ok(n) if n.rev > before.rev => changed.push(section.clone()),
                        Ok(_) => {}
                        Err(e) => problems.push(e.message),
                    }
                }
                let n = ctx.run::<api::NotesCmd>(&call, api::TaskArgs { id: task }).await?;
                Ok(ToolText { text: Some(answer(&n, &changed, &problems)), error: None })
            })
        }),
    }
}

/// The note before a follow-up.
struct Before(Slot);

impl PromptPart for Before {
    fn id(&self) -> &'static str {
        "wisp.threads"
    }
    fn part(&self, _session: &Session) -> String {
        String::new()
    }
    fn before(&self, q: TurnNote) -> BoxFuture<'static, String> {
        let slot = self.0.clone();
        Box::pin(async move {
            let (Ok(st), Some(tools)) = (state(&slot), q.tools.as_deref()) else { return String::new() };
            let tool = format!("{tools}{TOOL}");
            // a subagent works for its lead: the lead's goal, constraints, decisions and what failed go ahead of its task
            if let Some(lead) = &q.session.parent_id {
                let Ok((n, _)) = notes::load(&st.store, lead.as_str()).await else { return String::new() };
                return lead_note(&notes::md(&n, &["goal", "constraints", "decisions", "failed"], LEAD_MAX, &tool)).unwrap_or_default();
            }
            let task = q.session.id.as_str();
            let Ok((n, _)) = notes::load(&st.store, task).await else { return String::new() };
            let md = notes::md(&n, &[], NOTE_MAX, &tool);
            let mut out = Vec::new();
            if q.turn.idx == 0 {
                out.push(first_turn(&tool));
            }
            if let Some(note) = turn_note(&NoteIn {
                notes: &md,
                rev: n.rev,
                yours_unseen: notes::yours_unseen(&n),
                turn: q.turn.idx,
                prompt: &q.turn.prompt,
                tool: &tool,
            }) {
                out.push(note);
            }
            if !md.is_empty()
                && let Err(e) = notes::mark_seen(&st.store, task, n.rev).await
            {
                tracing::warn!(error = %e, "threads: seen not kept");
            }
            out.join("\n\n")
        })
    }
}

/// The folder each run's long command output goes to.
struct Out(Slot);

/// A task id as a folder name (ids are letters, digits and dashes; anything else is dropped).
pub fn folder_name(task: &str) -> String {
    task.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect()
}

impl RunPart for Out {
    fn id(&self) -> &'static str {
        "wisp.threads"
    }
    fn env(&self, session: &Session) -> BTreeMap<String, String> {
        let Ok(st) = state(&self.0) else { return BTreeMap::new() };
        let name = folder_name(session.id.as_str());
        if name.is_empty() {
            return BTreeMap::new();
        }
        BTreeMap::from([(OFFLOAD_DIR_ENV.to_owned(), st.out.join(name).display().to_string())])
    }
}

pub fn register(s: &mut Setup, slot: &Slot) {
    s.contribute(&TOOL_POINT, Arc::new(tool(slot)));
    s.contribute(&PROMPT, Arc::new(Before(slot.clone())) as Arc<dyn PromptPart>);
    s.contribute(&RUN, Arc::new(Out(slot.clone())) as Arc<dyn RunPart>);
}
