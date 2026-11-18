// Audit A #8: a swiped row runs what its side says, as the row's menu does: Mark done, Save for later and Archive at
// once (each with its Undo), Delete task only after asking, as the desktop's Delete asks (it can't be undone).
import { expect, test, vi } from 'vitest';
import type { EntityKey } from '@wisp/client';
import { runSwipe } from '../src/ui/swipe-run.ts';

const key = 'local:s1' as EntityKey;

test('a_delete_swipe_asks_first_and_deletes_nothing_yet', () => {
  const askDelete = vi.fn();
  const run = vi.fn();
  const actions = { client: { run, entities: { row: () => () => ({ id: 's1', state: 'review' }) } } } as never;
  runSwipe('delete', { actions, askDelete }, key);
  expect(askDelete).toHaveBeenCalledWith(key);
  expect(run).not.toHaveBeenCalled();
});

test('the_other_sides_run_their_action_at_once', () => {
  const calls: string[] = [];
  const act = { done: () => calls.push('done'), later: () => calls.push('later'), archive: () => calls.push('archive') };
  for (const kind of ['done', 'later', 'archive'] as const) runSwipe(kind, { actions: {} as never, askDelete: () => calls.push('ask') }, key, act);
  expect(calls).toEqual(['done', 'later', 'archive']);
});
