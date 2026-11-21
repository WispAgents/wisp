// ADR-0065 step 13: the phone list's bulk actions act on the picked tasks a row's own menu would offer them for, and
// leave the rest as they are (design/mobile.md §A.4: Mark done, Save for later, Archive, Delete).
import { expect, test } from 'vitest';
import { bulkTargets, canBulk } from '../src/frame/bulk.ts';
import { isSaved } from '../src/frame/commands.ts';
import type { SessionRow } from '../src/rows.ts';

const row = (id: string, o: Partial<SessionRow> = {}): SessionRow => ({ id, title: id, state: 'review', agent: 'claude', archived: false, unread: false, created_at: 0, updated_at: 0, summary: {}, ...o }) as SessionRow;

test('bulk_actions_skip_the_tasks_that_cannot_take_them', () => {
  const rows = new Map([
    ['ready', row('ready')],
    ['running', row('running', { state: 'running' })],
    ['done', row('done', { state: 'done' })],
    ['archived', row('archived', { archived: true })],
    ['saved', row('saved', { summary: { forLater: true } as never })],
    ['pending', row('pending', { summary: { pending: true } as never })],
  ]);
  const keys = [...rows.keys(), 'gone'];
  const of = (k: string) => rows.get(k);
  expect(bulkTargets('done', keys, of)).toEqual(['ready', 'saved', 'pending']);
  expect(bulkTargets('archive', keys, of)).toEqual(['ready', 'done', 'saved', 'pending']);
  // a task still waiting to send (pending) can't be saved for later, as on its own menu
  expect(bulkTargets('later', keys, of)).toEqual(['ready', 'done']);
  expect(bulkTargets('delete', keys, of)).toEqual(['ready', 'running', 'done', 'archived', 'saved', 'pending']);
});

test('a_working_task_is_never_marked_done_or_archived_in_bulk', () => {
  for (const state of ['running', 'waiting'] as const) {
    expect(canBulk('done', row('x', { state }))).toBe(false);
    expect(canBulk('archive', row('x', { state }))).toBe(false);
  }
});

// Owner's desktop test 2026-11-17 #12: the server's summary says a saved task's forLater as `{at, by}` (runtime host
// ops.rs for_later), never `true`: read as not saved, its bookmark stayed empty and Save for later saved it again.
test('a_task_the_server_says_is_saved_reads_as_saved', () => {
  const saved = row('saved', { summary: { forLater: { at: 5, by: 'you' } } as never });
  expect(canBulk('later', saved)).toBe(false);
  expect(isSaved(saved.summary['forLater'])).toBe(true);
  for (const v of [undefined, null, false]) expect(isSaved(v)).toBe(false);
});
