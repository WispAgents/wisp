// Android's share sheet (audit mobile r1, M2): the manifest listed wisp and MainActivity held the share, but nothing in
// the bundle took it, so the native toast told you to update wisp on the computer. Now New task opens with the shared
// words in the box (marked as from outside) and the files attached, read through the bridge a piece at a time; a share
// while no computer can take tasks waits on the phone, unclaimed; a token that isn't the share's reads nothing.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createClient, signal, type Client } from '@wisp/client';
import { followShares, type ShareBridge, type ShareWindow } from '../src/share-in.ts';
import { tick } from './helpers.ts';

const TOKEN = 'b7c1d0e2-0000-4000-8000-000000000001';
const PICTURE = Uint8Array.from({ length: 10 }, (_, i) => i * 7);
const b64 = (b: Uint8Array): string => btoa(String.fromCharCode(...b));

/** MainActivity's share bridge, as Shared.java answers: the share once for its token, then 4-byte pieces. */
function fakeAndroid() {
  const listeners = new Map<string, Set<(e: Event) => void>>();
  const log = { asks: 0, takes: [] as string[], chunks: 0, done: [] as string[] };
  let claimed = false;
  const app: ShareBridge = {
    askShare: () => {
      log.asks++;
    },
    takeShare: (token) => {
      log.takes.push(token);
      if (token !== TOKEN || claimed) return '';
      claimed = true;
      return JSON.stringify({ title: '', text: 'Look at this https://example.com/pr/1', left: 0, files: [{ name: 'shot.png', type: 'image/png', size: PICTURE.length }] });
    },
    shareChunk: (token, file, offset) => {
      log.chunks++;
      if (token !== TOKEN || file !== 0) return '';
      return b64(PICTURE.subarray(offset, offset + 4));
    },
    shareDone: (token) => void log.done.push(token),
  };
  const w: ShareWindow & { fire(token?: string): void } = {
    wispAndroid: app,
    addEventListener: (t, fn) => void (listeners.get(t) ?? listeners.set(t, new Set()).get(t))?.add(fn),
    removeEventListener: (t, fn) => void listeners.get(t)?.delete(fn),
    fire(token) {
      for (const fn of listeners.get('wisp-share') ?? []) fn(new CustomEvent('wisp-share', { detail: token ? { token } : {} }));
    },
  };
  return { w, log, listeners };
}

function setup(ready = true) {
  const client: Client = createClient({ clientId: 'p', build: 'test', home: 'studio' as never, width: 390 });
  const stuff = client.plugin('wisp.stuff');
  const attached: File[] = [];
  const said: string[] = [];
  const can = signal(ready);
  const f = fakeAndroid();
  const scope = client.scope.child('share');
  followShares({ window: f.w, ready: can, stuff: () => stuff, attach: (fs) => void attached.push(...fs), say: (t) => void said.push(t), t: (k) => k, scope });
  const draft = (field: string): unknown => client.drafts.get<never>('studio:launcher', field)();
  return { client, f, attached, said, can, draft, scope };
}

async function until(cond: () => boolean): Promise<void> {
  for (let i = 0; i < 200 && !cond(); i++) await new Promise((r) => setTimeout(r));
}

test('a_share_opens_new_task_with_its_words_and_its_files', async () => {
  const s = setup();
  assert.equal(s.f.log.asks, 1, 'asks at once for a share that came before the page listened');
  s.f.w.fire(TOKEN);
  await until(() => s.f.log.done.length > 0 && s.attached.length > 0);
  assert.equal(s.draft('text'), 'Look at this https://example.com/pr/1');
  assert.equal(s.draft('sharedIn'), true, 'marked as from outside until it is sent');
  assert.equal(s.client.nav.nav().main.kind, 'home', 'New task');
  assert.equal(s.attached.length, 1);
  const file = s.attached[0];
  assert.ok(file);
  assert.equal(file.name, 'shot.png');
  assert.equal(file.type, 'image/png');
  assert.deepEqual(new Uint8Array(await file.arrayBuffer()), PICTURE, 'the bytes put back together from the pieces');
  assert.deepEqual(s.f.log.done, [TOKEN], 'Android’s copy let go once');
  // the same event again (MainActivity offers it on each page load): it's gone, nothing more happens
  s.f.w.fire(TOKEN);
  await tick();
  assert.deepEqual(s.f.log.done, [TOKEN]);
  assert.deepEqual(s.said, []);
  s.client.dispose();
});

test('a_token_that_is_not_the_shares_reads_nothing', async () => {
  const s = setup();
  s.f.w.fire('someone-elses');
  s.f.w.fire(); // the list's repaint event carries none
  await tick();
  assert.deepEqual(s.f.log.takes, ['someone-elses']);
  assert.equal(s.f.log.chunks, 0);
  assert.deepEqual(s.f.log.done, []);
  assert.equal(s.draft('text'), undefined);
  s.client.dispose();
});

test('with_no_computer_to_take_it_the_share_waits_unclaimed_and_says_to_open_one', async () => {
  const s = setup(false);
  s.f.w.fire(TOKEN);
  s.f.w.fire(TOKEN);
  await tick();
  assert.deepEqual(s.f.log.takes, [], 'not claimed: it stays on the phone');
  assert.deepEqual(s.said, ['phone.share.pick'], 'once, and never "update wisp"');
  // a computer can take tasks now: the share is asked for again, and this time taken
  s.can(true);
  assert.equal(s.f.log.asks, 2);
  s.f.w.fire(TOKEN);
  await until(() => s.f.log.done.length > 0 && s.attached.length > 0);
  assert.equal(s.draft('text'), 'Look at this https://example.com/pr/1');
  // the shell it ran on goes: its listener with it
  s.scope.dispose();
  assert.equal(s.f.listeners.get('wisp-share')?.size ?? 0, 0);
  s.client.dispose();
});
