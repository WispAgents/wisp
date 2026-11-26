//! A clipped step's whole output (audit F8): a step keeps an output's start and end (`activity::clip_output`), and
//! its whole text, when that was cut, goes once into the store's blobs, held by its task (`FULL_OWNER`, let go when the
//! task is deleted). `runtime.fullOutput` reads it back for the opened step's "Show all N lines", only when asked.
//! Masked as the step is (ADR-0050): an output that read or quoted a .env value never has one (the turn machine names
//! none), and what the task read since is hidden in it unless Hide .env values is off.

use wisp_kernel::{Setup, WireError};
use wisp_runtime_api::commands as c;
use wisp_runtime_api::model::{Activity, FullOutput};

use crate::env_view::on;
use crate::plugin::{Slot, rt};

/// Who holds a task's whole outputs in the store's blobs: the task (its id).
pub(crate) const FULL_OWNER: &str = "runtime.session";

/// The step's whole output, if it names one, and whether it is an error (masked as one).
fn full_of(a: &Activity) -> Option<(&FullOutput, bool)> {
    match a {
        Activity::Tool { full: Some(f), .. } | Activity::ToolOutput { full: Some(f), env: false, .. } => Some((f, false)),
        Activity::ToolError { full: Some(f), .. } => Some((f, true)),
        _ => None,
    }
}

/// `text` with what quotes one of `known` as `••••`: an error's quotes as its step's are, an output's values.
#[must_use]
pub(crate) fn masked(text: &str, error: bool, known: &[String]) -> String {
    let spans = if error { wisp_envmask::quote_spans(text, known) } else { wisp_envmask::value_spans(text, known) };
    if spans.is_empty() { text.to_owned() } else { wisp_envmask::mask_spans(text, &spans) }
}

pub(crate) fn register(s: &mut Setup, slot: &Slot) {
    let sl = slot.clone();
    s.command::<c::FullOutput, _, _>(move |ctx, _call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            let gone = || WireError::not_found("That output is no longer kept.");
            let turn = r.repo.turn(a.turn_id.as_str()).await.map_err(|_| gone())?;
            if turn.session_id != a.id {
                return Err(gone()); // a turn of another task: the capability was checked against this one
            }
            let page = r.activity(a.turn_id.as_str(), a.step.checked_sub(1)).await.map_err(WireError::from)?;
            let step = page.items.into_iter().find(|s| s.seq == a.step).ok_or_else(gone)?;
            let (full, error) = full_of(&step.activity).ok_or_else(gone)?;
            let hash = full.blob.clone();
            let bytes = r.repo.store.read_async(move |rd| rd.blob(&hash)).await.map_err(|e| WireError::internal(e.to_string()))?;
            let text = String::from_utf8(bytes.ok_or_else(gone)?).map_err(|_| gone())?;
            if !on(&ctx) {
                return Ok(c::FullOutputText { text });
            }
            // fail closed: what the task read can't be told, so neither is this
            let known = r.env_known(a.id.as_str()).await.map_err(|_| WireError::unavailable("Couldn't hide this task's .env values."))?;
            Ok(c::FullOutputText { text: masked(&text, error, &known.0) })
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_the_task_read_since_is_hidden_in_the_whole_output() {
        let known = vec!["sk-live-one".to_owned()];
        assert_eq!(masked("ok\nkey sk-live-one\nend", false, &known), format!("ok\nkey {}\nend", wisp_envmask::DOTS));
        assert_eq!(masked("nothing here", false, &known), "nothing here");
    }

    #[tokio::test]
    async fn a_clipped_outputs_whole_text_round_trips_through_the_store_by_the_hash_its_step_names() {
        use std::sync::Arc;
        let store = wisp_store::Store::open(wisp_store::Location::Memory, Arc::new(wisp_store::SystemClock)).unwrap();
        let whole = format!("HEAD\n{}TAIL", "a line of a build\n".repeat(400));
        let full = wisp_runtime_core::activity::full_of(&whole).unwrap();
        let info = store.put_blob(whole.clone().into_bytes(), Some((FULL_OWNER.to_owned(), "s1".to_owned()))).await.unwrap();
        assert_eq!(info.hash, full.blob, "the step names the blob the host keeps");
        assert_eq!(store.blob(&full.blob).unwrap().as_deref(), Some(whole.as_bytes()));
        // held by its task: deleting it lets the blob go
        store.write(|w| w.blob_unref_owner(FULL_OWNER, "s1")).await.unwrap();
        store.gc_blobs().await.unwrap();
        assert_eq!(store.blob(&full.blob).unwrap(), None);
    }
}
