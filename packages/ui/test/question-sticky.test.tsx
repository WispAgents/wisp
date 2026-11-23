// The `question` block ("Needs your call") renders in chat flow at the end of the turn (not sticky/fixed over the
// answer); Answer in my own words is gone (the reply box says it); keys 1–9 pick an option while the reply box is
// empty, never while you type somewhere.
import { expect, test } from 'vitest';
import css from '../src/css/answer-rich.css?raw';
import { mount, part } from './helpers.tsx';
import { Answer } from '../src/index.ts';

const key = (k: string, o: KeyboardEventInit = {}): void => void document.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, ...o }));

test('a_live_question_stays_in_chat_flow_not_sticky', () => {
  expect(css).not.toMatch(/\[data-type='question'\][^}]*position:\s*sticky/);
});

test('the_question_has_no_own_words_chip_and_keys_pick_its_options_while_the_reply_box_is_empty', () => {
  const said: string[] = [];
  mount(() => (
    <>
      <Answer ctx={{ latest: true, reply: (t) => void said.push(t), draft: () => {} }} blocks={[{ type: 'question', question: 'Which?', options: ['Postgres', 'SQLite', 'Both'] }]} />
      <div data-kit="reply-box">
        <textarea />
      </div>
      <input data-test="other" />
    </>
  ));
  expect(part('block.question')?.querySelector('[data-own]')).toBeNull();
  expect(part('block.question')?.textContent).not.toContain('own words');
  key('2');
  expect(said).toEqual(['SQLite']);
  key('9'); // no such option
  key('1', { metaKey: true }); // ⌘1 is the app's
  expect(said).toEqual(['SQLite']);
  const box = document.querySelector('textarea') as HTMLTextAreaElement;
  box.focus();
  key('3');
  expect(said).toEqual(['SQLite', 'Both']);
  box.value = 'well';
  key('1');
  (document.querySelector('[data-test="other"]') as HTMLInputElement).focus();
  key('1');
  expect(said).toEqual(['SQLite', 'Both']);
});

test('an_earlier_question_takes_no_keys', () => {
  const said: string[] = [];
  mount(() => <Answer ctx={{ latest: false, reply: (t) => void said.push(t) }} blocks={[{ type: 'question', question: 'Which?', options: ['A', 'B'] }]} />);
  key('1');
  expect(said).toEqual([]);
});

test('number_keys_leave_an_open_menu_or_dialog_alone', () => {
  const said: string[] = [];
  mount(() => (
    <>
      <Answer ctx={{ latest: true, reply: (t) => void said.push(t) }} blocks={[{ type: 'question', question: 'Which?', options: ['A', 'B'] }]} />
      <div role="menu">
        <button type="button" role="menuitem" data-test="item">
          x
        </button>
      </div>
    </>
  ));
  (document.querySelector('[data-test="item"]') as HTMLButtonElement).focus();
  key('1');
  key('2', { isComposing: true } as KeyboardEventInit);
  expect(said).toEqual([]);
});
