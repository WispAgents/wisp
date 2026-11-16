// Where this computer is named (owner 2026-11-08: "This computer", never "This Mac" whatever the OS): the names that
// follow the OS, and the static ones registered before the platform is known (r298).
import { expect, test } from 'vitest';
import en from '../src/i18n/en.json' with { type: 'json' };
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { readFileSync } from 'node:fs';
import { STATIC_NAME_KEY, machineLabelKey } from '../src/view.ts';

// the registrations read as source: importing index.ts compiles every component (test-imports rule) and its load
// counted against the test's 5 s under CI load (6f90a492)
const index = readFileSync(new URL('../src/index.ts', import.meta.url), 'utf8');

const all: Record<string, string> = { ...en, ...lazy };

test('the computer is named after its OS: This Mac, This PC, This computer elsewhere and when unknown_r298', () => {
  expect(all[machineLabelKey('darwin')]).toBe('This Mac');
  expect(all[machineLabelKey('win32')]).toBe('This PC');
  expect(all[machineLabelKey('linux')]).toBe('This computer');
  expect(all[machineLabelKey(undefined)]).toBe('This computer');
});

test('the settings pane title and the chip label never say This Mac before the OS is known_r298', () => {
  // static (registered before any platform is read): the first page's catalogue has them, so they never show a key
  expect(index).toMatch(/defineSettingsPane\(\{ id: 'machine',[^}]*title: STATIC_NAME_KEY/);
  expect(index).toMatch(/id: 'machine\.chip',[^\n]*bar: \{[^}]*label: STATIC_NAME_KEY/);
  expect(index).not.toContain('machine.name.mac');
  expect(STATIC_NAME_KEY).toBe('machine.name.computer');
  expect((en as Record<string, string>)['machine.name.computer']).toBe('This computer');
});
