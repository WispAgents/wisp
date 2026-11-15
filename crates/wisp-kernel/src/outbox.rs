//! Events that go out with a store write's commit (ADR-0074), for work outside a command (a turn's runner, a
//! plugin's timer): `Ctx::outbox` collects them, the store queues them inside the write's transaction just before
//! its COMMIT (`stage`, held) and lets them go right after it (`Staged::commit`), or drops them when it rolls back.
//! So whoever can read the write's rows finds its events already queued (`Ctx::events_delivered` waits for them),
//! and a write that never commits never publishes.
use std::sync::{Arc, Mutex, PoisonError};

use crate::dispatch::Meta;
use crate::durable::NewEvent;
use crate::events::{Logged, Payload, Pending, Staged};
use crate::host::Ctx;
use crate::{Call, Event};

/// A write's events, collected before the write (`Ctx::outbox`) and handed to the store with it
/// (`wisp_store::Store::write_with`). Dropped without a write, they're never published.
pub struct Outbox {
    ctx: Ctx,
    call: Call,
    events: Mutex<Vec<Pending>>,
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbox").field("events", &self.len()).finish_non_exhaustive()
    }
}

impl Outbox {
    pub(crate) fn new(ctx: Ctx) -> Self {
        let call = ctx.root_call();
        Self { ctx, call, events: Mutex::default() }
    }

    /// Records `e`; it goes out when the write it's handed to commits, after the ones recorded before it.
    pub fn emit<E: Event>(&self, e: &E) {
        let meta = Meta { durable: E::DURABLE, subject: e.subject(), key: e.key() };
        let p = crate::dispatch::pending(&self.ctx, &self.call, E::TYPE, Payload::Typed(Arc::new(e.clone())), meta);
        self.events.lock().unwrap_or_else(PoisonError::into_inner).push(p);
    }

    pub fn len(&self) -> usize {
        self.events.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// For the store, inside the write's transaction: the durable events, in order, to append to the log there.
    pub fn rows(&self) -> Vec<NewEvent> {
        self.events.lock().unwrap_or_else(PoisonError::into_inner).iter().filter(|p| p.durable).map(Pending::row).collect()
    }

    /// For the store, inside the write's transaction after its last op and before COMMIT: queues every event, held.
    /// `cursor`: the change feed's head the write leaves; `seqs`: what the log answered for `rows` (`None`: a repeat
    /// within 24 h, so not published; missing: no log, the event goes out as an ephemeral one). The store settles
    /// what this returns on the writer thread right after COMMIT or ROLLBACK.
    pub fn stage(self, cursor: Option<u64>, seqs: &[Option<u64>]) -> Staged {
        let mut events = self.events.into_inner().unwrap_or_else(PoisonError::into_inner);
        let mut n = 0;
        let mut fresh = Vec::new();
        for p in &mut events {
            if !p.durable {
                continue;
            }
            p.written = match seqs.get(n) {
                Some(Some(s)) => Logged::At(*s),
                Some(None) => Logged::Repeat,
                None => Logged::Not,
            };
            n += 1;
            if let Logged::At(s) = p.written {
                fresh.push((s, p.clone()));
            }
        }
        // a durable repeat (same type and key within 24 h) is the same fact: not published again
        events.retain(|p| p.written != Logged::Repeat);
        let Ok(k) = self.ctx.k() else {
            return Staged::inert();
        };
        let mut staged = k.bus.stage(&k, &self.call.inner.root, cursor, events);
        staged.fresh = fresh;
        staged.kernel = Arc::downgrade(&k);
        staged
    }
}

impl Ctx {
    /// Events that go out with a store write's commit, atomically with its rows becoming visible (ADR-0074): emit on
    /// it, then hand it to `Store::write_with`. For work outside a command (inside one, `emit` on its call already
    /// waits for its commit). Additive (2026-11-15).
    pub fn outbox(&self) -> Outbox {
        Outbox::new(self.clone())
    }
}
