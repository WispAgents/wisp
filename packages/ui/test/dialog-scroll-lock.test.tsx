// Perf audit 2026-11-18 #9: Zag's scroll lock writes `--scrollbar-width` on <html> (inherited: the whole document
// restyles) as a dialog opens and closes. wisp's page never scrolls, so a dialog there takes no lock; a page that does
// scroll keeps it.
import { afterEach, beforeAll, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, tick, until } from './helpers.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { pageScrolls } from '../src/controls/dialog-machine.ts';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());
afterEach(() => vi.restoreAllMocks());

const scrolling = (on: boolean): void => {
  const el = document.scrollingElement as HTMLElement;
  vi.spyOn(el, 'scrollHeight', 'get').mockReturnValue(on ? 2000 : 800);
  vi.spyOn(el, 'clientHeight', 'get').mockReturnValue(800);
};

async function openDialog(): Promise<void> {
  const [open, setOpen] = createSignal(false);
  mount(() => (
    <Dialog open={open()} onOpenChange={setOpen} title="Settings" part="t.dialog">
      <button type="button">ok</button>
    </Dialog>
  ));
  await tick(20);
  setOpen(true);
  await until(() => document.querySelector('[data-part~="t.dialog"]')?.getAttribute('data-state') === 'open');
  await tick(40);
}

test('pageScrolls: only a document taller than the window_dialog_page_scrolls', () => {
  scrolling(false);
  expect(pageScrolls()).toBe(false);
  vi.restoreAllMocks();
  scrolling(true);
  expect(pageScrolls()).toBe(true);
});

test('a page that never scrolls: opening a dialog writes nothing on <html> or <body>_dialog_no_lock', async () => {
  scrolling(false);
  await openDialog();
  expect(document.documentElement.style.getPropertyValue('--scrollbar-width')).toBe('');
  expect(document.body.hasAttribute('data-scroll-lock')).toBe(false);
  expect(document.body.style.overflow).toBe('');
});

test('a page that scrolls keeps the lock_dialog_lock_kept', async () => {
  scrolling(true);
  await openDialog();
  expect(document.body.hasAttribute('data-scroll-lock')).toBe(true);
});
