//! A lead's delegates on another wisp (a sandbox, another machine, a crew job there; v1 services/relay.js `watch`):
//! that wisp's runtime doesn't tell this one when their turn ends, so while the lead owes one, wisp asks there
//! (`runtime.get` and `runtime.turns`, through `machines.route`) on the kernel clock, on the plugin's scope, and looks
//! at the lead again (`check_lead`) once it ended. The watch is derived from the store: it lasts while the record is
//! owed (read, sent back, or the lead stopped or deleted ends it) and is armed again as the lead is looked at after a
//! restart (`start` → `check_lead` → Hold). One per lead and job.

use std::sync::{Arc, PoisonError};

use serde_json::json;
use wisp_runtime_core::owed;

use super::report_back::OwedRec;
use super::str_at;
use crate::runtime::Runtime;

/// How often a job elsewhere is asked about (v1 long-polled it in 5 s steps).
const FAR_POLL_MS: u64 = 1000;
/// The longest wait between asks: its wisp doesn't answer, or it ended and the lead's own run is on (its end collects it).
const FAR_BACKOFF_MS: u64 = 10_000;

impl Runtime {
    /// Watches the far ones of `list` still at work, for lead `lead` (one watch per job; a second ask is a no-op).
    pub(crate) fn watch_far(self: &Arc<Self>, lead: &str, list: &[OwedRec]) {
        for key in list.iter().filter(|d| d.is_far_at_work()).map(OwedRec::key) {
            let mark = format!("{lead} {key}");
            if !self.far_watch.lock().unwrap_or_else(PoisonError::into_inner).insert(mark.clone()) {
                continue;
            }
            let (rt, lead) = (self.clone(), lead.to_owned());
            self.host.spawn(Box::pin(async move {
                rt.far_watch_loop(&lead, &key).await;
                rt.far_watch.lock().unwrap_or_else(PoisonError::into_inner).remove(&mark);
            }));
        }
    }

    /// Until the lead no longer owes job `key`: each step, how it stands there; ended, the lead is looked at again (a
    /// lead whose run is on is left to its end, which collects it; the watch goes on until that's written).
    async fn far_watch_loop(self: &Arc<Self>, lead: &str, key: &str) {
        let mut wait = FAR_POLL_MS;
        loop {
            self.clock.sleep(wait).await;
            let Ok(Some(t)) = self.owed_turn(lead, None).await else { return };
            let Some(rec) = super::report_back::records(&t).iter().find(|d| owed::is_owed(d) && str_at(d, "sessionId") == key).cloned()
            else {
                return;
            };
            let far = self.far_of(&rec).await;
            wait = if far.is_some() { FAR_POLL_MS } else { (wait * 2).min(FAR_BACKOFF_MS) };
            if self.owed_on_one(&rec, far).await.is_none_or(|d| d.running()) {
                continue;
            }
            // ended: a lead whose run is on is left to its end, asked again only now and then until that's written
            wait = FAR_BACKOFF_MS;
            let busy = self.run_state(lead) != wisp_runtime_core::queue::Run::Idle || self.claims.count(lead) > 0;
            if !busy && let Err(err) = self.check_lead(lead).await {
                tracing::warn!(task = %lead, %err, "runtime: a lead's delegate elsewhere ended, looked at again");
            }
        }
    }

    /// Stop on the lead reaches its delegates elsewhere still at work (v1: the relay stops the run there):
    /// `runtime.stop` on their wisp, off the caller's way. One that can't be reached is let go (it's owed nothing now).
    pub(crate) fn stop_far(self: &Arc<Self>, keys: &[String]) {
        for key in keys {
            let Some((machine, id)) = key.split_once('|') else { continue };
            let (rt, machine, id) = (self.clone(), machine.to_owned(), id.to_owned());
            self.host.spawn(Box::pin(async move {
                if let Err(err) = rt.route(&machine, "runtime.stop", json!({ "id": id })).await {
                    tracing::warn!(error = %err.message, "runtime: a stopped lead's delegate elsewhere");
                }
            }));
        }
    }
}
