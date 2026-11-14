// What watches Mark done (src/ask.ts) loads apart from the first page (ADR-0011): what was in flight is noted when
// the plugin starts, so a Done made between then and the watcher landing is still asked about, and one already in
// flight at start (restored from disk after a reload) is not asked about again.
import { expect, test, vi } from 'vitest';
import { signal, type EntityKey, type PendingInfo, type PeerId, type PluginClient } from '@wisp/client';
import { watchDone } from '../src/ask.ts';

const peer = 'home' as PeerId;
const done = (mid: string, id: string): PendingInfo => ({ mid, peer, ops: [{ e: 'session', id: `home:${id}` as EntityKey, op: 'patch', ops: [{ op: 'replace', path: '/state', value: 'done' }] }] });

test('a_done_made_before_the_watcher_lands_is_asked_about_and_one_restored_at_start_is_not', async () => {
  const pending = signal<readonly PendingInfo[]>([done('m-restored', 'old')]);
  const run = vi.fn(async () => ({ onDone: 'ask', folders: [] }));
  const c = {
    entities: { pending: () => pending, peek: () => ({ state: 'done' }) },
    run,
    navigate: vi.fn(),
    scope: { add: () => undefined, timeout: () => () => undefined },
  } as unknown as PluginClient;
  // the plugin starts: what's in flight is noted (index.ts setup)
  const seen = new Set(pending().map((p) => p.mid));
  // Mark done, before the watcher's module has landed; its command has settled by the time it does
  pending([...pending(), done('m-new', 'fresh')]);
  watchDone(c, seen);
  pending([]);
  await new Promise((r) => setTimeout(r, 0));
  expect(run.mock.calls.map((x) => (x as unknown[])[1])).toEqual([{ peer, sessionId: 'fresh', sizes: false }]);
});
