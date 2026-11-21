// QA desktop r2 #22: the Machines dialog opened with a focus ring on its × after a mouse click: on desktop focus went
// to the first button (the close). With nothing of the caller's to focus, it lands on the dialog itself, as it did on
// phones already (QA round 2 #14): no ring on ×, and a screen reader reads the dialog from its title.
import { expect, test } from 'vitest';
import { frame, mount, part, tick } from './helpers.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';

test('a_dialog_opens_with_focus_on_itself_not_its_close_button', async () => {
  await preloadControls();
  mount(() => <Dialog title="Machines" trigger={{ label: 'Machines' }} part="t.machines">body</Dialog>);
  part('t.machines.trigger')?.click();
  await tick(10);
  await frame();
  await tick(10);
  const content = part('t.machines');
  expect(content?.contains(document.activeElement)).toBe(true);
  expect(document.activeElement?.tagName, 'not a button (the ×)').not.toBe('BUTTON');
});

// CI (2026-11-18): an uncaught "Your focus-trap needs to have at least one focusable element": after a close, the
// dialog still offered the last open's content element (gone from the page) as the first focus; the trap refused it
// and found nothing else. Opened again, focus lands on the new content and nothing throws.
test('a_dialog_opened_again_focuses_its_new_content_never_the_last_ones', async () => {
  await preloadControls();
  const thrown: unknown[] = [];
  const onError = (e: ErrorEvent): void => void thrown.push(e.error ?? e.message);
  window.addEventListener('error', onError);
  mount(() => <Dialog title="Machines" trigger={{ label: 'Machines' }} part="t.again">body</Dialog>);
  for (let i = 0; i < 2; i++) {
    part('t.again.trigger')?.click();
    await tick(10);
    await frame();
    await tick(10);
    expect(part('t.again')?.contains(document.activeElement), `open ${i + 1}`).toBe(true);
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick(10);
    await frame();
  }
  window.removeEventListener('error', onError);
  expect(thrown).toEqual([]);
});
