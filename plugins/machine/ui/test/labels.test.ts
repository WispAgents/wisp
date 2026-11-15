// Where this computer is named (owner 2026-11-08: "This computer", never "This Mac" whatever the OS): the names that
// follow the OS, and the static ones registered before the platform is known (r298).
import { expect, test } from 'vitest';
import en from '../src/i18n/en.json' with { type: 'json' };
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { machineLabelKey } from '../src/view.ts';
// imported once at the top: inside the test, loading the plugin's whole UI counted against its 5 s (CI load, 6f90a492)
import ui from '../src/index.ts';

const all: Record<string, string> = { ...en, ...lazy };

test('the computer is named after its OS: This Mac, This PC, This computer elsewhere and when unknown_r298', () => {
  expect(all[machineLabelKey('darwin')]).toBe('This Mac');
  expect(all[machineLabelKey('win32')]).toBe('This PC');
  expect(all[machineLabelKey('linux')]).toBe('This computer');
  expect(all[machineLabelKey(undefined)]).toBe('This computer');
});

test('the settings pane title and the chip label never say This Mac before the OS is known_r298', () => {
  const pane = ui.settings?.find((s) => s.id === 'machine');
  const chip = ui.extensions?.find((x) => x.id === 'machine.chip');
  // static (registered before any platform is read): the first page's catalogue has them, so they never show a key
  expect(pane?.title).toBe('machine.name.computer');
  expect(chip?.bar?.label).toBe('machine.name.computer');
  expect((en as Record<string, string>)['machine.name.computer']).toBe('This computer');
});
