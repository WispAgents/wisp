//! /btw: side questions answered on a copy of the conversation, kept on the task.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use wisp_runtime_api::commands::{BtwArgs, BtwEntry, ProbeAnswer, ProbeArgs};
use wisp_runtime_api::harness::HarnessRun;
use wisp_runtime_api::model::Mode;

use super::with_slot;
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// wisp.worktrees reads a /btw pending longer than this (with room, its `SIDE_MS`) as cut off.
const BTW_TIMEOUT: Duration = Duration::from_secs(300);

/// v1 `btwAnswer`: the reply as markdown; one that ended with a wisp block anyway reads as its notes and blocks.
pub(crate) fn btw_answer(text: &str) -> String {
    let p = wisp_runtime_core::blocks::parse::parse(text);
    let rendered = (!p.fallback).then(|| {
        let md = wisp_runtime_core::blocks::blocks_to_markdown(
            &p.blocks,
            &wisp_runtime_core::blocks::render::Context { changes: &[], approval: None, summary: None },
        );
        [p.notes.unwrap_or_default(), md].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join("\n\n")
    });
    wisp_runtime_core::prompt::btw::btw_answer(text, rendered.as_deref())
}

impl Runtime {
    /// `runtime.btw`: a side question answered on a copy of the conversation, kept on the task.
    #[allow(clippy::many_single_char_names)]
    pub async fn btw(self: &Arc<Self>, a: BtwArgs) -> Result<BtwEntry> {
        if a.question.trim().is_empty() {
            return Err(RuntimeError::Invalid("Ask something first.".into()));
        }
        // claimed before its first await: a delete from here on stops it (before its spawn: it never spawns) and waits
        let claim = self.claims.claim(a.id.as_str());
        let s = self.repo.session(a.id.as_str()).await?;
        let h = self.harness(&s.agent)?;
        // where it runs first, as a turn's (ADR-0053): an archived task's worktree comes back, or it's refused with why
        self.folder_ready(&s, None).await.map_err(RuntimeError::Conflict)?;
        if claim.cancelled() {
            return Err(RuntimeError::NoTask(a.id.0.clone()));
        }
        let entry = BtwEntry {
            id: wisp_kernel::ids::uuid7(),
            question: a.question.clone(),
            answer: String::new(),
            at: self.now(),
            pending: true,
            error: None,
        };
        self.put_btw(a.id.as_str(), &entry).await?;
        let fork = h.info().fork && s.resume_id.is_some();
        // v1 launch.js askBtw: on a fork of the conversation or not, and whether the task's run was going
        let running = !matches!(self.run_state(s.id.as_str()), wisp_runtime_core::queue::Run::Idle);
        self.host.track("btw", json!({ "agent": s.agent, "forked": fork, "running": running }));
        // v1 btwPrompt: the rules of a side question, and (not forked) what the task did so far; v1 BTW_SYSTEM
        let prompt = self.btw_prompt(&s, &a.question, fork).await;
        let attachments = self.files_for_run(&a.attachments).await;
        let run = HarnessRun {
            session_id: s.id.0.clone(),
            prompt,
            system_prompt: wisp_runtime_core::prompt::btw::BTW_SYSTEM.to_owned(),
            attachments,
            cwd: s.cwd.clone(),
            dirs: s.dirs.clone(),
            resume_id: if fork { s.resume_id.clone() } else { None },
            fork,
            mode: Mode::Plan,
            model: s.model.clone(),
            effort: s.effort.clone(),
            gate: h.info().side_gate,
            quiet: self.lean_task(&s), // a /btw is yours: lean as the task is (v1 runner.js:889)
            ..HarnessRun::default()
        };
        let (rt, mut e) = (self.clone(), entry.clone());
        // owned by the plugin's scope: switching the runtime off ends the side run with it
        self.host.spawn(Box::pin(async move {
            match rt.side_run(h.as_ref(), &run, BTW_TIMEOUT, Some(claim.stop())).await {
                // v1 btwAnswer: an agent that ended with a wisp block anyway gets it read as markdown (orch #10)
                Ok(o) if !o.is_error => e.answer = btw_answer(&o.text),
                Ok(o) => e.error = Some(o.text),
                Err(err) => e.error = Some(err.to_string()),
            }
            e.pending = false;
            // a task deleted meanwhile has no row to put it on (`put_btw` reads it first): nothing is written
            match rt.put_btw(&s.id.0, &e).await {
                Ok(()) | Err(crate::error::RuntimeError::NoTask(_)) => {}
                Err(err) => tracing::warn!(task = %s.id.0, btw = %e.id, "couldn't save the side question's answer: {err}"),
            }
            drop(claim);
        }));
        Ok(entry)
    }

    /// `runtime.probe`: a question to a fork of the conversation, answering only, awaited, kept nowhere (ADR-0096).
    pub async fn probe(self: &Arc<Self>, a: ProbeArgs) -> Result<ProbeAnswer> {
        if a.question.trim().is_empty() {
            return Err(RuntimeError::Invalid("Ask something first.".into()));
        }
        let claim = self.claims.claim(a.id.as_str());
        let s = self.repo.session(a.id.as_str()).await?;
        let h = self.harness(&s.agent)?;
        let Some(resume) = s.resume_id.clone().filter(|_| h.info().fork) else {
            return Err(RuntimeError::Conflict("This task's agent can't answer on a copy of its conversation.".into()));
        };
        self.folder_ready(&s, None).await.map_err(RuntimeError::Conflict)?;
        let run = HarnessRun {
            session_id: s.id.0.clone(),
            prompt: a.question,
            cwd: s.cwd.clone(),
            dirs: s.dirs.clone(),
            resume_id: Some(resume),
            fork: true,
            mode: Mode::Plan,
            model: s.model.clone(),
            effort: s.effort.clone(),
            gate: h.info().side_gate,
            lean: true,
            quiet: true,
            ..HarnessRun::default()
        };
        let wait =
            Duration::from_millis(a.timeout_ms.unwrap_or(120_000).clamp(1_000, BTW_TIMEOUT.as_millis().try_into().unwrap_or(300_000)));
        let o = self.side_run(h.as_ref(), &run, wait, Some(claim.stop())).await?;
        drop(claim);
        if o.is_error {
            return Err(RuntimeError::Conflict(o.text));
        }
        Ok(ProbeAnswer { text: o.text, cost_usd: o.cost_usd })
    }

    /// v1 `btwPrompt`: on a fork the rules and the question; else the task's last turns written out too, with what
    /// its .env reads must not show (ADR-0050). Unknown .env reads: the rules and the question only.
    async fn btw_prompt(&self, s: &wisp_runtime_api::model::Session, question: &str, fork: bool) -> String {
        use wisp_runtime_core::prompt::btw::btw_prompt;
        use wisp_runtime_core::prompt::handoff::{PastTurn, Render};
        let render = |blocks: &[Value], _t: &wisp_runtime_api::model::Turn| {
            wisp_runtime_core::blocks::blocks_to_markdown(
                blocks,
                &wisp_runtime_core::blocks::render::Context { changes: &[], approval: None, summary: None },
            )
        };
        if fork {
            return btw_prompt(&[], question, true, self.now(), Render(&render));
        }
        let (Ok(mut all), Ok(known)) = (self.repo.turns(s.id.as_str(), None, None).await, self.env_known(s.id.as_str()).await) else {
            return btw_prompt(&[], question, false, self.now(), Render(&render));
        };
        all.sort_by_key(|t| t.idx);
        let name = |id: &str| self.harness(id).map_or_else(|_| id.to_owned(), |h| h.info().name.clone());
        let past: Vec<PastTurn<'_>> = all
            .iter()
            .map(|t| PastTurn { env: &known.0, ..PastTurn::new(t, name(if t.agent.is_empty() { &s.agent } else { &t.agent })) })
            .collect();
        btw_prompt(&past, question, false, self.now(), Render(&render))
    }

    async fn put_btw(self: &Arc<Self>, id: &str, e: &BtwEntry) -> Result<()> {
        let _g = self.task_lock(id).await;
        let mut s = self.repo.session(id).await?;
        let v = serde_json::to_value(e).map_err(|x| RuntimeError::Invalid(x.to_string()))?;
        with_slot(&mut s.ext, |m| {
            if let Some(arr) = m.entry("btw").or_insert_with(|| json!([])).as_array_mut() {
                match arr.iter_mut().find(|x| x.get("id").and_then(Value::as_str) == Some(e.id.as_str())) {
                    Some(x) => *x = v,
                    None => arr.push(v),
                }
            }
        });
        self.persist(&mut s, Vec::new()).await.map(|_| ())
    }

    /// `runtime.btw.drop`
    pub async fn btw_drop(self: &Arc<Self>, id: &str, btw: &str) -> Result<()> {
        let _g = self.task_lock(id).await;
        let mut s = self.repo.session(id).await?;
        with_slot(&mut s.ext, |m| {
            if let Some(arr) = m.get_mut("btw").and_then(Value::as_array_mut) {
                arr.retain(|x| x.get("id").and_then(Value::as_str) != Some(btw));
            }
        });
        self.persist(&mut s, Vec::new()).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::btw_answer;

    /// orch #10: an answer that ended with a wisp block anyway reads as its text; a plain one stays as it is.
    #[test]
    fn a_btw_answer_with_a_wisp_block_reads_as_markdown() {
        let text = "All done.\n\n```wisp\n{\"blocks\":[{\"type\":\"hero\",\"title\":\"Tests pass\",\"summary\":\"12 of 12.\",\"status\":\"done\"}]}\n```";
        let md = btw_answer(text);
        assert!(md.starts_with("All done.") && md.contains("Tests pass") && !md.contains("```wisp") && !md.contains("\"blocks\""), "{md}");
        assert_eq!(btw_answer("  Yes, both.  "), "Yes, both.");
        assert_eq!(btw_answer(""), "_No answer._");
    }
}
