// A phone Select pressed before its machine has loaded (controls/Select.tsx, ./lazy-part.ts): the press shows the
// button open in its frame and the machine replays it once bound. That early press is the machine's from then on, so
// when the list later grows past six (the machine unmounts, the button goes back to its plain props) the button reads
// closed, as nothing is open. Its own file: nothing else here has loaded the select machine, so the press is truly
// before it (each test file gets its own modules).
import { expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';
import { Select } from '../src/controls/Select.tsx';
import { phoneOverlays } from '../src/controls/phone-part.ts';
import { FEW, MANY } from './platform-scenes-controls.tsx';

const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);

test('a_select_pressed_before_its_machine_reads_closed_once_it_grows_into_a_sheet', async () => {
  const [items, setItems] = createSignal<readonly (typeof MANY)[number][]>(FEW);
  mount(() => (
    <PlatformLookProvider look={() => 'android'}>
      <Select items={items()} value="codex" onChange={() => {}} aria-label="Model" />
    </PlatformLookProvider>
  ));
  const trigger = (): HTMLElement | null => q('[data-kit="select-trigger"]');
  trigger()?.click();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('true'); // in the press's frame, the machine not there yet
  await import('../src/controls/select-machine.tsx');
  await vi.waitFor(() => expect(q('[data-kit="listbox"]')?.getAttribute('data-state')).toBe('open'));
  // grown past six while its menu is open: the machine goes, and nothing is open
  setItems(MANY);
  expect(q('[data-kit="listbox"]')).toBeNull();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
  expect(trigger()?.getAttribute('data-state')).toBe('closed');
  // the next press opens the sheet; closing it leaves the button closed too
  trigger()?.click();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('true');
  await phoneOverlays.load();
  await vi.waitFor(() => expect(q('[data-kit="select-sheet"]')).not.toBeNull());
  setItems(FEW);
  expect(q('[data-kit="select-sheet"]')).toBeNull();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
});
