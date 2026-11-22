//! The server half of sync (plan §4.3–§4.5, §7.2.3), sans-IO: `handle(input, now)` returns what to
//! do and never reads a clock or a socket. One instance owns every stream of this wisp.
//!
//! size-ok: one sans-IO machine; its `handle` arms share private state, and the frame/fold/outbox parts already live in their own files

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use wisp_protocol::{Change, ClientMessage, EntityOp, PROTO, PROTO_MIN, Rows, Snapshot, Topic, Welcome, tag};

use crate::conn::{Conn, Core, Sub, SubState, json};
use crate::outbox::{Front, FrontKind, Outbox, SharedOp};
use crate::parse::{parse_client_message, parse_hello};
use crate::{ChangeRow, CloseReason, CommandOutcome, ConnId, ConnectQuery, Input, Output, SnapshotAnswer, Stats, SyncOptions};

pub struct SyncServer<C> {
    pub(crate) core: Core<C>,
    pub(crate) conns: BTreeMap<ConnId, Conn<C>>,
    by_client: HashMap<String, ConnId>,
}

impl<C> std::fmt::Debug for SyncServer<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncServer").field("head", &self.core.head).field("conns", &self.conns.len()).finish_non_exhaustive()
    }
}

impl<C: Clone> SyncServer<C> {
    pub fn new(opts: SyncOptions<C>) -> Self {
        let (head, oldest) = (opts.head_seq, opts.oldest_seq);
        Self {
            core: Core { opts, head, oldest, req: 0, out: Vec::new(), stats: Stats::default() },
            conns: BTreeMap::new(),
            by_client: HashMap::new(),
        }
    }

    pub fn epoch(&self) -> &str {
        &self.core.opts.epoch
    }

    pub fn head_seq(&self) -> u64 {
        self.core.head
    }

    pub fn stats(&self) -> Stats {
        self.core.stats
    }

    pub fn caller_of(&self, conn: ConnId) -> Option<&C> {
        self.conns.get(&conn).map(|c| &c.caller)
    }

    pub fn conn_of(&self, client_id: &str) -> Option<ConnId> {
        self.by_client.get(client_id).copied()
    }

    /// Streams that count as use of this box: any non-passive subscription.
    pub fn active_count(&self) -> usize {
        self.conns.values().filter(|c| c.active).count()
    }

    /// When `Input::Tick` next has work: a due flush or heartbeat. None when nothing is connected. A
    /// time in the past means now.
    pub fn next_wake_at(&self) -> Option<u64> {
        self.conns.values().map(|c| self.core.wake_at(c)).min()
    }

    pub fn handle(&mut self, input: Input<C>, now: u64) -> Vec<Output<C>> {
        match input {
            Input::Connect { conn, caller, query } => self.connect(conn, caller, &query, now),
            Input::Message { conn, message } => self.message(conn, message, now),
            Input::Committed { rows } => self.committed(rows, now),
            Input::SnapshotRows(a) => self.snapshot_rows(a, now),
            Input::WindowRows { conn, req, rows, done } => self.window_rows(conn, req, rows, done, now),
            Input::CommandResult { conn, mid, outcome } => {
                self.command_result(conn, mid, outcome, now);
            }
            Input::Notice { notice, topic } => {
                let msg = json(&notice);
                for c in self.conns.values_mut().filter(|c| topic.as_ref().is_none_or(|t| c.subs.contains_key(t))) {
                    c.control.push(msg.clone());
                    self.core.maybe_flush(c, now);
                }
            }
            Input::Retained { oldest_seq } => self.core.oldest = self.core.oldest.max(oldest_seq),
            Input::Advance { head } => self.core.head = self.core.head.max(head),
            Input::Tick => self.tick(now),
            Input::Drained { conn, buffered_bytes } => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.buffered = buffered_bytes;
                    self.core.request_waiting(c);
                    self.core.maybe_flush(c, now);
                }
            }
            Input::Close { conn } => self.drop_conn(conn),
        }
        std::mem::take(&mut self.core.out)
    }

    // ---------- connections ----------

    fn connect(&mut self, id: ConnId, caller: C, query: &ConnectQuery, now: u64) {
        let hello = parse_hello(query, &id.to_string());
        if let Some(old) = self.by_client.get(&hello.client_id).copied().filter(|old| *old != id) {
            self.drop_conn(old);
            self.core.out.push(Output::Close { conn: old, reason: CloseReason::Replaced });
        }
        let mut c = Conn {
            id,
            caller,
            client_id: hello.client_id.clone(),
            subs: HashMap::new(),
            out: Outbox::default(),
            control: Vec::new(),
            acks: Vec::new(),
            windows: HashMap::new(),
            buffered: 0,
            last_flush_at: None,
            last_write_at: now,
            active: false,
        };
        let core = &mut self.core;
        let compatible = hello.proto_max.min(PROTO) >= hello.proto_min.max(PROTO_MIN);
        let resume = compatible
            && hello.epoch.as_deref() == Some(core.opts.epoch.as_str())
            && hello.cursor.is_some_and(|x| x >= core.oldest && x <= core.head);
        let proto = if compatible { hello.proto_max.min(PROTO) } else { PROTO };
        let welcome = Welcome {
            t: tag::Welcome,
            epoch: core.opts.epoch.clone(),
            head_seq: core.head,
            oldest_seq: core.oldest,
            resume,
            proto,
            peer: core.opts.peer.clone(),
        };
        if !compatible {
            let w = core.watermark(&c);
            core.write(&mut c, &[json(&welcome)], now, w);
            core.out.push(Output::Close { conn: id, reason: CloseReason::Incompatible });
            return;
        }
        c.control.push(json(&welcome));
        for t in &hello.invalid {
            Core::deny(&mut c, t, "invalid");
        }
        let mark = core.out.len();
        let since = if resume { hello.cursor } else { None };
        for t in &hello.topics {
            core.subscribe(&mut c, t, hello.passive.has(t), since);
        }
        for t in &hello.fresh {
            core.subscribe(&mut c, t, hello.passive.has(t), None);
        }
        core.update_activity(&mut c);
        core.flush(&mut c, now);
        // The welcome goes before the snapshot requests of this connect: a host that answers at once
        // would otherwise send a snapshot the client drops, since nothing counts before the welcome.
        if let Some(pos) = core.out[mark..].iter().position(|o| matches!(o, Output::Send { .. })) {
            core.out[mark..=mark + pos].rotate_right(1);
        }
        c.last_flush_at = None; // ...and it doesn't hold back the snapshots behind it
        self.by_client.insert(hello.client_id, id);
        self.conns.insert(id, c);
    }

    fn drop_conn(&mut self, id: ConnId) {
        if let Some(c) = self.conns.remove(&id)
            && self.by_client.get(&c.client_id) == Some(&id)
        {
            self.by_client.remove(&c.client_id);
        }
    }

    fn message(&mut self, id: ConnId, raw: serde_json::Value, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        let m = match parse_client_message(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(conn = %id, error = %e, "sync: dropped a client message");
                return;
            }
        };
        let core = &mut self.core;
        match m {
            ClientMessage::Sub(s) => {
                core.subscribe(c, &s.topic, s.passive == Some(true), s.since);
                core.update_activity(c);
            }
            ClientMessage::Unsub(u) => {
                c.subs.remove(&u.topic);
                c.out.drop_topic(&u.topic, false);
                core.update_activity(c);
            }
            ClientMessage::Window(w) => {
                if (core.opts.can_read)(&c.caller, &w.topic) {
                    c.windows.insert(w.req.clone(), w.topic.clone());
                    core.out.push(Output::WindowRequest { conn: id, req: w.req, topic: w.topic, before_seq: w.before_seq, limit: w.limit });
                } else {
                    Core::deny(c, w.topic.as_str(), "forbidden");
                }
            }
            ClientMessage::Mutate(m) => {
                let _span = tracing::debug_span!("sync.mutate", conn = %id, cmd = %m.cmd, mid = %m.mid).entered();
                core.out.push(Output::Command {
                    conn: id,
                    caller: c.caller.clone(),
                    mid: m.mid,
                    cmd: m.cmd,
                    args: m.args,
                    base_seq: m.base_seq,
                });
            }
            ClientMessage::Hello(_) => {} // the hello rides on the GET; a second one means nothing
        }
        core.maybe_flush(c, now);
    }

    // ---------- answers from the host ----------

    fn snapshot_rows(&mut self, a: SnapshotAnswer, now: u64) {
        let Some(c) = self.conns.get_mut(&a.conn) else {
            return;
        };
        let Some(sub) = c.subs.get_mut(&a.topic).filter(|s| s.state == SubState::Loading && s.req == a.req) else {
            return;
        };
        let since = sub.since;
        self.core.head = self.core.head.max(a.seq);
        // a snapshot in pages goes page by page as each is read (ADR-0081): the first screen draws from the first
        let paged = a.more || sub.paged;
        let wire_seq = if a.more { *sub.page_seq.get_or_insert(a.seq) } else { sub.page_seq.take().unwrap_or(a.seq) };
        sub.paged = a.more;
        let mut buffered = Vec::new();
        if !a.more {
            sub.state = SubState::Live;
            sub.since = None;
            buffered = std::mem::take(&mut sub.buffer);
            sub.buffer_bytes = 0;
        }
        match since.filter(|_| !a.full) {
            None => {
                let frame = match &a.rows_json {
                    Some(rows) => snapshot_frame(&a.topic, wire_seq, rows, a.more),
                    None => json(&Snapshot {
                        t: tag::Snapshot,
                        topic: a.topic.clone(),
                        seq: wire_seq,
                        rows: a.rows,
                        more: a.more.then_some(true),
                    }),
                };
                // replacing rows the client holds as of `since`: the cursor stays there until this is sent
                c.out.push_front(&a.topic, Front { json: frame, kind: FrontKind::Snapshot, first_seq: since.map(|s| s + 1) });
                if !a.more {
                    for d in a.deleted {
                        let op = Arc::new(SharedOp::new(EntityOp { topic: a.topic.clone(), e: d.e, id: d.id, change: Change::Del }));
                        let bytes = op.json().len();
                        c.out.push_op(op, a.seq, bytes);
                    }
                }
            }
            Some(since) => {
                let first = since + 1;
                // rows the host sent as text for a full answer, taken as a delta after all: read back (never expected)
                let rows = match &a.rows_json {
                    Some(text) => serde_json::from_str(text).unwrap_or_default(),
                    None => a.rows,
                };
                let ops = rows.into_iter().map(|r: wisp_protocol::Row| (r.e, r.id, Change::Upsert { v: r.v }));
                for (e, id, change) in ops.chain(a.deleted.into_iter().map(|d| (d.e, d.id, Change::Del))) {
                    let op = Arc::new(SharedOp::new(EntityOp { topic: a.topic.clone(), e, id, change }));
                    let bytes = op.json().len();
                    c.out.push_op(op, first, bytes);
                }
            }
        }
        for (seq, op) in buffered.into_iter().filter(|(seq, _)| *seq > a.seq) {
            let bytes = op.json().len();
            c.out.push_op(op, seq, bytes);
        }
        if paged {
            self.core.flush(c, now);
        } else {
            self.core.maybe_flush(c, now);
        }
    }

    fn window_rows(&mut self, id: ConnId, req: String, rows: Vec<wisp_protocol::Row>, done: bool, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        let Some(topic) = (if done { c.windows.remove(&req) } else { c.windows.get(&req).cloned() }) else {
            return;
        };
        let msg = Rows { t: tag::Rows, req, rows, done };
        c.out.push_front(&topic, Front { json: json(&msg), kind: FrontKind::Rows, first_seq: None });
        self.core.maybe_flush(c, now);
    }

    fn command_result(&mut self, id: ConnId, mid: String, outcome: CommandOutcome, now: u64) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        match outcome {
            CommandOutcome::Ok { seq, result } => {
                c.acks.push(crate::conn::PendingAck { mid, seq, result });
            }
            CommandOutcome::Err { code, message, reason } => {
                c.control.push(json(&wisp_protocol::Reject { t: tag::Reject, mid, code, message, reason }));
            }
        }
        // the answer to what someone just did goes now, not at the next flush (ADR-0017): the flush still holds an ack
        // until the effects it reports are out; a socket past its high-water mark waits to drain as before (a flush
        // then would shed topics)
        if c.buffered <= self.core.opts.high_water_bytes {
            self.core.flush(c, now);
        } else {
            self.core.maybe_flush(c, now);
        }
    }

    // ---------- the change feed ----------

    fn committed(&mut self, mut rows: Vec<ChangeRow>, now: u64) {
        // Longer-lived topics (higher rank number, e.g. `list` over `session:<id>`) come first at the same seq so
        // non-idempotent ops (`Patch`/`Append`) shared across multiple live subs go once on the longer-lived topic.
        rows.sort_by(|a, b| a.seq.cmp(&b.seq).then_with(|| b.topic.rank().cmp(&a.topic.rank())).then_with(|| a.topic.cmp(&b.topic)));
        let mut live_sent: HashSet<(ConnId, u64, String, String)> = HashSet::new();
        for row in rows {
            let seq = row.seq;
            self.core.head = self.core.head.max(seq);
            let wanted =
                self.conns.values().any(|c| c.subs.get(&row.topic).is_some_and(|s| matches!(s.state, SubState::Live | SubState::Loading)));
            if !wanted {
                continue; // waiting and dropped: the snapshot or delta asked for later covers it
            }
            let dedupe = matches!(row.change, Change::Patch { .. } | Change::Append { .. });
            let topic = row.topic.clone();
            let op = Arc::new(SharedOp::new(EntityOp { topic: row.topic, e: row.entity, id: row.id, change: row.change }));
            let bytes = op.json().len();
            for c in self.conns.values_mut() {
                let Some(sub) = c.subs.get_mut(&topic).filter(|s| matches!(s.state, SubState::Live | SubState::Loading)) else {
                    continue;
                };
                let op = op.clone();
                if sub.state == SubState::Live {
                    if dedupe && !live_sent.insert((c.id, seq, op.op.e.clone(), op.op.id.clone())) {
                        continue;
                    }
                    c.out.push_op(op, seq, bytes);
                    continue;
                }
                sub.buffer.push((seq, op));
                sub.buffer_bytes += bytes;
                // the host is slow to answer: start the topic over rather than hold changes without end
                if sub.buffer_bytes > self.core.opts.load_buffer_bytes {
                    // pages of the snapshot given up on, still queued, would only be started over by the client
                    c.out.drop_topic(&topic, true);
                    let req = self.core.next_req();
                    Core::<C>::start_request(c.id, &topic, sub, req, &mut self.core.out);
                }
            }
        }
        for c in self.conns.values_mut() {
            self.core.maybe_flush(c, now);
        }
    }

    fn tick(&mut self, now: u64) {
        for c in self.conns.values_mut() {
            self.core.request_waiting(c);
            self.core.maybe_flush(c, now);
            self.core.heartbeat(c, now);
        }
    }
}

impl<C> Core<C> {
    pub(crate) fn subscribe(&mut self, c: &mut Conn<C>, topic: &Topic, passive: bool, since: Option<u64>) {
        if !(self.opts.can_read)(&c.caller, topic) {
            return Self::deny(c, topic.as_str(), "forbidden");
        }
        if let Some(existing) = c.subs.get_mut(topic).filter(|s| s.state != SubState::Dropped && since.is_none()) {
            existing.passive = passive; // the same sub again: only its passive flag can change
            return;
        }
        c.out.drop_topic(topic, true);
        // a cursor older than the change log can't be resumed from: take the whole topic
        let from = since.filter(|s| *s >= self.oldest && *s <= self.head);
        let mut sub = Sub {
            passive,
            state: SubState::Waiting,
            since: from,
            req: 0,
            buffer: Vec::new(),
            buffer_bytes: 0,
            paged: false,
            page_seq: None,
        };
        if c.buffered <= self.low_water() {
            let req = self.next_req();
            Self::start_request(c.id, topic, &mut sub, req, &mut self.out);
        }
        c.subs.insert(topic.clone(), sub);
    }

    pub(crate) fn next_req(&mut self) -> u64 {
        self.req += 1;
        self.req
    }

    pub(crate) fn start_request(conn: ConnId, topic: &Topic, sub: &mut Sub, req: u64, out: &mut Vec<Output<C>>) {
        sub.state = SubState::Loading;
        sub.req = req;
        sub.paged = false;
        sub.page_seq = None;
        sub.buffer.clear();
        sub.buffer_bytes = 0;
        out.push(Output::SnapshotRequest { conn, topic: topic.clone(), since: sub.since, req });
    }

    /// Snapshots held back while the connection was over the high-water mark.
    pub(crate) fn request_waiting(&mut self, c: &mut Conn<C>) {
        if c.buffered > self.low_water() {
            return;
        }
        let mut waiting: Vec<_> = c.subs.iter_mut().filter(|(_, s)| s.state == SubState::Waiting).collect();
        waiting.sort_by(|a, b| a.0.rank().cmp(&b.0.rank()).then_with(|| a.0.cmp(b.0)));
        for (topic, sub) in waiting {
            self.req += 1;
            Self::start_request(c.id, topic, sub, self.req, &mut self.out);
        }
    }

    pub(crate) fn update_activity(&mut self, c: &mut Conn<C>) {
        let active = c.subs.values().any(|s| !s.passive);
        if active != c.active {
            c.active = active;
            self.out.push(Output::Activity { conn: c.id, active });
        }
    }
}

/// A `snapshot` frame around rows already encoded, as `json(&Snapshot { .. })` writes it (same fields, same order).
pub fn snapshot_frame(topic: &Topic, seq: u64, rows: &str, more: bool) -> String {
    let topic = json(topic);
    let mut f = String::with_capacity(rows.len() + topic.len() + 64);
    f.push_str("{\"t\":\"snapshot\",\"topic\":");
    f.push_str(&topic);
    f.push_str(",\"seq\":");
    f.push_str(&seq.to_string());
    f.push_str(",\"rows\":");
    f.push_str(rows);
    if more {
        f.push_str(",\"more\":true");
    }
    f.push('}');
    f
}
