//! A deleted lead's delegates on other machines (ADR-0060): each one's task there and the lead's copies there go
//! (`machines.uncarry` here, `machines.dropCopies` there). A machine that can't be reached keeps the drop in its
//! row (`data.uncarry`) and the sweep tries again: when it comes online, and every 10 minutes. One the machine answered
//! and refused (a copy it can't remove) `GIVE_UP` times is let go, with a warning and a note on the machine's row
//! (`data.uncarryStuck`): never failed every sweep forever.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use wisp_kernel::WireError;
use wisp_machines_api as api;
use wisp_machines_core::carry as core;

use crate::State;
use crate::carry::on_machine;
use crate::error::{MachinesError, Res};

/// A lead's delegate there, gone: its task (already gone is fine), then every copy of that lead's.
async fn let_go(st: &Arc<State>, id: &str, d: &core::Drop) -> Res<()> {
    if !d.task.is_empty() {
        let args = wisp_runtime_api::commands::IdArgs { id: wisp_runtime_api::SessionId::from(d.task.as_str()) };
        if let Err(e) = on_machine::<wisp_runtime_api::commands::Delete>(st, id, args).await {
            let w = WireError::from(e);
            if w.code != wisp_kernel::Code::NotFound {
                return Err(MachinesError::Wire(w));
            }
        }
    }
    on_machine::<api::DropCopies>(st, id, api::DropCopiesArgs { lead: d.lead.clone() }).await.map(drop)
}

/// The drops machine `id` keeps for the sweep, changed by `f` and saved in its row.
fn keep_drops(st: &Arc<State>, id: &str, f: impl FnOnce(Vec<core::Drop>) -> Vec<core::Drop>) {
    let Ok(mut row) = st.peer(id) else { return };
    let list = f(core::pending(row.data.get(core::UNCARRY)));
    if list.is_empty() {
        if row.data.remove(core::UNCARRY).is_none() {
            return;
        }
    } else {
        row.data.insert(core::UNCARRY.to_owned(), serde_json::to_value(&list).unwrap_or(Value::Null));
    }
    st.save_peer(&row);
    st.publish(id);
}

/// `machines.uncarry`: a lead's delegate on machine `id` goes; one that can't be reached is kept for the sweep.
pub(crate) async fn uncarry(st: &Arc<State>, a: api::UncarryArgs) -> Res<api::Uncarried> {
    st.peer(&a.id)?;
    if a.lead.trim().is_empty() {
        return Err(MachinesError::Wire(WireError::invalid("whose delegate? (no lead)")));
    }
    let d = core::Drop { lead: a.lead, task: a.task, tries: 0 };
    match let_go(st, &a.id, &d).await {
        Ok(()) => {
            keep_drops(st, &a.id, |l| core::without(l, &d));
            Ok(api::Uncarried { done: true })
        }
        Err(e) => {
            tracing::warn!(machine = a.id.as_str(), lead = d.lead.as_str(), "a delegate's copy there, kept for the sweep: {e}");
            keep_drops(st, &a.id, |l| core::with_drop(l, d));
            Ok(api::Uncarried { done: false })
        }
    }
}

/// The sweep (ADR-0060): every drop a machine kept, tried again; `only`: that machine's (it came online).
pub(crate) async fn sweep(st: &Arc<State>, only: Option<&str>) {
    for row in st.peers() {
        if only.is_some_and(|o| o != row.id) {
            continue;
        }
        for d in core::pending(row.data.get(core::UNCARRY)) {
            match let_go(st, &row.id, &d).await {
                Ok(()) => keep_drops(st, &row.id, |l| core::without(l, &d)),
                Err(e) if answered(&e) && online(st, &row.id) => refused(st, &row.id, &d, &e.to_string()),
                // not reached (offline, the link down): it takes the drop once it's back, however long that is
                Err(_) => {}
            }
        }
    }
}

/// The machine itself answered with the refusal (a copy that stayed, a delete it turned down): trying again won't
/// change it by itself. Not reaching it at all isn't one.
fn answered(e: &MachinesError) -> bool {
    matches!(e, MachinesError::Peer { .. } | MachinesError::Rejected { .. })
}

/// Online now: an error from a relay in front of a machine that's asleep isn't the machine's refusal.
fn online(st: &Arc<State>, id: &str) -> bool {
    st.with_link(id, |l| l.conn.status.state == wisp_machines_api::PeerState::Online).unwrap_or(false)
}

/// One more refusal counted in machine `id`'s row; at `GIVE_UP` the drop is let go with a warning and its note.
fn refused(st: &Arc<State>, id: &str, d: &core::Drop, why: &str) {
    let Ok(mut row) = st.peer(id) else { return };
    if core::refused(&mut row.data, d, why) {
        tracing::warn!(
            machine = id,
            lead = d.lead.as_str(),
            task = d.task.as_str(),
            "a delegate's copy there couldn't be removed after {} tries; the sweep let it go (remove it by hand): {why}",
            core::GIVE_UP
        );
    }
    st.save_peer(&row);
    st.publish(id);
}

/// `machines.dropCopies` (on the machine): every copy and upload of lead `lead`'s delegates, under wisp's own folder.
pub(crate) async fn drop_copies(plugin: &Path, lead: &str) -> wisp_kernel::Result<api::DroppedCopies> {
    if lead.trim().is_empty() {
        return Err(WireError::invalid("whose copies? (no lead)"));
    }
    let mut copies = 0;
    let mut kept = Vec::new();
    for (sub, dir) in [(core::COPIES, true), (core::INCOMING, false)] {
        let Ok(mut rd) = tokio::fs::read_dir(plugin.join(sub)).await else { continue };
        while let Ok(Some(e)) = rd.next_entry().await {
            if !e.file_name().to_str().is_some_and(|n| core::of_lead(n, lead)) {
                continue;
            }
            let gone = if dir { tokio::fs::remove_dir_all(e.path()).await } else { tokio::fs::remove_file(e.path()).await };
            match gone {
                Ok(()) if dir => copies += 1,
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => kept.push(format!("{}: {err}", e.file_name().to_string_lossy())),
            }
        }
    }
    // one that stayed (something still wrote in it) is an error: the lead's wisp keeps the drop for the sweep,
    // never "done" with the copy left behind
    if !kept.is_empty() {
        return Err(WireError::unavailable(format!("a copy stayed: {}", kept.join("; "))));
    }
    Ok(api::DroppedCopies { copies })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a machine that answered counts a refusal toward giving up: one offline or out of reach is tried until it's
    /// back.
    #[test]
    fn only_a_refusal_the_machine_answered_counts_toward_giving_up() {
        assert!(answered(&MachinesError::Peer { message: "a copy stayed".into(), status: 503, hold: None }));
        assert!(answered(&MachinesError::Rejected { code: "forbidden".into(), message: "no".into() }));
        assert!(!answered(&MachinesError::Offline("offline".into())));
        assert!(!answered(&MachinesError::Waiting("queued".into())));
        assert!(!answered(&MachinesError::Wire(WireError::unavailable("link down"))));
    }
}
