//! Silent runs (TRY-23, `wisp_budget_core::silent`): each running turn's last sign of life, from its steps; once a
//! minute, a run silent 30 minutes while its plan still moves is told to the phone with Stop (`channels.send` about
//! the task), and with *Stop a run silent for* on (`budget.silentStop`, off by default) one silent that long is
//! stopped. Only wisp sees a run's output and its plan together, for every harness.

use serde_json::Value;
use wisp_budget_core::silent::{Due, Quiet, Sign, due, stop_after_ms, stopped_text, told_text};
use wisp_budget_core::windows::window_label;
use wisp_channels_api::views::{Action, ActionKind, Msg};
use wisp_kernel::Ctx;
use wisp_runtime_api::model::{Activity, Step, ToolKind};

use crate::{State, account_of, lock, now};

/// The setting (manifest): "off", or the hours.
pub const STOP: &str = "budget.silentStop";

/// What a step says of its turn.
fn sign(s: &Step) -> Sign {
    match &s.activity {
        Activity::Tool { tool: ToolKind::Agent, call_id: Some(id), output: None, .. } => Sign::Opened(id.clone()),
        Activity::Tool { call_id: Some(id), output: Some(_), .. } | Activity::ToolOutput { call_id: id, .. } => Sign::Closed(id.clone()),
        _ => Sign::Other,
    }
}

impl State {
    /// The account's longest window as it stands: its label and how much is used (0..1).
    fn longest(&self, agent: &str, account: &str) -> Option<(String, f64)> {
        let b = lock(&self.budget);
        let acc = b.get(agent, &account_of(account))?;
        acc.windows.iter().max_by_key(|(_, w)| w.ms.unwrap_or(0)).map(|(id, w)| (window_label(id, w.ms), w.used))
    }

    /// A turn started on `agent@account`: it's watched from now.
    pub(crate) fn quiet_begin(&self, sid: &str, tid: &str, agent: &str, account: &str, at: i64) {
        let used = self.longest(agent, account).map(|(_, u)| u);
        lock(&self.quiet).insert(sid.to_owned(), Quiet::new(agent, account, tid, at, used));
    }

    /// Its steps: a sign of life.
    pub(crate) fn quiet_steps(&self, sid: &str, steps: &[Step], at: i64) {
        let Some((agent, account)) = lock(&self.quiet).get(sid).map(|q| (q.agent.clone(), q.account.clone())) else { return };
        let used = self.longest(&agent, &account).map(|(_, u)| u);
        let signs: Vec<Sign> = steps.iter().map(sign).collect();
        if let Some(q) = lock(&self.quiet).get_mut(sid) {
            q.steps(&signs, at, used);
        }
    }

    pub(crate) fn quiet_end(&self, sid: &str) {
        lock(&self.quiet).remove(sid);
    }

    /// One look: tell or stop the runs that are due.
    pub(crate) async fn silent(&self, ctx: &Ctx) {
        let at = now(ctx);
        let stop_after = stop_after_ms(ctx.setting(STOP).as_ref().and_then(Value::as_str));
        let runs: Vec<(String, Quiet)> = lock(&self.quiet).iter().map(|(s, q)| (s.clone(), q.clone())).collect();
        for (sid, q) in runs {
            let longest = self.longest(&q.agent, &q.account);
            match due(&q, at, longest.as_ref().map(|(_, u)| *u), stop_after) {
                Due::Nothing => {}
                Due::Tell { minutes, spent } => {
                    if let Some(x) = lock(&self.quiet).get_mut(&sid) {
                        x.told = true;
                    }
                    let window = longest.map(|(l, _)| l).unwrap_or_default();
                    let stop = Action { label: "Stop".into(), kind: ActionKind::Stop, reference: q.turn_id.clone(), index: 0 };
                    tell(ctx, &sid, told_text(minutes, spent, &window), vec![stop]).await;
                }
                Due::Stop { minutes } => {
                    if let Some(x) = lock(&self.quiet).get_mut(&sid) {
                        x.stopped = true;
                    }
                    let args = wisp_runtime_api::commands::StopArgs { id: sid.as_str().into(), clear_queue: false };
                    if let Err(err) = ctx.run::<wisp_runtime_api::commands::Stop>(&ctx.root_call(), args).await {
                        tracing::warn!(error = %err.message, session = %sid, "budget: stopping a silent run");
                        continue;
                    }
                    tell(ctx, &sid, stopped_text(minutes), Vec::new()).await;
                }
            }
        }
    }
}

/// To the phone, about the task (its title is the message's).
async fn tell(ctx: &Ctx, sid: &str, text: String, actions: Vec<Action>) {
    let title = ctx
        .run::<wisp_runtime_api::commands::Get>(&ctx.root_call(), wisp_runtime_api::commands::IdArgs { id: sid.into() })
        .await
        .ok()
        .map(|t| t.title)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "A task".into());
    let msg = Msg { title, text, actions };
    let args = wisp_channels_api::SendArgs { msg, session_id: Some(sid.to_owned()) };
    if let Err(err) = ctx.run::<wisp_channels_api::SendNotice>(&ctx.root_call(), args).await {
        tracing::debug!(error = %err.message, "budget: no phone to tell of a silent run");
    }
}
