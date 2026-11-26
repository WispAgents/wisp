//! `runtime.after@1` (`GoalHold`): a turn that ends done on a task whose goal is still being worked on holds the task
//! in Working while its judges vote, as land.rs says a goal round does. Before, the round was judged from `turn.ended`
//! after the turn had landed: the task sat in Needs review, unread, with a notification, while its judges still voted,
//! and every round said done. Now met lands it once (one `turn.finished`), another round's follow-up starts the next
//! turn without it ever passing through Review, and a stall asks you (Waiting).

use serde_json::Value;
use wisp_kernel::{BoxFuture, Ctx, Result};
use wisp_runtime_api::model::{Session, Turn, TurnStatus};
use wisp_runtime_api::points::{AfterTurn, TurnEnd};

use crate::calls::{spec_of, state_of};
use crate::inflight::Inflight;
use crate::judging::{Holds, held_round, judged, judging_now};
use crate::{goal_with, kinds};

pub struct GoalHold {
    pub(crate) ctx: Ctx,
    pub(crate) defaults: kinds::Defaults,
    pub(crate) live: Inflight,
    pub(crate) hold: Holds,
}

/// Whether this turn's end is a goal round to hold the task for: it ended done (not stopped, not failed), it isn't an
/// inner run (a subagent's, wisp's own) unless the task is a child with a goal of its own (a workflow's step), and the
/// task's goal is judged and still being worked on (not met, paused or cleared, no round already under way).
pub fn wants_round(session: &Session, turn: &Turn, end: TurnEnd) -> bool {
    if end.stopped || end.is_error || turn.status != TurnStatus::Done {
        return false;
    }
    let spec: Value = spec_of(session);
    let own = judged(session, &spec);
    let inner = session.parent_id.is_some() || matches!(turn.by.as_deref(), Some("wisp" | "compact"));
    if !own || (inner && session.parent_id.is_none()) {
        return false;
    }
    let st = state_of(session).unwrap_or_default();
    goal_with(&spec, Some(&st), &kinds::GoalSpec::default()).is_some()
        && !matches!(st.state.as_str(), "met" | "paused" | "cleared")
        && !judging_now(&st)
}

impl AfterTurn for GoalHold {
    fn id(&self) -> &'static str {
        "wisp.goal.round"
    }
    fn order(&self) -> i32 {
        10
    }
    fn wants(&self, session: &Session, turn: &Turn, end: TurnEnd) -> bool {
        // the round itself landing the task (`runtime.settle` asks again): never taken back
        !self.hold.lands(session.id.as_str()) && wants_round(session, turn, end)
    }
    fn run(&self, session: Session, turn: Turn) -> BoxFuture<'static, Result<()>> {
        let (ctx, d, live, hold) = (self.ctx.clone(), self.defaults.clone(), self.live.clone(), self.hold.clone());
        Box::pin(async move {
            held_round(&ctx, &d, &live, &hold, session.id.as_str(), turn.id.as_str()).await;
            Ok(())
        })
    }
    fn cancel(&self, session: &Session, stop: bool, _by: Option<&str>) -> bool {
        // a follow-up meanwhile: the round goes on (its verdict follows that turn); you stopping the task halts it,
        // its judges stopped
        let id = session.id.as_str();
        if !stop || !self.hold.holds(id) {
            return false;
        }
        self.live.halt(id);
        let (ctx, live, id) = (self.ctx.clone(), self.live.clone(), id.to_owned());
        self.ctx.spawn(async move {
            live.stop(&ctx, &id, None).await;
        });
        true
    }
}
