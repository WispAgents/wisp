// Audit A #18: in the Mac app the task's title (most of its head) was [data-no-drag]: the head's middle neither moved
// the window nor showed it could be renamed. The title is a drag-or-click region: a press that moves drags the window,
// a press that doesn't is a click (it edits the title in place).
import { expect, test } from 'vitest';
import { desktopPlatform } from '../src/platform/desktop.ts';
import type { PlatformWindow } from '../src/platform/types.ts';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const taskCss = readFileSync(join(import.meta.dirname, '..', '..', 'ui', 'src', 'css', 'app-task.css'), 'utf8');

function mac(calls: string[]): PlatformWindow {
  return {
    navigator: { platform: 'MacIntel', userAgent: 'Mac' } as Navigator,
    document,
    matchMedia: () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }) as unknown as MediaQueryList,
    addEventListener: () => {},
    removeEventListener: () => {},
    open: () => undefined,
    location: { reload: () => {}, origin: 'tauri://localhost' },
    __TAURI_INTERNALS__: { invoke: (cmd: string) => (calls.push(cmd), Promise.resolve(null)) },
  };
}

const mouse = (el: Element, type: string, x: number, y: number, detail = 1): boolean => el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, buttons: type === 'mouseup' ? 0 : 1, clientX: x, clientY: y, detail }));

test('the_titles_press_drags_the_window_when_it_moves_and_is_a_click_when_it_doesnt', () => {
  const calls: string[] = [];
  const root = document.createElement('div');
  root.innerHTML = '<header data-drag=""><h2 data-drag-click="">A title</h2><button>Ship</button></header>';
  document.body.append(root);
  const stop = desktopPlatform(mac(calls)).dragRegions?.(root) ?? (() => {});
  const title = root.querySelector('h2') as HTMLElement;
  // a press without moving: no drag, the click goes through (it renames)
  expect(mouse(title, 'mousedown', 100, 10), 'the press is not taken').toBe(true);
  mouse(title, 'mouseup', 100, 10);
  expect(calls).toEqual([]);
  // a press that moves: the window drags
  mouse(title, 'mousedown', 100, 10);
  mouse(document.body, 'mousemove', 108, 12);
  expect(calls).toEqual(['start_drag']);
  // the head's own region still drags at once; a button keeps its press
  mouse(root.querySelector('header') as HTMLElement, 'mousedown', 5, 5);
  expect(calls).toEqual(['start_drag', 'start_drag']);
  mouse(root.querySelector('button') as HTMLElement, 'mousedown', 300, 5);
  expect(calls).toHaveLength(2);
  stop();
  root.remove();
});

test('a_title_being_edited_looks_like_a_field', () => {
  expect(taskCss).toMatch(/\[data-kit='sh-title'\]\[contenteditable\][^{]*\{[^}]*box-shadow:/);
});

// review: while the title is being renamed a press selects text (no drag), and a press whose release the page never
// saw stops watching at the next move with the button up (no drag later, no listeners piling up)
test('an_edited_title_selects_and_a_lost_release_never_drags_later', () => {
  const calls: string[] = [];
  const root = document.createElement('div');
  root.innerHTML = '<header data-drag=""><h2 data-drag-click="">A title</h2></header>';
  document.body.append(root);
  const stop = desktopPlatform(mac(calls)).dragRegions?.(root) ?? (() => {});
  const title = root.querySelector('h2') as HTMLElement;
  Object.defineProperty(title, 'isContentEditable', { configurable: true, get: () => true });
  mouse(title, 'mousedown', 10, 10);
  document.body.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: 60, clientY: 10, buttons: 1 }));
  expect(calls, 'editing: no drag').toEqual([]);
  Object.defineProperty(title, 'isContentEditable', { configurable: true, get: () => false });
  // the release happened outside: the next move has no button down
  mouse(title, 'mousedown', 10, 10);
  document.body.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: 60, clientY: 10, buttons: 0 }));
  document.body.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: 90, clientY: 10, buttons: 1 }));
  expect(calls).toEqual([]);
  stop();
  root.remove();
});
