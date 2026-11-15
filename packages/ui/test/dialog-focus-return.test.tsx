// Focus goes back to what opened a Dialog the moment it closes (design/feedback.md: within the frame), whether the
// machine was there when it opened or landed after (controls/lazy-part.ts). Zag's focus trap remembers what was
// focused when it starts (a frame after opening), and a dialog opened before its machine has moved focus into itself
// by then; and it gives focus back on a timer, so for that while focus sat on <body> (CI on a slow Mac saw it there).
// The first test is the first to use the dialog's machine in this file, so its "before the machine" is real.
import { describe, expect, test } from 'vitest';
import { frame, mount, part, tick } from './helpers.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { Popover } from '../src/controls/Popover.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';

const key = (el: Element, k: string): void => {
  el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};

const flush = async (): Promise<void> => {
  await import('../src/controls/dialog-machine.ts');
  await tick(10);
};

describe('Dialog focus return', () => {
  test('opened before its machine landed: Escape gives focus back to the trigger, not <body>', async () => {
    mount(() => <Dialog title="Rename" trigger={{ label: 'Rename…' }} part="t.rename">body</Dialog>);
    const trigger = part('t.rename.trigger');
    trigger?.focus();
    trigger?.click();
    await flush();
    await frame();
    const content = part('t.rename');
    expect(content?.contains(document.activeElement)).toBe(true);
    key(document.activeElement ?? document.body, 'Escape');
    await tick(10);
    expect(part('t.rename')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  test('focus is on the trigger as the dialog goes, before any timer', async () => {
    await preloadControls();
    mount(() => <Dialog title="Delete" trigger={{ label: 'Delete…' }} part="t.delete">body</Dialog>);
    const trigger = part('t.delete.trigger');
    trigger?.focus();
    trigger?.click();
    await tick(10);
    await frame();
    expect(part('t.delete')?.contains(document.activeElement)).toBe(true);
    key(document.activeElement ?? document.body, 'Escape');
    // Zag answers the key in a microtask: the surface is gone after it and focus is back with it, before any timer
    await Promise.resolve();
    expect(part('t.delete')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  test('a Popover drawn as a phone sheet gives focus back to its trigger the same way', async () => {
    await preloadControls();
    mount(() => (
      <PlatformLookProvider look={() => 'android'}>
        <Popover trigger={{ label: 'Filter' }} title="Filter" part="t.pop">
          body
        </Popover>
      </PlatformLookProvider>
    ));
    const trigger = part('t.pop.trigger');
    trigger?.focus();
    trigger?.click();
    await tick(10);
    await frame();
    expect(part('t.pop')?.getAttribute('data-kit')).toBe('sheet');
    expect(part('t.pop')?.contains(document.activeElement)).toBe(true);
    key(document.activeElement ?? document.body, 'Escape');
    // a controlled sheet: it goes once the Popover's open state reaches it
    await tick(10);
    expect(part('t.pop')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});
