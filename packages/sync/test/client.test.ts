// SyncClient rules, one by one.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { ClientMessage, ServerMessage, Welcome } from '@wisp/protocol';
import { SyncClient } from '../src/client.ts';
import type { ClientOutput } from '../src/types.ts';
import { PEER } from './harness.ts';

const welcome = (over: Partial<Welcome> = {}): Welcome => ({ t: 'welcome', epoch: 'E1', head_seq: 10, oldest_seq: 0, resume: false, proto: 2, peer: PEER, ...over });
const sent = (outs: ClientOutput[]): ClientMessage[] => outs.flatMap((o) => (o.kind === 'send' ? [o.message] : []));
const kinds = (outs: ClientOutput[]): string[] => outs.map((o) => o.kind);

function live(c: SyncClient, w: Partial<Welcome> = {}, id = 10): ClientOutput[] {
  c.handle({ kind: 'open' }, 0);
  return c.handle({ kind: 'frame', id, messages: [welcome(w)] }, 0);
}

const newClient = (over: Partial<ConstructorParameters<typeof SyncClient>[0]> = {}): SyncClient => new SyncClient({ peer: 'laptop', clientId: 'tab1', build: 'b1', ...over });

test('two views of one session are one sub; the last one out unsubscribes', () => {
  const c = newClient();
  live(c);
  assert.deepEqual(sent(c.handle({ kind: 'subscribe', topic: 'session:a' }, 0)), [{ t: 'sub', topic: 'session:a' }]);
  assert.deepEqual(sent(c.handle({ kind: 'subscribe', topic: 'session:a' }, 0)), []);
  assert.deepEqual(sent(c.handle({ kind: 'unsubscribe', topic: 'session:a' }, 0)), []);
  assert.deepEqual(sent(c.handle({ kind: 'unsubscribe', topic: 'session:a' }, 0)), [{ t: 'unsub', topic: 'session:a' }]);
});

test('passive refs: a passive-only topic is a passive sub; the first active view makes it count', () => {
  const c = newClient();
  live(c);
  assert.deepEqual(sent(c.handle({ kind: 'subscribe', topic: 'machines', passive: true }, 0)), [{ t: 'sub', topic: 'machines', passive: true }]);
  assert.deepEqual(sent(c.handle({ kind: 'subscribe', topic: 'machines' }, 0)), [{ t: 'sub', topic: 'machines' }]);
  assert.deepEqual(sent(c.handle({ kind: 'unsubscribe', topic: 'machines' }, 0)), [{ t: 'sub', topic: 'machines', passive: true }]);
});

test('the hello: proto window, cursor for this epoch, resumed and fresh topics, passive ones', () => {
  const c = newClient({ saved: { epoch: 'E1', cursor: 42, synced: ['list'], pending: [] } });
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  c.handle({ kind: 'subscribe', topic: 'session:a' }, 0);
  c.handle({ kind: 'subscribe', topic: 'machines', passive: true }, 0);
  const req = c.handle({ kind: 'open' }, 0).find((o) => o.kind === 'request');
  assert.deepEqual(req, { kind: 'request', params: { proto_min: '2', proto_max: '2', client_id: 'tab1', build: 'b1', epoch: 'E1', cursor: '42', topics: 'list', fresh: 'session:a,machines', passive: 'machines' } });
  // a resumed welcome keeps the rows; the cursor moves to the frame's id
  const outs = c.handle({ kind: 'frame', id: 42, messages: [welcome({ resume: true })] }, 0);
  assert.ok(!kinds(outs).includes('reset'));
  assert.equal(c.state, 'live');
  assert.deepEqual(c.save(), { epoch: 'E1', cursor: 42, synced: ['list'], pending: [], heardAt: 0 });
});

test('resume=false or a new epoch drops the caches for this peer', () => {
  const c = newClient({ saved: { epoch: 'E1', cursor: 42, synced: ['list'], pending: [] } });
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  assert.ok(kinds(live(c, { resume: false })).includes('reset'));
  const d = newClient({ saved: { epoch: 'E1', cursor: 42, synced: ['list'], pending: [] } });
  d.handle({ kind: 'subscribe', topic: 'list' }, 0);
  assert.ok(kinds(live(d, { resume: true, epoch: 'E2' })).includes('reset'), 'a resume from another epoch is not trusted');
  assert.equal(d.epoch, 'E2');
});

test('pending mutations go again after welcome, with the same mid, until ack or reject', () => {
  const c = newClient();
  c.handle({ kind: 'mutate', mid: 'm1', cmd: 'session.followUp', args: { text: 'hi' } }, 0);
  assert.deepEqual(sent(live(c)), [{ t: 'mutate', mid: 'm1', cmd: 'session.followUp', args: { text: 'hi' } }]);
  c.handle({ kind: 'lost' }, 0);
  const again = live(c);
  assert.deepEqual(sent(again).map((m) => m.t === 'mutate' && m.mid), ['m1'], 'a flaky tunnel: same key, never run twice');
  // the ack comes after the frame's ops, so base has the effect before the optimistic patch goes
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  c.handle({ kind: 'frame', id: 11, messages: [{ t: 'snapshot', topic: 'list', seq: 11, rows: [] }] }, 0);
  const outs = c.handle({ kind: 'frame', id: 12, messages: [{ t: 'ack', mid: 'm1', seq: 12 }, { t: 'patch', seq: 12, ops: [{ topic: 'list', e: 's', id: '1', op: 'upsert', v: 1 }] }] }, 0);
  assert.deepEqual(kinds(outs), ['entityOps', 'ack']);
  assert.deepEqual(c.save().pending, []);
  c.handle({ kind: 'mutate', mid: 'm2', cmd: 'x', args: null }, 0);
  const rej = c.handle({ kind: 'frame', id: 12, messages: [{ t: 'reject', mid: 'm2', code: 'forbidden', message: 'no' }] }, 0);
  assert.deepEqual(rej.filter((o) => o.kind === 'reject'), [{ kind: 'reject', mid: 'm2', code: 'forbidden', message: 'no' }]);
});

test('version window: outside it, the side that is older must update', () => {
  const old = newClient({ protoMin: 1, protoMax: 1 });
  assert.deepEqual(live(old).filter((o) => o.kind === 'state' && o.state === 'incompatible'), [{ kind: 'state', state: 'incompatible', update: 'client' }]);
  assert.equal(old.state, 'incompatible');
  assert.deepEqual(old.handle({ kind: 'lost' }, 0), [], 'no retry loop against a peer it cannot speak to');
  const ahead = newClient({ protoMin: 5, protoMax: 7 });
  assert.deepEqual(live(ahead), [{ kind: 'state', state: 'incompatible', update: 'peer' }], 'nothing else in the frame counts');
  assert.equal(ahead.state, 'incompatible');
  // overlapping windows speak the highest both know
  assert.deepEqual(live(newClient({ protoMin: 2, protoMax: 4 })).filter((o) => o.kind === 'state'), [{ kind: 'state', state: 'live' }]);
});

test('snapshot pages: the first joins the topic as it lands, the last brings every page as the snapshot; a page at another seq starts over (ADR-0081)', () => {
  const c = newClient();
  live(c);
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  const page = (seq: number, id: string, more: boolean): ServerMessage => ({ t: 'snapshot', topic: 'list', seq, rows: [{ e: 's', id, v: id }], ...(more && { more: true }) });
  // the first screen draws from the first page, before the rest is read
  assert.deepEqual(c.handle({ kind: 'frame', id: 10, messages: [page(10, 'a', true)] }, 0), [{ kind: 'snapshotPage', peer: 'laptop', topic: 'list', rows: [{ e: 's', id: 'a', v: 'a' }] }]);
  // the server started the topic over at a later seq: the old pages are worthless, and the snapshot replaces what they showed;
  // only a snapshot's first page shows early (one repaint for it, one for the whole topic, not one a page)
  const outs = c.handle({ kind: 'frame', id: 12, messages: [page(12, 'b', true), page(12, 'c', true), page(12, 'd', false)] }, 0);
  assert.deepEqual(outs, [
    { kind: 'snapshotPage', peer: 'laptop', topic: 'list', rows: [{ e: 's', id: 'b', v: 'b' }] },
    { kind: 'snapshot', peer: 'laptop', topic: 'list', rows: [{ e: 's', id: 'b', v: 'b' }, { e: 's', id: 'c', v: 'c' }, { e: 's', id: 'd', v: 'd' }] },
  ]);
  // ops wait for the whole snapshot: a page doesn't make the topic synced
  const op = { topic: 'list' as const, e: 's', id: 'x', op: 'patch' as const, ops: [] };
  const d = newClient();
  live(d);
  d.handle({ kind: 'subscribe', topic: 'list' }, 0);
  d.handle({ kind: 'frame', id: 10, messages: [page(10, 'a', true)] }, 0);
  assert.deepEqual(d.handle({ kind: 'frame', id: 11, messages: [{ t: 'patch', seq: 11, ops: [op] }] }, 0), []);
});

test('ops for a topic still waiting for its snapshot are ignored', () => {
  const c = newClient();
  live(c);
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  const op = { topic: 'list' as const, e: 's', id: '1', op: 'patch' as const, ops: [] };
  assert.deepEqual(c.handle({ kind: 'frame', id: 11, messages: [{ t: 'patch', seq: 11, ops: [op] }] }, 0), []);
});

test('resync: a synced topic resubscribes from the cursor, one never synced starts fresh', () => {
  const c = newClient();
  live(c);
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  c.handle({ kind: 'subscribe', topic: 'session:a' }, 0);
  c.handle({ kind: 'frame', id: 15, messages: [{ t: 'snapshot', topic: 'list', seq: 15, rows: [] }] }, 0);
  const outs = c.handle({ kind: 'frame', id: 14, messages: [{ t: 'resync', topic: 'list' }, { t: 'resync', topic: 'session:a' }] }, 0);
  assert.deepEqual(kinds(outs), ['needResubscribe', 'send', 'needResubscribe', 'send']);
  // the frame's id is the watermark, even when it is lower than the last one
  assert.deepEqual(sent(outs), [{ t: 'sub', topic: 'list', since: 14 }, { t: 'sub', topic: 'session:a' }]);
  assert.equal(c.cursor, 14);
});

test('denied topics are reported and left out of the next hello', () => {
  const c = newClient();
  c.handle({ kind: 'subscribe', topic: 'session:secret' }, 0);
  live(c);
  const outs = c.handle({ kind: 'frame', id: 10, messages: [{ t: 'notice', kind: 'denied', data: { topic: 'session:secret', code: 'forbidden' } }, { t: 'notice', kind: 'toast', data: 'hi' }] }, 0);
  assert.deepEqual(outs, [{ kind: 'denied', topic: 'session:secret', code: 'forbidden' }, { kind: 'notice', notice: { t: 'notice', kind: 'toast', data: 'hi' } }]);
  c.handle({ kind: 'lost' }, 0);
  const req = c.handle({ kind: 'open' }, 0).find((o) => o.kind === 'request');
  assert.equal(req?.kind === 'request' && req.params['fresh'], undefined);
});

test('lost: retries back off as v1 did (2/5/15/30/60 s, the last repeated), and reset after a welcome', () => {
  const c = newClient();
  c.handle({ kind: 'open' }, 0);
  const at: number[] = [];
  for (let i = 0; i < 7; i++) {
    const r = c.handle({ kind: 'lost' }, 1000).find((o) => o.kind === 'retry');
    at.push(r?.kind === 'retry' ? r.at - 1000 : -1);
    c.handle({ kind: 'open' }, 1000);
  }
  assert.deepEqual(at, [2000, 5000, 15000, 30000, 60000, 60000, 60000]);
  c.handle({ kind: 'frame', id: 1, messages: [welcome()] }, 0);
  const r = c.handle({ kind: 'lost' }, 0).find((o) => o.kind === 'retry');
  assert.deepEqual(r, { kind: 'retry', at: 2000 });
});

test('a_silent_stream_is_reopened_from_its_cursor', () => {
  const c = newClient({ keepaliveMs: 15_000 });
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  c.handle({ kind: 'open' }, 0);
  c.handle({ kind: 'frame', id: 10, messages: [welcome()] }, 0);
  c.handle({ kind: 'frame', id: 12, messages: [{ t: 'snapshot', topic: 'list', seq: 12, rows: [] }] }, 1_000);
  // a heartbeat (an empty frame) is life: the watchdog counts from it
  c.handle({ kind: 'frame', id: null, messages: [] }, 16_000);
  assert.deepEqual(c.handle({ kind: 'tick' }, 45_000), [], 'one keepalive and a bit: still fine');
  const outs = c.handle({ kind: 'tick' }, 46_000);
  assert.deepEqual(outs, [{ kind: 'drop' }, { kind: 'state', state: 'offline' }, { kind: 'retry', at: 48_000 }], 'two keepalives of silence: dropped, then the first wait');
  assert.equal(c.state, 'offline');
  assert.deepEqual(c.handle({ kind: 'tick' }, 50_000), [], 'nothing open, nothing to drop');
  // reopened from the cursor: the hello resumes the topic it holds
  const req = c.handle({ kind: 'open' }, 48_000).find((o) => o.kind === 'request');
  assert.equal(req?.kind === 'request' && req.params['cursor'], '12');
  assert.equal(req?.kind === 'request' && req.params['topics'], 'list');
  // a connect that never answers is silent too: dropped at the same line, the wait growing
  const again = c.handle({ kind: 'tick' }, 78_000);
  assert.deepEqual(again.at(-1), { kind: 'retry', at: 83_000 });
  assert.equal(c.save().heardAt, 16_000, 'when it was last heard from: the offline bar says so');
});

test('reopen skips the backoff: offline it opens now; live it drops the stream and resumes', () => {
  const c = newClient();
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  live(c);
  c.handle({ kind: 'frame', id: 11, messages: [{ t: 'snapshot', topic: 'list', seq: 11, rows: [] }] }, 0);
  c.handle({ kind: 'lost' }, 0);
  const now = c.handle({ kind: 'reopen' }, 100);
  assert.deepEqual(kinds(now), ['state', 'request'], 'no waiting out the 2 s');
  assert.equal(c.state, 'connecting');
  const outs = c.handle({ kind: 'reopen' }, 200);
  assert.deepEqual(kinds(outs), ['drop', 'state', 'request'], 'a stream that may be dead is replaced at once, with no offline flash');
  const req = outs.find((o) => o.kind === 'request');
  assert.equal(req?.kind === 'request' && req.params['cursor'], '11');
  assert.deepEqual(newClient().handle({ kind: 'reopen' }, 0), [], 'never opened: nothing to reopen');
});

test('subs and unsubs made while connecting are sent after the welcome', () => {
  const c = newClient();
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  c.handle({ kind: 'open' }, 0);
  c.handle({ kind: 'subscribe', topic: 'session:a' }, 0);
  c.handle({ kind: 'unsubscribe', topic: 'list' }, 0);
  const outs = c.handle({ kind: 'frame', id: 10, messages: [welcome()] }, 0);
  assert.deepEqual(sent(outs), [{ t: 'sub', topic: 'session:a' }, { t: 'unsub', topic: 'list' }]);
});

test('a drifted topic starts over: unsub, then a fresh sub', () => {
  const c = newClient();
  live(c);
  c.handle({ kind: 'subscribe', topic: 'list' }, 0);
  assert.deepEqual(sent(c.handle({ kind: 'drifted', topic: 'list' }, 0)), [{ t: 'unsub', topic: 'list' }, { t: 'sub', topic: 'list' }]);
});
