import { expect, test } from 'vitest';
import type { EntityKey } from '@wisp/client';
import { historyOf, lastTurnOf, offerView, runSave, saveable, saveStateOf, type Tracked } from '../src/save.ts';
import { t } from '../src/i18n.ts';
import '../src/i18n/lazy.ts';

const tracked = (files: number, scrubbed = 0): Tracked => ({ repo: '/r', store: 'files', files: Array.from({ length: files }, (_, i) => `.wisp/f${i}`), commits: [], scrubbed });

test('save_to_repo_reads_the_history_slot_from_a_synced_row_or_a_flat_one', () => {
  expect(historyOf({ id: 'a', body: { ext: { 'wisp.history': { offer: true, repo: '/r' } } } }).offer).toBe(true);
  expect(historyOf({ id: 'a', ext: { 'wisp.history': { tracked: { files: ['x'] } } } }).tracked?.files).toEqual(['x']);
  expect(historyOf({ id: 'a' })).toEqual({});
  expect(historyOf(undefined)).toEqual({});
});

test('the_menu_saves_any_task_but_a_pending_one_or_a_subagents', () => {
  expect(saveable({ id: 'a' })).toBe(true);
  expect(saveable({ id: 'a', pending: true })).toBe(false);
  expect(saveable({ id: 'a', parent_id: 'p' })).toBe(false);
  expect(saveable({ id: 'a', parentId: 'p' })).toBe(false);
  expect(saveable({ id: 'a', parent_id: null })).toBe(true);
  expect(saveable(undefined)).toBe(false);
});

test('the_answer_offers_then_says_saving_then_what_was_saved', () => {
  expect(offerView({ offer: true }, undefined)).toEqual({ v: 'offer' });
  expect(offerView({}, undefined)).toEqual({ v: 'none' });
  expect(offerView({ offer: false, tracked: { files: [] } }, undefined)).toEqual({ v: 'none' });
  expect(offerView({ offer: true }, { s: 'saving' })).toEqual({ v: 'saving' });
  // the server clears the offer as it saves: the answer keeps saying what was saved
  expect(offerView({ offer: false }, { s: 'saved', files: 3, repo: '/r', scrubbed: 1 })).toEqual({ v: 'saved', files: 3, scrubbed: 1 });
  // refused: offered again while the task still is
  expect(offerView({ offer: true }, { s: 'failed' })).toEqual({ v: 'offer' });
  expect(offerView({}, { s: 'failed' })).toEqual({ v: 'none' });
});

test('only_the_tasks_last_turn_shows_the_offer', () => {
  const rows = [
    { id: 'p:t1', body: { id: 't1', idx: 0, sessionId: 's1' } },
    { id: 'p:t3', body: { id: 't3', idx: 2, sessionId: 's1' } },
    { id: 'p:t2', body: { id: 't2', idx: 1, sessionId: 's1' } },
    { id: 'p:u9', body: { id: 'u9', idx: 9, sessionId: 's2' } },
  ];
  expect(lastTurnOf(rows, 's1')).toBe('t3');
  expect(lastTurnOf([{ id: 'x', idx: 4, sessionId: 's3' }], 's3')).toBe('x');
  expect(lastTurnOf(rows, 'none')).toBeUndefined();
});

test('a_save_keeps_its_state_and_a_second_press_while_saving_does_nothing', async () => {
  const key = 'local:a1' as EntityKey;
  let calls = 0;
  let finish: (v: Tracked) => void = () => {};
  const run = (): Promise<Tracked> => {
    calls++;
    return new Promise((r) => (finish = r));
  };
  const first = runSave(run, key);
  expect(saveStateOf(key)).toEqual({ s: 'saving' });
  await runSave(run, key);
  expect(calls).toBe(1);
  finish(tracked(4, 2));
  await first;
  expect(saveStateOf(key)).toEqual({ s: 'saved', files: 4, repo: '/r', scrubbed: 2 });
  await runSave(() => Promise.reject(new Error('no')), 'local:b2' as EntityKey);
  expect(saveStateOf('local:b2')).toEqual({ s: 'failed' });
});

test('the_saved_line_says_how_many_files_and_secrets', () => {
  expect(t('history.saved', { count: 1 })).toBe('Saved to the repo: 1 file in .wisp/, to commit with the code');
  expect(t('history.saved.scrubbed', { count: 3, scrubbed: 2 })).toBe('Saved to the repo: 3 files in .wisp/ (2 scrubbed: keys, tokens, private paths)');
  expect(t('history.save')).toBe('Save to repo');
});
