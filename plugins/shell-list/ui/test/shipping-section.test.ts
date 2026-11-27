// The owner (2026-11-27): a task you clicked Ship on leaves Needs you for its own Shipping section, between Needs you
// and Working, while wisp.ship pushes it and until its PR lands.
import { expect, test } from 'vitest';
import { flatten, itemKey } from '../src/model/flat.ts';
import { SECTION_IDS, needsYou, sections, type Keyed } from '../src/model/sections.ts';
import { signature } from '../src/model/placement.ts';
import type { EntityKey, PrRef, SessionRow } from '../src/types.ts';

const row = (id: string, o: Partial<SessionRow> = {}): Keyed => ({ key: `local:${id}` as EntityKey, s: { id, title: id, agent: 'claude', state: 'review', createdAt: 1, updatedAt: 1, ...o } as SessionRow });
const pr = (state: PrRef['state']): PrRef => ({ number: 7, url: 'https://github.com/o/r/pull/7', state });

test('shipping_sits_between_needs_you_and_working', () => {
  expect(SECTION_IDS.slice(SECTION_IDS.indexOf('needs'), SECTION_IDS.indexOf('working') + 1)).toEqual(['needs', 'shipping', 'working']);
});

test('a_shipping_or_open_pr_task_leaves_needs_you_for_shipping', () => {
  const all = [
    row('plain'),
    row('pushing', { shipping: { step: 'push' }, updatedAt: 2 }),
    row('open', { pr: pr('OPEN'), updatedAt: 3 }),
    row('merged', { pr: pr('MERGED') }),
    row('fixing', { state: 'running', pr: pr('OPEN') }),
    row('asks', { state: 'waiting', pr: pr('OPEN') }),
    row('done', { state: 'done', pr: pr('OPEN') }),
    row('gone', { archived: true, pr: pr('OPEN') }),
  ];
  const sec = sections(all);
  expect(sec.shipping.map((e) => e.key)).toEqual(['local:open', 'local:pushing']);
  // a merged PR is back for you to look at; one running, asking, done or archived sits where that says
  expect(sec.needs.map((e) => e.key)).toEqual(['local:asks', 'local:plain', 'local:merged']);
  expect(sec.working.map((e) => e.key)).toEqual(['local:fixing']);
  expect(sec.done.map((e) => e.key)).toEqual(['local:done']);
  expect(sec.archived.map((e) => e.key)).toEqual(['local:gone']);
  expect(flatten(sec, { toggled: new Set(), split: false }).map(itemKey)).toEqual([
    'h:needs', 'local:asks', 'local:plain', 'local:merged', 'h:shipping', 'local:open', 'local:pushing', 'h:working', 'local:fixing',
    'h:done', 'h:archived',
  ]);
});

// the list rebuilds its sections only when a row's placement signature changes: Ship and the PR landing must change it
test('shipping_and_the_pr_landing_change_the_rows_placement', () => {
  const s = row('a').s;
  expect(signature({ ...s, shipping: { step: 'commit' } })).not.toBe(signature(s));
  expect(signature({ ...s, pr: pr('OPEN') })).not.toBe(signature({ ...s, pr: pr('MERGED') }));
});

// Audit final2 doc6: a shipped task whose PR wants you (a failing check, a conflict, changes requested) or whose
// report you haven't read sat in Shipping, quiet, and the tab title never counted it. It is back in Needs you.
test('a_shipped_task_that_needs_you_leaves_shipping_for_needs_you', () => {
  const open = (o: Partial<PrRef> = {}): PrRef => ({ ...pr('OPEN'), checks: { pass: 3, fail: 0 }, ...o });
  expect(sections([row('quiet', { pr: open() })]).shipping.map((e) => e.key)).toEqual(['local:quiet']);
  const loud: [string, Partial<SessionRow>][] = [
    ['failing', { pr: open({ checks: { pass: 2, fail: 1 } }) }],
    ['conflict', { pr: open({ conflict: true }) }],
    ['changes', { pr: open({ review: 'CHANGES_REQUESTED' }) }],
    ['unread', { pr: open(), unread: true }],
    ['pushing', { shipping: { step: 'push' }, pr: open({ checks: { fail: 1 } }) }],
  ];
  for (const [id, o] of loud) {
    const sec = sections([row(id, o)]);
    expect(sec.needs.map((e) => e.key), id).toEqual([`local:${id}`]);
    expect(sec.shipping, id).toEqual([]);
  }
  // the tab title counts a failing PR though its report was read
  expect(needsYou(row('failing', { pr: open({ checks: { fail: 1 } }) }).s)).toBe(true);
  expect(needsYou(row('quiet', { pr: open() }).s)).toBe(false);
});

test('a_failing_check_changes_the_rows_placement', () => {
  const s = row('a', { pr: pr('OPEN') }).s;
  expect(signature({ ...s, pr: { ...pr('OPEN'), checks: { fail: 1 } } })).not.toBe(signature(s));
});

// Audit final2 new2: a compare task was never shipped (only a single task was), so one whose shipped run is quiet
// stayed in Needs you while the shell's Sidebar put that run under Shipping.
test('a_compare_task_whose_shipped_run_is_quiet_goes_to_shipping', () => {
  const two = (a: Partial<SessionRow>, b: Partial<SessionRow>): Keyed[] => [row('r1', { groupId: 'g', createdAt: 1, ...a }), row('r2', { groupId: 'g', createdAt: 2, ...b })];
  const quiet: PrRef = { ...pr('OPEN'), checks: { pass: 3, fail: 0 } };
  expect(sections(two({ pr: quiet }, {})).shipping.map((e) => e.key)).toEqual(['g:g']);
  expect(sections(two({ pr: quiet }, {})).needs).toEqual([]);
  expect(sections(two({ pr: { ...quiet, checks: { fail: 1 } } }, {})).needs.map((e) => e.key)).toEqual(['g:g']);
  expect(sections(two({ pr: quiet }, { unread: true })).needs.map((e) => e.key)).toEqual(['g:g']);
  expect(sections(two({ pr: quiet }, { unread: true })).shipping).toEqual([]);
});
