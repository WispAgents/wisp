// Setup audit B13: Settings → Plugins → Trust applied at once, and Remove took a plugin out with no confirm and no Undo,
// while connectors, machines, accounts and API keys all ask. Trust asks first and names what the plugin gets; Remove
// asks first in v1's words; Untrust only takes away, so it doesn't ask.
import { expect, test } from 'vitest';
import { confirmOf } from '../src/confirm.ts';
import en from '../src/i18n/en.json';
import lazy from '../src/i18n/en.lazy.json';

const all = { ...en, ...lazy } as Record<string, string>;
const say = (k: string, v?: Readonly<Record<string, string>>): string => (all[k] ?? k).replace(/\{(\w+)\}/g, (_m, n: string) => v?.[n] ?? '');
const mine = { id: 'user.qa-pink', name: 'QA Pink', version: '1.0.0', state: 'active', enabled: true, builtin: false, trusted: false, permissions: [] };

test('trust_and_remove_ask_first_and_untrust_does_not_setup_audit_b13', () => {
  const trust = confirmOf('trust', mine, say);
  expect(trust?.title).toBe('Trust QA Pink?');
  expect(trust?.detail).toContain('your environment');
  expect(trust?.detail).toContain('your files');
  expect(trust?.ok).toBe('Trust it');
  const remove = confirmOf('remove', mine, say);
  expect(remove?.title).toBe('Remove QA Pink?');
  expect(remove?.detail).toContain('Turns it off and deletes it');
  expect(remove?.ok).toBe('Remove');
  expect(confirmOf('untrust', { ...mine, trusted: true }, say)).toBeUndefined();
  for (const k of ['plugins.trustAsk.title', 'plugins.trustAsk.detail', 'plugins.removeAsk.detail', 'plugins.cancel']) expect(all[k], k).toBeTruthy();
});
