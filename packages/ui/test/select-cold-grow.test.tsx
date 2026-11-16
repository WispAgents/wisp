// A phone Select pressed before its machine has loaded whose list grows past six before the machine lands
// (controls/Select.tsx): the machine never mounts to take the early press, so the press goes with the menu it was for.
// The button reads closed, and a list that shrinks back mounts the machine without replaying that stale press. Its own
// file: nothing else here has loaded the select machine (each test file gets its own modules).
import { expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { frame, mount } from './helpers.tsx';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';
import { Select } from '../src/controls/Select.tsx';
import { FEW, MANY } from './platform-scenes-controls.tsx';

const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);

test('a_select_pressed_before_its_machine_that_grows_into_a_sheet_first_drops_the_press', async () => {
  const [items, setItems] = createSignal<readonly (typeof MANY)[number][]>(FEW);
  mount(() => (
    <PlatformLookProvider look={() => 'android'}>
      <Select items={items()} value="codex" onChange={() => {}} aria-label="Model" />
    </PlatformLookProvider>
  ));
  const trigger = (): HTMLElement | null => q('[data-kit="select-trigger"]');
  trigger()?.click();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('true'); // in the press's frame, the machine not there yet
  setItems(MANY); // in the same frame: before the machine's chunk can land
  expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
  // the chunk lands and the list shrinks back: the machine mounts and binds, and replays nothing
  // check:allow dynamic-import-in-test waits on the lazy machine chunk the press asked for; a top import would preload it and lose the before-the-machine state
  await import('../src/controls/select-machine.tsx');
  setItems(FEW);
  await vi.waitFor(() => expect(q('[data-kit="listbox"]')).not.toBeNull());
  await frame(); // past the microtask a replay would run in
  expect(q('[data-kit="listbox"]')?.getAttribute('data-state')).toBe('closed');
  expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
});
