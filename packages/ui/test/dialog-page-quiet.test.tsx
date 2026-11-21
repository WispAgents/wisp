// perf-ui-r2 #6: opening ⌘K or a dialog restyled the whole document (1,526–1,792 elements): Zag's modal dialog blocks
// the page's pointer with `pointer-events: none` on <body>, an inherited property. The kit's dialog now writes no style
// on <body>, and keeps what modal meant: the page outside aria-hidden (screen readers read the dialog only), focus
// trapped inside, aria-modal, a press outside closing it, all undone as it closes.
import { beforeAll, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { frame, mount, part, tick, until } from './helpers.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { hideOutside } from '../src/controls/dialog-machine.ts';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());

test('opening_a_dialog_writes_no_style_on_body_and_hides_the_page_from_screen_readers_until_it_closes', async () => {
  const [open, setOpen] = createSignal(false);
  mount(() => (
    <>
      <main data-testid="app">
        <button type="button">behind</button>
      </main>
      <Dialog open={open()} onOpenChange={setOpen} title="Settings" part="t.quiet">
        <button type="button">ok</button>
      </Dialog>
    </>
  ));
  await tick(20);
  const app = document.querySelector('[data-testid="app"]')?.parentElement as HTMLElement;
  setOpen(true);
  await until(() => part('t.quiet')?.getAttribute('data-state') === 'open');
  await frame();
  await frame();
  expect(document.body.getAttribute('style') ?? '', 'no pointer-events (inherited) on <body>').not.toContain('pointer-events');
  expect(app.getAttribute('aria-hidden'), 'the page is hidden from screen readers').toBe('true');
  const content = part('t.quiet') as HTMLElement;
  expect(content.getAttribute('aria-modal')).toBe('true');
  expect(content.closest('[aria-hidden="true"]'), 'the dialog itself is read').toBeNull();
  expect(content.contains(document.activeElement), 'focus moved in').toBe(true);
  // the trap: Tab from the last control stays inside
  (content.querySelector('button:last-of-type') as HTMLElement).focus();
  document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
  await tick(10);
  expect(content.contains(document.activeElement)).toBe(true);
  // a press outside the content closes it, as a modal one did
  const outside = document.querySelector('[data-kit="dialog-positioner"]') as HTMLElement;
  expect(outside.style.pointerEvents, 'the positioner takes the press').toBe('');
  setOpen(false);
  await until(() => part('t.quiet') === null);
  await frame();
  expect(app.hasAttribute('aria-hidden'), 'the page is read again').toBe(false);
  expect(app.hasAttribute('data-aria-hidden')).toBe(false);
});

test('two_dialogs_hide_the_page_until_both_close_and_what_was_hidden_before_stays_hidden', () => {
  const root = document.createElement('div');
  const pre = document.createElement('div');
  pre.setAttribute('aria-hidden', 'true');
  const live = document.createElement('div');
  live.setAttribute('aria-live', 'polite');
  const a = document.createElement('div');
  const b = document.createElement('div');
  document.body.append(root, pre, live, a, b);
  const undoA = hideOutside(a);
  expect([root, b].map((x) => x.getAttribute('aria-hidden'))).toEqual(['true', 'true']);
  expect(live.hasAttribute('aria-hidden'), 'a live region still speaks').toBe(false);
  const undoB = hideOutside(b);
  expect(a.getAttribute('aria-hidden'), 'the first dialog under the second').toBe('true');
  undoB();
  expect(root.getAttribute('aria-hidden'), 'still hidden: the first is open').toBe('true');
  expect(a.hasAttribute('aria-hidden')).toBe(false);
  undoA();
  expect(root.hasAttribute('aria-hidden')).toBe(false);
  expect(pre.getAttribute('aria-hidden'), 'hidden before any dialog: left so').toBe('true');
  for (const el of [root, pre, live, a, b]) el.remove();
});
