//! A side discussion's runs (v1 side.js, runner.js #startTurn): a task of its own about another (`sideOf`), working
//! in that main task's folder as it is now. Each run hears the side rules in its system prompt (v1 `sideNote`) and,
//! before each of the user's messages, where the main task stands now (v1 `sidePrompt`). Nothing of it goes into the
//! main task's conversation: it resumes its own (orch #5, tasks #6).

use wisp_runtime_api::harness::{HarnessInfo, HarnessRun};
use wisp_runtime_api::model::{Mode, Session};
use wisp_runtime_core::prompt::handoff::{PastTurn, Render};
use wisp_runtime_core::prompt::side::{MainTask, SideOf, side_note, side_prompt};

use crate::runtime::Runtime;

impl Runtime {
    /// v1 `sideMode`: Edits behind wisp's gate where the harness can ask before a write, else Plan.
    pub(crate) fn side_mode(&self, info: &HarnessInfo) -> Mode {
        if info.side_gate && self.host.agent_access().is_some() { Mode::Edits } else { Mode::Plan }
    }

    /// The side rules for the system prompt ('' for a task that isn't a side discussion).
    pub(crate) fn side_system(&self, s: &Session, info: &HarnessInfo) -> String {
        let Some(of) = s.ext.get("wisp.runtime").and_then(|o| o.get("sideOf")) else { return String::new() };
        let id = of.get("id").and_then(serde_json::Value::as_str).unwrap_or_default();
        if id.is_empty() {
            return String::new();
        }
        let title = of.get("title").and_then(serde_json::Value::as_str).unwrap_or_default();
        let tool = match (&info.ask_tool, self.host.agent_access()) {
            (Some(a), Some(_)) => {
                a.rsplit_once("ask_user").map_or_else(|| "wisp_session".to_owned(), |(pre, _)| format!("{pre}wisp_session"))
            }
            _ => String::new(),
        };
        side_note(SideOf { id, title }, &tool, self.side_mode(info) == Mode::Edits, &info.name)
    }

    /// The run of a side discussion as it goes: in the main task's folder now (it may have moved to a worktree since),
    /// in its side mode, every write through the gate in Edits, and the user's words after the main task's state.
    pub(crate) async fn side_discussion_run(&self, s: &Session, info: &HarnessInfo, run: &mut HarnessRun, said: &str) {
        let Some(main_id) = crate::run::side_main(s) else { return };
        let main = self.repo.session(main_id).await.ok();
        if let Some(m) = &main
            && !m.cwd.is_empty()
        {
            run.cwd.clone_from(&m.cwd);
            run.dirs.clone_from(&m.dirs);
        }
        run.mode = self.side_mode(info);
        run.gate = run.mode == Mode::Edits;
        let render = |blocks: &[serde_json::Value], _t: &wisp_runtime_api::model::Turn| {
            wisp_runtime_core::blocks::blocks_to_markdown(
                blocks,
                &wisp_runtime_core::blocks::render::Context { changes: &[], approval: None, summary: None },
            )
        };
        let wrapped = match &main {
            Some(m) => {
                let mut turns = self.repo.turns(main_id, None, None).await.unwrap_or_default();
                turns.sort_by_key(|t| t.idx);
                let known = self.env_known(main_id).await.ok();
                let env: &[String] = known.as_ref().map_or(&[], |k| k.0.as_slice());
                let name = |id: &str| self.harness(id).map_or_else(|_| id.to_owned(), |h| h.info().name.clone());
                let past: Vec<PastTurn<'_>> = turns
                    .iter()
                    .map(|t| PastTurn { env, ..PastTurn::new(t, name(if t.agent.is_empty() { &m.agent } else { &t.agent })) })
                    .collect();
                side_prompt(Some(MainTask { title: &m.title, turns: &past }), said, self.now(), Render(&render))
            }
            None => side_prompt(None, said, self.now(), Render(&render)),
        };
        // the user's words were the run's prompt: the main task's state goes ahead of them
        if let Some(at) = run.prompt.rfind(said).filter(|_| !said.is_empty()) {
            run.prompt.replace_range(at..at + said.len(), &wrapped);
        } else {
            run.prompt = wrapped;
        }
    }
}
