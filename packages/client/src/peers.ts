// The peer manager (plan §7.2.1, §7.3.1): N peers at once, each with its own transport, sync link
// (epoch + cursor), status and subscriptions, merged into one entity store. It is the host of the
// sans-IO link: it opens the streams the link asks for, feeds it frames, and carries out what it
// returns. Subscriptions belong to the asking scope: two components showing one task share one
// subscription (the link refcounts), and the last to go ends it. A sleeping box stays asleep
// until something wakes it: opening its task or sending it a command (plan §7.2.3).
// size-ok: one factory whose closures share each peer's runtime; the watch refcount (perf r2 #14) sits with watch()

import type { Ack, ClientMessage, Notice, PeerInfo, Reject, Topic } from '@wisp/protocol';
import type { Devtools } from './devtools.ts';
import type { EntityWriter } from './entities.ts';
import { hasFeature, updateCommand, updateReason, type FeatureAnswer, type FeatureNeed } from './features.ts';
import { syncLink, type ClientInput, type ClientOutput, type LinkSaved, type PeerLink } from './peer.ts';
import type { Scope } from './scope.ts';
import { batch, signal, untracked, type Readable, type WritableSignal } from './signals.ts';
import { BACKOFF_MS } from '@wisp/sync';
import { PeerDown, type Frame, type PeerHttp, type Transport } from './transport.ts';
import type { PeerId, Warn } from './types.ts';

export type PeerStatus =
  | { kind: 'connecting' }
  | { kind: 'online' }
  | { kind: 'asleep' }
  | { kind: 'waking' }
  /** `heardAt`: when it was last heard from, this run or a saved one (the offline bar's *last updated*). Additive. */
  | { kind: 'unreachable'; reason: string; heardAt?: number }
  /** `command`: what updates the side that's too old, when it is the peer (`npm install -g wisp-agents`). Additive. */
  | { kind: 'incompatible'; reason: string; update: 'peer' | 'app'; command?: string };

export interface PeerConfig {
  peer: PeerId;
  /** what this device calls it (shown over the peer's own name); empty: the name the peer gives itself */
  name: string;
  transport: Transport;
  /** the link state saved by an earlier run (first paint) */
  saved?: LinkSaved;
  /** another link implementation; @wisp/sync's SyncClient by default */
  link?: PeerLink;
}

export type Answer = Ack | Reject;
export type RowsAnswer = Extract<ClientOutput, { kind: 'rows' }>;

export interface Peers {
  add(cfg: PeerConfig): void;
  remove(peer: PeerId): void;
  list(): Readable<readonly PeerId[]>;
  status(peer: PeerId): Readable<PeerStatus>;
  info(peer: PeerId): Readable<PeerInfo | undefined>;
  name(peer: PeerId): string;
  hasFeature(peer: PeerId, need: FeatureNeed): FeatureAnswer;
  /** keeps `topic` of `peer` live while `scope` lives; a passive watch never wakes a box */
  watch(peer: PeerId, topic: Topic, scope: Scope, opts?: { passive?: boolean }): void;
  wake(peer: PeerId): Promise<void>;
  /** opens `peer`'s stream again now (every peer when omitted), from its cursor, skipping the backoff: pull to
   *  refresh, the me menu's *Reconnect*, the offline bar's *Retry*. A sleeping box stays asleep. Additive. */
  reconnect(peer?: PeerId): void;
  /** app resumed or network/tunnel returned: reopen offline or stale streams without disturbing healthy ones. Additive. */
  resume(peer?: PeerId): void;
  /** hands a command to the peer's link: sent now if live, else after the next welcome */
  mutate(peer: PeerId, m: { mid: string; cmd: string; args: unknown }): boolean;
  /** asks for older rows of `topic` (a transcript page); false when the peer isn't online */
  window(peer: PeerId, w: { req: string; topic: Topic; beforeSeq: number; limit: number }): boolean;
  save(peer: PeerId): LinkSaved | undefined;
  /** the peer's own routes (raw bodies, event streams), through its transport */
  http(peer: PeerId): PeerHttp;
  onLive(fn: (peer: PeerId) => void): () => void;
  onRemoved?(fn: (peer: PeerId) => void): () => void;
  onSent(fn: (peer: PeerId, mid: string) => void): () => void;
  onAnswer(fn: (peer: PeerId, a: Answer) => void): () => void;
  onNotice(fn: (peer: PeerId, n: Notice) => void): () => void;
  onRows(fn: (peer: PeerId, r: RowsAnswer) => void): () => void;
}

export interface PeersOptions {
  scope: Scope;
  writer: EntityWriter;
  clientId: string;
  build: string;
  warn?: Warn;
  devtools?: Devtools;
}

/** how often an open stream's silence is checked: its link drops one quiet for two keepalives */
const WATCH_MS = 5_000;
/** how long a topic nothing watches keeps its rows: opened again by then (Back, J/K and back) it draws at once */
export const KEEP_UNWATCHED_MS = 30_000;

interface PeerRuntime {
  readonly cfg: PeerConfig;
  readonly link: PeerLink;
  readonly scope: Scope;
  readonly status: WritableSignal<PeerStatus>;
  readonly info: WritableSignal<PeerInfo | undefined>;
  conn?: Scope;
  cancelRetry?: () => void;
  /** FIFO queue for POST /api/events so unsub+sub never race out of order on the wire; `posting` while one is in flight */
  postQueue: { conn: Scope | undefined; msg: ClientMessage }[];
  posting: boolean;
  asleep: boolean;
  waking: boolean;
  lastError: string;
  /** POSTs that failed since the last one that got through: past the first, the reopen waits out BACKOFF_MS */
  postFails: number;
  cancelReopen?: () => void;
}

const listen = <F>(set: Set<F>, fn: F): (() => void) => {
  set.add(fn);
  return () => set.delete(fn);
};
const message = (err: unknown): string => (err instanceof Error ? err.message : String(err));

// size-ok: a factory; its body is small closures over shared private state, each under the cap
export function createPeers(o: PeersOptions): Peers {
  const warn = o.warn ?? (() => {});
  const now = (): number => o.scope.timers.now();
  const peers = new Map<PeerId, PeerRuntime>();
  /** watches on a peer not added yet, started when it is */
  const waiting = new Map<PeerId, { topic: Topic; scope: Scope; opts: { passive?: boolean } }[]>();
  /** mutations on a peer not added yet (e.g. restored before extraPeers/machines load), handed over on add() */
  const waitingMutates = new Map<PeerId, { mid: string; cmd: string; args: unknown }[]>();
  const ids = signal<readonly PeerId[]>([]);
  // only what's open stays in memory (AGENTS.md): a topic's rows go a while after its last watch ends (perf r2 #14: a
  // task's turns and events stayed for good once it had been opened, every streamed event of a task looked at once)
  const watching = new Map<string, { n: number; stop?: () => void }>();
  const held = (peer: PeerId, topic: Topic, by: 1 | -1): void => {
    const k = `${peer}\u0000${topic}`;
    const w = watching.get(k) ?? { n: 0 };
    w.n += by;
    w.stop?.();
    delete w.stop;
    watching.set(k, w);
    if (w.n > 0) return;
    w.stop = o.scope.timeout(KEEP_UNWATCHED_MS, () => {
      if ((watching.get(k)?.n ?? 0) > 0) return;
      watching.delete(k);
      o.writer.dropTopic(peer, topic);
      const p = peers.get(peer);
      if (p) feed(p, { kind: 'evicted', topic });
    });
  };
  const statusOf = new Map<PeerId, WritableSignal<PeerStatus>>();
  const infoOf = new Map<PeerId, WritableSignal<PeerInfo | undefined>>();
  const on = {
    live: new Set<(p: PeerId) => void>(),
    removed: new Set<(p: PeerId) => void>(),
    sent: new Set<(p: PeerId, mid: string) => void>(),
    answer: new Set<(p: PeerId, a: Answer) => void>(),
    notice: new Set<(p: PeerId, n: Notice) => void>(),
    rows: new Set<(p: PeerId, r: RowsAnswer) => void>(),
  };

  const lazy = <V>(m: Map<PeerId, WritableSignal<V>>, peer: PeerId, init: V): WritableSignal<V> => {
    let s = m.get(peer);
    if (!s) m.set(peer, (s = signal<V>(init)));
    return s;
  };
  const statusSig = (peer: PeerId): WritableSignal<PeerStatus> => lazy(statusOf, peer, { kind: 'unreachable', reason: 'not added' } as PeerStatus);
  const infoSig = (peer: PeerId): WritableSignal<PeerInfo | undefined> => lazy(infoOf, peer, undefined as PeerInfo | undefined);

  const feed = (p: PeerRuntime, input: ClientInput): void => {
    if (p.scope.alive) for (const out of p.link.handle(input, now())) carry(p, out);
  };

  /** an op the store couldn't apply: the link starts that topic over (unsub, then a fresh sub) */
  const drifted = (p: PeerRuntime, topic: Topic): void => {
    o.devtools?.record({ kind: 'frame', peer: p.cfg.peer, t: `drifted ${topic}` });
    feed(p, { kind: 'drifted', topic });
  };

  function setState(p: PeerRuntime, out: Extract<ClientOutput, { kind: 'state' }>): void {
    switch (out.state) {
      case 'idle':
        return;
      case 'connecting':
        return p.status(p.waking ? { kind: 'waking' } : { kind: 'connecting' });
      case 'live':
        p.waking = false;
        p.status({ kind: 'online' });
        for (const fn of on.live) fn(p.cfg.peer);
        return;
      case 'offline': {
        const heardAt = p.link.heardAt;
        return p.status(p.asleep ? { kind: 'asleep' } : { kind: 'unreachable', reason: p.lastError, ...(heardAt !== undefined && { heardAt }) });
      }
      case 'incompatible': {
        const update = out.update ?? 'peer';
        const name = p.cfg.name || (untracked(p.info)?.name ?? '');
        p.conn?.dispose();
        return p.status({ kind: 'incompatible', reason: updateReason(update, name), update: update === 'client' ? 'app' : 'peer', ...(update === 'peer' && { command: updateCommand() }) });
      }
    }
  }

  function carry(p: PeerRuntime, out: ClientOutput): void {
    const peer = p.cfg.peer;
    switch (out.kind) {
      case 'request':
        return void connect(p, out.params);
      case 'send': {
        if (out.message.t === 'mutate') for (const fn of on.sent) fn(peer, out.message.mid);
        // Serialize POST /api/events per peer so back-to-back unsub+sub never arrive out of order. An idle peer posts
        // synchronously (callers and tests see the POST start at once); one with a POST in flight queues behind it.
        p.postQueue.push({ conn: p.conn, msg: out.message });
        drainPosts(p);
        return; // a lost mutate is resent after the next welcome, with the same mid
      }
      case 'reset':
        return o.writer.resetPeer(peer);
      case 'snapshot':
        return o.writer.snapshot(peer, out.topic, out.rows);
      case 'snapshotPage':
        return o.writer.snapshotPage(peer, out.topic, out.rows);
      case 'entityOps':
        for (const topic of o.writer.applyServer(peer, out.ops)) drifted(p, topic);
        return;
      case 'ack':
        for (const fn of on.answer) fn(peer, { t: 'ack', mid: out.mid, seq: out.seq, ...(out.result !== undefined && { result: out.result }) });
        return;
      case 'reject':
        for (const fn of on.answer) fn(peer, { t: 'reject', mid: out.mid, code: out.code, message: out.message, ...(out.reason !== undefined && { reason: out.reason }) });
        return;
      case 'rows':
        for (const fn of on.rows) fn(peer, out);
        return;
      case 'denied':
        for (const fn of on.notice) fn(peer, { t: 'notice', kind: 'denied', data: { topic: out.topic, code: out.code } });
        return;
      case 'notice':
        for (const fn of on.notice) fn(peer, out.notice);
        return;
      case 'needResubscribe':
        return o.devtools?.record({ kind: 'frame', peer, t: `resync ${out.topic}` });
      case 'state':
        return setState(p, out);
      case 'retry':
        if (p.asleep) return; // sleeping boxes stay asleep
        p.cancelRetry?.();
        p.cancelRetry = p.scope.timeout(Math.max(0, out.at - now()), () => feed(p, { kind: 'open' }));
        return;
      case 'drop':
        // silent too long, or reopened: this stream ends here; the link has counted it lost already
        p.lastError = 'no answer';
        p.conn?.dispose();
        delete p.conn;
        return;
    }
  }

  /** a POST that went through: the failure count starts over and a pending backoff reopen is called off */
  function onPostOk(p: PeerRuntime): void {
    p.postFails = 0;
    p.cancelReopen?.();
  }

  /** A POST that failed while its stream is still the open one (HTTP 404, a broken socket, a refused write). */
  function onPostError(p: PeerRuntime, conn: Scope | undefined, msg: ClientMessage, err: unknown): void {
    if (!p.scope.alive || (conn && (!conn.alive || p.conn !== conn))) return;
    warn(`${msg.t} to ${p.cfg.peer} not sent`, message(err));
    if (untracked(p.status).kind !== 'online') return;
    // The first failure reopens now (a stream the peer already dropped). One that keeps failing after the
    // reopen (a peer that refuses the write: the grid's box answered the phone's POSTs 403) waits out the
    // backoff with the stream left open, so the computer doesn't flip online/offline several times a second.
    const fails = ++p.postFails;
    if (fails === 1) return feed(p, { kind: 'reopen' });
    if (p.cancelReopen) return; // one is already on its way
    const wait = BACKOFF_MS[Math.min(fails - 1, BACKOFF_MS.length) - 1] ?? 0;
    const cancel = p.scope.timeout(wait, () => {
      delete p.cancelReopen;
      if (untracked(p.status).kind === 'online') feed(p, { kind: 'reopen' });
    });
    p.cancelReopen = () => {
      cancel();
      delete p.cancelReopen;
    };
  }

  /** Posts the queue in order, one at a time: the first synchronously, each next one when the previous settles. */
  function drainPosts(p: PeerRuntime): void {
    if (p.posting) return;
    p.posting = true;
    const step = (): void => {
      for (let next = p.postQueue.shift(); next; next = p.postQueue.shift()) {
        const { conn, msg } = next;
        if (!p.scope.alive || (conn && (!conn.alive || p.conn !== conn))) continue;
        let res: Promise<void>;
        try {
          res = p.cfg.transport.post(msg, conn?.signal ?? p.scope.signal);
        } catch (err: unknown) {
          onPostError(p, conn, msg, err);
          continue;
        }
        void res.then(
          () => {
            onPostOk(p);
            step();
          },
          (err: unknown) => {
            onPostError(p, conn, msg, err);
            step();
          },
        );
        return;
      }
      p.posting = false;
    };
    step();
  }

  function onFrame(p: PeerRuntime, frame: Frame): void {
    const peer = p.cfg.peer;
    if (frame.messages.length === 0) return feed(p, { kind: 'frame', id: frame.id, messages: frame.messages }); // a heartbeat: alive
    o.devtools?.record({ kind: 'frame', peer, t: frame.messages.map((m) => m.t).join(','), ...(frame.id !== null && { seq: frame.id }) });
    for (const m of frame.messages) if (m.t === 'welcome') p.info(m.peer); // features come from the peer
    batch(() => {
      feed(p, { kind: 'frame', id: frame.id, messages: frame.messages });
      if (frame.id !== null) o.writer.advance(peer, frame.id);
    });
  }

  async function connect(p: PeerRuntime, params: Readonly<Record<string, string>>): Promise<void> {
    p.conn?.dispose();
    p.cancelRetry?.();
    const conn = p.scope.child('conn');
    p.conn = conn;
    // the watchdog: the link drops a stream (or a connect) that says nothing for two keepalives
    const watch = (): void => {
      conn.timeout(WATCH_MS, () => {
        feed(p, { kind: 'tick' });
        if (conn.alive) watch();
      });
    };
    watch();
    try {
      for await (const frame of p.cfg.transport.connect(params, conn.signal)) {
        if (!conn.alive) return;
        onFrame(p, frame);
      }
      p.lastError = 'connection closed';
    } catch (err) {
      if (!conn.alive) return;
      p.asleep = err instanceof PeerDown && err.status === 'asleep';
      p.lastError = message(err);
    }
    if (!conn.alive) return; // replaced or removed: not a loss
    conn.dispose();
    feed(p, { kind: 'lost' });
  }

  const manager: Peers = {
    add(cfg) {
      if (peers.has(cfg.peer)) throw new Error(`peer ${cfg.peer} is added twice`);
      const scope = o.scope.child(`peer:${cfg.peer}`);
      const link = cfg.link ?? syncLink({ peer: cfg.peer, clientId: o.clientId, build: o.build, ...(cfg.saved && { saved: cfg.saved }) });
      const p: PeerRuntime = { cfg, link, scope, status: statusSig(cfg.peer), info: infoSig(cfg.peer), postQueue: [], posting: false, asleep: false, waking: false, lastError: 'not connected', postFails: 0 };
      peers.set(cfg.peer, p);
      ids([...untracked(ids), cfg.peer]);
      scope.add(() => {
        peers.delete(cfg.peer);
        ids(untracked(ids).filter((x) => x !== cfg.peer));
        p.status({ kind: 'unreachable', reason: 'removed' });
      });
      // open after the current task, so the views that watch right after add() ride on the hello
      scope.timeout(0, () => feed(p, { kind: 'open' }));
      // the watches and pending mutations that waited for this peer go now
      const pending = waiting.get(cfg.peer) ?? [];
      waiting.delete(cfg.peer);
      for (const w of pending) manager.watch(cfg.peer, w.topic, w.scope, w.opts);
      const queuedMutates = waitingMutates.get(cfg.peer) ?? [];
      waitingMutates.delete(cfg.peer);
      for (const m of queuedMutates) manager.mutate(cfg.peer, m);
    },
    // a removed peer's rows go with it: a forgotten computer's tasks leave the list, and a computer re-keyed by its
    // welcome (the phone's `grid-mac` → its wisp's id) isn't listed twice, every task and project doubled (r285)
    remove: (peer) => {
      waitingMutates.delete(peer);
      peers.get(peer)?.scope.dispose();
      o.writer.resetPeer(peer);
      for (const fn of on.removed) fn(peer);
    },
    http: (peer) => peerHttp(() => peers.get(peer)?.cfg.transport, peer),
    list: () => () => ids(),
    status: (peer) => {
      const s = statusSig(peer);
      return () => s();
    },
    info: (peer) => {
      const s = infoSig(peer);
      return () => s();
    },
    // the name this device gave the computer (the phone's "Cloud", a machine's saved name) wins over what the
    // computer calls itself: a grid box's own name was its container id ("1959efa12b19"). Empty: the peer's own.
    name: (peer) => peers.get(peer)?.cfg.name || untracked(infoSig(peer))?.name || peer,
    hasFeature: (peer, need) => hasFeature(untracked(infoSig(peer)), need, manager.name(peer)),
    watch(peer, topic, scope, opts = {}) {
      const p = peers.get(peer);
      if (!p) {
        // a peer the app adds a moment later (a machine's, once its row arrives): the watch waits for it
        const w = { topic, scope, opts };
        const list = waiting.get(peer) ?? [];
        list.push(w);
        waiting.set(peer, list);
        scope.add(() => {
          const l = waiting.get(peer)?.filter((x) => x !== w) ?? [];
          if (l.length > 0) waiting.set(peer, l);
          else waiting.delete(peer);
        });
        return;
      }
      const passive = opts.passive === true;
      feed(p, { kind: 'subscribe', topic, passive });
      held(peer, topic, +1);
      scope.add(() => {
        feed(p, { kind: 'unsubscribe', topic, passive });
        held(peer, topic, -1);
      });
      if (!passive && p.asleep) void manager.wake(peer); // opening one of its tasks wakes a box
    },
    async wake(peer) {
      const p = peers.get(peer);
      if (!p || p.waking) return;
      p.asleep = false;
      p.waking = true;
      p.status({ kind: 'waking' });
      try {
        await p.cfg.transport.wake?.(p.scope.signal);
      } catch (err) {
        p.waking = false;
        p.lastError = message(err);
        p.status({ kind: 'unreachable', reason: p.lastError });
        return;
      }
      p.cancelRetry?.();
      feed(p, { kind: 'open' });
    },
    reconnect(peer) {
      for (const p of peer === undefined ? [...peers.values()] : [peers.get(peer)]) {
        if (!p || p.asleep || p.waking) continue; // a sleeping box isn't woken by a refresh (ADR-0065 edge cases)
        p.cancelRetry?.();
        delete p.cancelRetry;
        feed(p, { kind: 'reopen' });
      }
    },
    resume(peer) {
      for (const p of peer === undefined ? [...peers.values()] : [peers.get(peer)]) {
        if (!p || p.asleep || p.waking) continue;
        p.cancelRetry?.();
        delete p.cancelRetry;
        feed(p, { kind: 'resume' });
      }
    },
    mutate(peer, m) {
      const p = peers.get(peer);
      if (!p) {
        const list = waitingMutates.get(peer) ?? [];
        if (!list.some((x) => x.mid === m.mid)) list.push(m);
        waitingMutates.set(peer, list);
        return false;
      }
      feed(p, { kind: 'mutate', mid: m.mid, cmd: m.cmd, args: m.args });
      if (p.asleep) void manager.wake(peer); // sending it a command wakes a box
      return true;
    },
    window(peer, w) {
      const p = peers.get(peer);
      if (!p || untracked(p.status).kind !== 'online') return false;
      feed(p, { kind: 'window', req: w.req, topic: w.topic, beforeSeq: w.beforeSeq, limit: w.limit });
      return true;
    },
    save: (peer) => peers.get(peer)?.link.save(),
    onLive: (fn) => listen(on.live, fn),
    onRemoved: (fn) => listen(on.removed, fn),
    onSent: (fn) => listen(on.sent, fn),
    onAnswer: (fn) => listen(on.answer, fn),
    onNotice: (fn) => listen(on.notice, fn),
    onRows: (fn) => listen(on.rows, fn),
  };
  return manager;
}

/** A peer's routes over its transport: JSON answers, a refusal as an Error with the server's words. */
function peerHttp(transport: () => Transport | undefined, peer: PeerId): PeerHttp {
  const need = <K extends 'request' | 'events'>(k: K): NonNullable<Transport[K]> => {
    const t = transport();
    const f = t?.[k];
    if (!t || !f) throw new Error(`${peer} can't be reached that way (${t?.kind ?? 'no transport'})`);
    return f.bind(t) as NonNullable<Transport[K]>;
  };
  return {
    async send(r, signal) {
      const res = await need('request')(r, signal ?? new AbortController().signal);
      let body: unknown = null;
      try {
        body = res.text ? JSON.parse(res.text) : null;
      } catch {
        body = res.text;
      }
      if (res.status >= 400) {
        const msg = (body as { error?: { message?: string }; message?: string } | null)?.error?.message ?? (body as { message?: string } | null)?.message;
        throw new Error(typeof msg === 'string' ? msg : `HTTP ${res.status}`);
      }
      return body;
    },
    events: (path, signal) => need('events')(path, signal),
    media(path, signal) {
      const t = transport();
      return t?.mediaUrl ? t.mediaUrl(path, signal ?? new AbortController().signal) : undefined;
    },
  };
}
