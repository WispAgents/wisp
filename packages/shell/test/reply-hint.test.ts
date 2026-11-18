// F-page-3 (v1 composer.js): one box answers. While the task asks, the reply box says so; on a live approval, typed
// words deny the call (runtime approval_decision), and the box says that before you type.
import { expect, test } from 'vitest';
import { replyPlaceholderKey } from '../src/frame/reply-hint.ts';

const row = (ask: unknown) => ({ state: 'waiting', body: { ext: { 'wisp.runtime': { ask } } } });

test('the_reply_box_answers_the_question_and_says_typing_denies_an_approval_v1_one_box', () => {
  expect(replyPlaceholderKey(undefined)).toBe('shell.task.reply');
  expect(replyPlaceholderKey({ state: 'review', body: {} })).toBe('shell.task.reply');
  expect(replyPlaceholderKey(row(null))).toBe('shell.task.reply'); // answered: the slot holds null
  expect(replyPlaceholderKey(row({ id: 'a1', question: 'Which DB?', options: ['Postgres', 'SQLite'] }))).toBe('shell.task.replyAnswer');
  expect(replyPlaceholderKey(row({ id: 'a2', question: 'Run npm publish?', permission: { tool: 'Bash', detail: 'npm publish', action: 'run npm publish' } }))).toBe('shell.task.replyDenies');
});

// UX 2026-11-18 desktop #1: a question's options sit above the reply box, and the box is the one other way to answer
// ("Answer in my own words" was a second control for it): it reads "Or answer in your own words…". A live approval's
// words still deny it.
test('the_reply_box_offers_own_words_while_the_latest_answer_asks_with_options', () => {
  expect(replyPlaceholderKey({ state: 'review', body: {} }, true)).toBe('shell.task.replyOwn');
  expect(replyPlaceholderKey({ state: 'review', body: {} }, false)).toBe('shell.task.reply');
  expect(replyPlaceholderKey(row({ id: 'a2', question: 'Run npm publish?', permission: { tool: 'Bash' } }), true)).toBe('shell.task.replyDenies');
});
