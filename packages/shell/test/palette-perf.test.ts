// perf-ui-r2 #1 and #2: ⌘K over ~4,500 tasks. Opening it scanned every key once per task (O(n²), one 800 ms task at
// 4×); each key re-lowercased five fields of every item and re-ranked them all. Counted, not timed: the work, not the
// machine's speed, is what these catch.
import { expect, test } from 'vitest';
import type { PaletteItem } from '../src/contract.ts';
import { createRanker, rank } from '../src/palette/match.ts';
import { taskItems } from '../src/palette/tasks.ts';

const N = 5000;

test('building_5000_tasks_palette_items_reads_the_keys_list_once_not_once_per_task', () => {
  const sessions = Array.from({ length: N }, (_, i) => ({ id: `s${i}`, title: `Task ${i}`, state: 'done', updated_at: i, summary: { cwd: '/x' } }));
  let reads = 0;
  const keys = new Proxy(
    sessions.map((s) => `peer1:${s.id}`),
    {
      get(t, k, r) {
        if (typeof k === 'string' && /^\d+$/.test(k)) reads++;
        return Reflect.get(t, k, r);
      },
    },
  );
  const rows: Record<string, unknown[]> = { session: sessions, project: [], setting: [] };
  const opened: unknown[] = [];
  const ctx = {
    client: { entities: { list: (k: string) => () => rows[k] ?? [], row: () => () => undefined, keys: () => () => keys }, home: () => 'peer1', scope: { timers: { now: () => 0 } } },
    t: (k: string) => k,
    navigate: (n: unknown) => opened.push(n),
  } as never;
  const items = taskItems(ctx);
  expect(items).toHaveLength(N);
  expect(items[0]?.id).toBe(`task:peer1:s${N - 1}`);
  items.at(-1)?.run?.();
  expect(opened).toEqual([{ t: 'openTask', id: 'peer1:s0' }]);
  // one pass builds the index (each key read once); the old scan read ~N²/2 = 12.5 million
  expect(reads).toBeLessThanOrEqual(N);
});

const items: PaletteItem[] = Array.from({ length: N }, (_, i) => ({
  id: `task:${i}`,
  group: i % 10 === 0 ? 'shell.palette.group.actions' : 'shell.palette.group.tasks',
  label: `${['Fix', 'Add', 'Refactor', 'Review'][i % 4]} ${['auth flow', 'rate limiting', 'the parser', 'sync reconnect'][i % 7 % 4]} #${i}`,
  sub: `${['api', 'web', 'docs'][i % 3]} · done · ${i}m`,
  keywords: i % 5 === 0 ? 'claude preview' : 'codex',
  ...(i % 6 === 0 && { tags: ['auth', 'db'] }),
  at: i,
}));

test('typing_that_extends_the_query_ranks_only_the_last_keys_hits_and_answers_as_a_fresh_rank', () => {
  const r = createRanker();
  const typed = ['r', 're', 'ref', 'refa', 'refac', 'refac p', 'refac pa', '#a', '#au refac'];
  let prev = N;
  for (const q of typed) {
    const got = r(items, q, N);
    expect(got.map((x) => [x.item.id, x.idx]), q).toEqual(rank(items, q, N).map((x) => [x.item.id, x.idx]));
    if (q !== '#a') expect(r.scored, `"${q}" ranks the last hits only`).toBeLessThanOrEqual(prev);
    prev = r.scored;
  }
  // "refac pa" scored only what "refac p" kept: about a quarter of the list, not all 5,000
  expect(r.scored).toBeLessThan(N / 2);
  // a query that doesn't extend the last one ranks everything again
  r(items, 'add', N);
  expect(r.scored).toBe(N);
});

test('a_list_rebuilt_while_typing_keeps_the_narrowing_and_ranks_what_changed_again', () => {
  // a task streaming rebuilds the palette's items on each patch: the same items, one changed (item 1 was an Add, so
  // "refac" didn't match it)
  const r = createRanker();
  r(items, 'refac', N);
  const before = r.scored;
  const rebuilt = items.map((it) => ({ ...it }));
  const renamed = { ...rebuilt[1], label: 'Refactor the zebra crossing' } as PaletteItem;
  rebuilt[1] = renamed;
  const got = r(rebuilt, 'refac zeb', N);
  expect(r.scored, 'the last hits and the one that changed').toBeLessThanOrEqual(before + 1);
  expect(got.map((x) => x.item.id)).toEqual(rank(rebuilt, 'refac zeb', N).map((x) => x.item.id));
  expect(got.some((x) => x.item.id === renamed.id), 'renamed into a match: found').toBe(true);
});

test('a_topic_item_left_out_while_its_topic_is_typed_comes_back_as_the_topic_grows', () => {
  // ⌘K's topic rows fill the box ("#auth"); one is left out while its own topic is in the query
  const topics: PaletteItem[] = [
    { id: 'topic:tag', group: 'Topics', label: '#tag', fill: '#tag ', tags: ['tag', 'tagsx'] },
    { id: 'topic:other', group: 'Topics', label: '#other', fill: '#other ', tags: ['other'] },
  ];
  const r = createRanker();
  for (const q of ['#t', '#tag', '#tags']) expect(r(topics, q, 0).map((x) => x.item.id), q).toEqual(rank(topics, q, 0).map((x) => x.item.id));
  expect(r(topics, '#tags', 0).map((x) => x.item.id)).toEqual(['topic:tag']);
});
