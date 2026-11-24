// F-page-25 (v1 editor.js hoverOn): FileView asks onHover after the pointer rests 600 ms on a name, shows the words in
// a tip, drops an answer that comes after the pointer moved on, and hides on a key.
import { afterEach, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { FileView } from '../src/content/file/FileView.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = '';
  vi.useRealTimers();
  vi.restoreAllMocks();
});

test('resting_on_a_name_shows_the_language_servers_words_and_a_move_or_key_hides_them', async () => {
  vi.useFakeTimers();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ font: '', measureText: (s: string) => ({ width: s.length * 8 }) } as never);
  vi.stubGlobal('matchMedia', () => ({ matches: false }));
  const asked: { line: number; col: number }[] = [];
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <FileView text={'let squash = 1;'} lang="ts" aria-label="a.ts" onHover={(at) => (asked.push(at), Promise.resolve('```ts\nconst squash: 1\n```'))} />, host);
  const area = host.querySelector('textarea') as HTMLTextAreaElement;
  vi.spyOn(area, 'getBoundingClientRect').mockReturnValue({ left: 0, top: 0 } as DOMRect);
  // jsdom: no padding, line-height `normal` (20 px); x 8*5+1 is col 6 (the q of squash)
  area.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: 41, clientY: 5 }));
  await vi.advanceTimersByTimeAsync(599);
  expect(asked).toEqual([]);
  await vi.advanceTimersByTimeAsync(1);
  expect(asked).toEqual([{ line: 1, col: 6 }]);
  expect(host.querySelector('[data-kit="fv-hover"]')?.textContent).toBe('const squash: 1');
  area.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'a' }));
  expect(host.querySelector('[data-kit="fv-hover"]')).toBeNull();
  // on a blank: nothing asked
  area.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: 8 * 3 + 1, clientY: 5 }));
  await vi.advanceTimersByTimeAsync(700);
  expect(asked).toHaveLength(1);
});

// audit E #1: Open file… picked a file and left the focus on the page, so typing "e" marked the task done. The file view
// takes the focus as it opens (v1 editor.js), its caret at the top; without `autofocus` (a phone) it leaves it be.
test('a_file_view_that_autofocuses_takes_the_keys_with_its_caret_at_the_top', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: false }));
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <FileView text={'one\ntwo\n'} aria-label="a.ts" onInput={() => undefined} autofocus />, host);
  const area = host.querySelector('textarea') as HTMLTextAreaElement;
  expect(document.activeElement).toBe(area);
  expect(area.selectionStart).toBe(0);
  dispose();
  document.body.innerHTML = '';
  const again = document.createElement('div');
  document.body.append(again);
  dispose = render(() => <FileView text={'one'} aria-label="a.ts" />, again);
  expect(document.activeElement).toBe(document.body);
});

test('editing_one_line_keeps_unchanged_lines_dom_nodes_and_does_not_jump_caret_back_to_line', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: false }));
  const [text, setText] = createSignal('const a = 1;\nconst b = 2;\nconst c = 3;');
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <FileView text={text()} lang="ts" line={2} aria-label="a.ts" onInput={setText} />, host);
  const area = host.querySelector('textarea') as HTMLTextAreaElement;
  expect(area.selectionStart).toBe('const a = 1;\n'.length);
  const linesBefore = [...host.querySelectorAll('[data-kit="fv-line"]')];
  const tok1Before = linesBefore[0]?.querySelector('span');
  const tok3Before = linesBefore[2]?.querySelector('span');
  // real user typing on line 2: lines 1 and 3 keep their exact DOM nodes and the jump-to-line effect does not re-run
  area.value = 'const a = 1;\nconst b = 200;\nconst c = 3;';
  area.selectionStart = 25;
  area.selectionEnd = 25;
  const setSel = vi.spyOn(area, 'setSelectionRange');
  area.dispatchEvent(new Event('input', { bubbles: true }));
  const linesAfter = [...host.querySelectorAll('[data-kit="fv-line"]')];
  expect(linesAfter[0]?.querySelector('span')).toBe(tok1Before);
  expect(linesAfter[2]?.querySelector('span')).toBe(tok3Before);
  expect(linesAfter[1]?.textContent).toBe('const b = 200;');
  expect(setSel).not.toHaveBeenCalled();
  expect(area.selectionStart).toBe(25);
});
