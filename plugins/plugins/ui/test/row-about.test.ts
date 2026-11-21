// Audit A F19: Settings → Plugins had no Trust (ADR-0048), never said what a plugin of yours asks for, and showed ids
// (`user.qa-pink`) not names. A row says its name, its version, whether it's trusted and what that means, and what
// it asks for; a built-in says only its id and version.
import { expect, test } from 'vitest';
import { rowAbout } from '../src/commands.ts';
import en from '../src/i18n/en.json';
import lazy from '../src/i18n/en.lazy.json';

const all = { ...en, ...lazy } as Record<string, string>;
const say = (k: string, v?: Readonly<Record<string, string>>): string => (all[k] ?? k).replace(/\{(\w+)\}/g, (_m, n: string) => v?.[n] ?? '');

test('a_plugin_of_yours_says_its_name_its_trust_and_what_it_asks_for_audit_a_f19', () => {
  const mine = { id: 'user.qa-pink', name: 'QA Pink', version: '1.0.0', state: 'active', enabled: true, builtin: false, trusted: false, permissions: ['settings:read'] };
  const a = rowAbout(mine, say);
  expect(a.title).toBe('QA Pink');
  expect(a.lines).toEqual(['Version 1.0.0 · user.qa-pink', 'Untrusted: it runs fenced in, and its styles don’t apply until you trust it.', 'It may: read your settings']);
  expect(a.trust).toEqual({ label: 'Trust', next: true });
  const trusted = rowAbout({ ...mine, trusted: true, permissions: [] }, say);
  expect(trusted.lines[1]).toBe('Trusted: it runs as its own process with your environment, and its styles apply.');
  expect(trusted.trust).toEqual({ label: 'Untrust', next: false });
  const builtin = rowAbout({ id: 'wisp.claude', version: '1.0.0-alpha.1', state: 'active', enabled: true, builtin: true }, say);
  expect([builtin.title, builtin.lines, builtin.trust]).toEqual(['claude', ['Version 1.0.0-alpha.1'], undefined]);
});
