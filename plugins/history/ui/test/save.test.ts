import { expect, test } from 'vitest';
import type { EntityKey } from '@wisp/client';
import { historyOf, lastTurnOf, offerView, pushable, pushedLink, pushedOf, pushedText, pushStateOf, runPush, runSave, saveable, saveStateOf, working, type Pushed, type Tracked } from '../src/save.ts';
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
  expect(offerView({ offer: false }, { s: 'saved', files: 3, repo: '/r', scrubbed: 1, at: 42 })).toEqual({ v: 'saved', files: 3, scrubbed: 1, at: 42 });
  // refused: offered again while the task still is
  expect(offerView({ offer: true }, { s: 'failed' })).toEqual({ v: 'offer' });
  expect(offerView({}, { s: 'failed' })).toEqual({ v: 'none' });
});

test('a_working_task_offers_nothing_even_after_an_earlier_offer', () => {
  expect(working({ id: 'a', state: 'running' })).toBe(true);
  expect(working({ id: 'a', body: { state: 'running', ext: { 'wisp.history': { offer: true } } } })).toBe(true);
  expect(working({ id: 'a', state: 'waiting' })).toBe(true);
  expect(working({ id: 'a', state: 'done' })).toBe(false);
  expect(working({ id: 'a', state: 'review' })).toBe(false);
  expect(working(undefined)).toBe(false);
});

test('only_the_tasks_last_turn_shows_the_offer', () => {
  const rows = [
    { id: 'p:t1', body: { id: 't1', idx: 0, sessionId: 's1' } },
    { id: 'p:t3', body: { id: 't3', idx: 2, sessionId: 's1' } },
    { id: 'p:t2', body: { id: 't2', idx: 1, sessionId: 's1' } },
    { id: 'p:u9', body: { id: 'u9', idx: 9, sessionId: 's2' } },
    { id: 'other:t9', session_id: 's1', idx: 99 },
  ];
  expect(lastTurnOf(rows, 's1', 'p')).toBe('t3');
  expect(lastTurnOf([{ id: 'p:x', idx: 4, session_id: 'p:s3' }], 's3', 'p')).toBe('x');
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
  expect(saveStateOf(key)).toMatchObject({ s: 'saved', files: 4, repo: '/r', scrubbed: 2 });
  await runSave(() => Promise.reject(new Error('no')), 'local:b2' as EntityKey);
  expect(saveStateOf('local:b2')).toEqual({ s: 'failed' });
});

test('the_saved_line_says_how_many_files_and_secrets', () => {
  expect(t('history.saved', { count: 1 })).toBe('Saved to the repo: 1 file in .wisp/, to commit with the code');
  expect(t('history.saved.scrubbed', { count: 3, scrubbed: 2 })).toBe('Saved to the repo: 3 files in .wisp/ (2 scrubbed: keys, tokens, private paths)');
  expect(t('history.save')).toBe('Save to repo');
});

test('a_push_keeps_its_state_and_a_second_press_while_pushing_does_nothing', async () => {
  const key = 'local:p1' as EntityKey;
  let calls = 0;
  let finish: (v: Pushed) => void = () => {};
  const run = (): Promise<Pushed> => {
    calls++;
    return new Promise((r) => (finish = r));
  };
  const first = runPush(run, key);
  expect(pushStateOf(key)).toEqual({ s: 'pushing' });
  await runPush(run, key);
  expect(calls).toBe(1);
  finish({ branch: 'wisp/history-abc', commit: 'c', files: ['.wisp/a'], pr: 'https://x/pull/1' });
  await first;
  expect(pushStateOf(key)).toEqual({ s: 'pushed', branch: 'wisp/history-abc', pr: 'https://x/pull/1' });
  await runPush(() => Promise.reject(new Error('no remote')), 'local:p2' as EntityKey);
  expect(pushStateOf('local:p2')).toEqual({ s: 'failed' });
});

test('only_a_saved_task_can_be_pushed_from_the_menu', () => {
  expect(pushable({ id: 'a', ext: { 'wisp.history': { tracked: { files: ['.wisp/x'] } } } })).toBe(true);
  expect(pushable({ id: 'a', ext: { 'wisp.history': { tracked: { files: [] } } } })).toBe(false);
  expect(pushable({ id: 'a' })).toBe(false);
  expect(pushable({ id: 'a', parent_id: 'p', ext: { 'wisp.history': { tracked: { files: ['x'] } } } })).toBe(false);
  expect(t('history.pushed.pr', { branch: 'wisp/history-abc' })).toBe('Pushed to wisp/history-abc, pull request open');
  expect(t('history.push')).toBe('Push');
});

test('a_save_after_a_push_offers_push_again_and_a_failed_one_keeps_the_push', async () => {
  // final audit F2: the line kept saying "Pushed to …" (and hid Push) for the newer save
  const key = 'local:r1' as EntityKey;
  await runSave(() => Promise.resolve(tracked(2)), key);
  await runPush(() => Promise.resolve({ branch: 'wisp/history-r1', commit: 'c', files: [], pr: 'https://x/pull/1' }), key);
  expect(pushStateOf(key)).toMatchObject({ s: 'pushed', pr: 'https://x/pull/1' });
  await runSave(() => Promise.reject(new Error('no')), key);
  expect(pushStateOf(key), 'a refused save changed nothing on disk').toMatchObject({ s: 'pushed', branch: 'wisp/history-r1' });
  await runSave(() => Promise.resolve(tracked(3)), key);
  expect(pushStateOf(key)).toBeUndefined();
  // the slot still holds the earlier push: this window's newer save wins over it
  const ext = { pushed: { branch: 'wisp/history-r1', commit: 'c', pr: 'https://x/pull/1' }, tracked: { files: ['a', 'b', 'c'] } };
  expect(offerView(ext, saveStateOf(key), pushStateOf(key))).toMatchObject({ v: 'saved', files: 3 });
});

test('a_push_from_before_this_window_shows_from_the_tasks_slot', () => {
  // final audit F3: after a reload nothing said where the files went
  const ext = { tracked: { files: ['a', 'b'] }, pushed: { branch: 'wisp/history-x', commit: 'c', pr: 'https://github.com/a/b/pull/1' } };
  const v = offerView(ext, undefined);
  expect(v).toEqual({ v: 'kept', files: 2, pushed: { s: 'pushed', branch: 'wisp/history-x', pr: 'https://github.com/a/b/pull/1' } });
  expect(pushedText(v.v === 'kept' ? v.pushed : undefined)).toBe('Pushed to wisp/history-x, pull request open');
  // saved before, never pushed: nothing new, until a push from the menu runs
  expect(offerView({ tracked: { files: ['a'] } }, undefined)).toEqual({ v: 'none' });
  expect(offerView({ tracked: { files: ['a'] } }, undefined, { s: 'pushing' })).toEqual({ v: 'kept', files: 1 });
  expect(pushedOf({ pushed: { branch: '', commit: 'c', pr: null } })).toEqual({ s: 'pushed', branch: '' });
});

test('after_a_reload_a_save_after_a_push_shows_no_old_push', async () => {
  // final audit F2: save, push, save again, then a fresh window (or a reload). history.track drops `pushed` from the
  // slot, so the line has no old branch or PR to bring back for the newer, unpushed save
  const key = 'local:r2' as EntityKey;
  await runSave(() => Promise.resolve(tracked(2)), key);
  await runPush(() => Promise.resolve({ branch: 'wisp/history-r2', commit: 'c', files: [], pr: 'https://x/pull/2' }), key);
  const before = { tracked: { files: ['a', 'b'] }, pushed: { branch: 'wisp/history-r2', commit: 'c', pr: 'https://x/pull/2' } };
  expect(offerView(before, undefined)).toMatchObject({ v: 'kept', pushed: { s: 'pushed', pr: 'https://x/pull/2' } });
  await runSave(() => Promise.resolve(tracked(3)), key);
  // the slot as the save left it, read by a window with no save or push of its own
  const after = { tracked: { files: ['a', 'b', 'c'] }, offer: false };
  expect(offerView(after, undefined, undefined)).toEqual({ v: 'none' });
  expect(offerView({ ...after, pushed: null }, undefined, undefined)).toEqual({ v: 'none' });
  expect(pushedOf({ ...after, pushed: null })).toBeUndefined();
  // this window: the newer save, with Push (no push state) and no Open PR
  expect(offerView(after, saveStateOf(key), pushStateOf(key))).toMatchObject({ v: 'saved', files: 3 });
  expect(pushStateOf(key)).toBeUndefined();
  expect(pushedLink(pushStateOf(key))).toBeUndefined();
});

test('a_branch_pushed_with_no_pr_and_no_page_still_says_no_pr', async () => {
  // final audit F4: a path remote (no page, so no compare): the line still says no PR opened, with no button
  const p = { s: 'pushed', branch: 'wisp/history-x', noPr: true } as const;
  expect(pushedText(p)).toBe('Branch pushed to wisp/history-x: open the pull request');
  expect(pushedLink(p)).toBeUndefined();
  expect(pushedOf({ pushed: { branch: 'wisp/history-x', commit: 'c', pr: null, compare: null, noPr: true } })).toEqual(p);
  const key = 'local:r3' as EntityKey;
  await runPush(() => Promise.resolve({ branch: 'wisp/history-x', commit: 'c', files: [], noPr: true }), key);
  expect(pushedText(pushStateOf(key))).toBe('Branch pushed to wisp/history-x: open the pull request');
});

test('a_branch_pushed_with_no_pr_says_to_open_one', () => {
  // final audit F4: no gh, the files stay and the line links the page to open the PR on
  const p = { s: 'pushed', branch: 'wisp/history-x', compare: 'https://github.com/a/b/compare/main...wisp/history-x?expand=1' } as const;
  expect(pushedText(p)).toBe('Branch pushed to wisp/history-x: open the pull request');
  expect(pushedLink(p)).toEqual({ label: 'Open pull request', url: p.compare });
  expect(pushedText({ s: 'pushed', branch: 'wisp/x' })).toBe('Pushed to wisp/x');
  expect(pushedLink({ s: 'pushed', branch: 'wisp/x' })).toBeUndefined();
  expect(pushedLink({ s: 'pushed', branch: 'wisp/x', pr: 'https://x/pull/2' })).toEqual({ label: 'Open PR', url: 'https://x/pull/2' });
});
