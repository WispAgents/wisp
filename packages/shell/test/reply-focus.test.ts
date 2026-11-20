// Audit H tasks, F #1 (partly fixed): a deep link to a task's Shell gave the terminal the keys, then about 3 s later
// (WebKit, a loaded machine) the task's body finished building and the reply box took the caret: `echo z` went there.
import { afterEach, expect, test } from 'vitest';
import { mayTakeCaret } from '../src/frame/reply-focus.ts';

afterEach(() => void (document.body.innerHTML = ''));

test('the_reply_box_never_takes_the_caret_from_the_side_panel_or_a_page_opened_on_the_shell_audit_h_f1', () => {
  document.body.innerHTML = `
    <aside data-pane="side"><div data-part="side.panel panel"><button role="tab">Shell</button><textarea aria-label="Terminal input"></textarea></div></aside>
    <input id="search" /><div contenteditable="true" id="ed"></div>
    <button id="row">a row</button>`;
  const q = (s: string): Element => document.querySelector(s)!;
  expect(mayTakeCaret(q('textarea'), document), 'the deep-linked Shell keeps the keys').toBe(false);
  expect(mayTakeCaret(q('[role=tab]'), document), 'a side panel tab keeps them').toBe(false);
  // a field outside the panel (the list's search a key opened the task from): as before, the reply box takes it
  expect(mayTakeCaret(q('#search'), document)).toBe(true);
  expect(mayTakeCaret(q('#ed'), document)).toBe(true);
  // as before: nothing focused, or the list row a key opened the task from
  expect(mayTakeCaret(document.body, document)).toBe(true);
  expect(mayTakeCaret(null, document)).toBe(true);
  expect(mayTakeCaret(q('#row'), document)).toBe(true);
  // opened on the Shell: the Shell takes the keys when it draws, the reply box never does (WebKit took them first)
  expect(mayTakeCaret(document.body, document, 'shell')).toBe(false);
  expect(mayTakeCaret(document.body, document, 'changes')).toBe(true);
});
