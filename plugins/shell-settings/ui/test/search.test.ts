// Audit A F11: Settings' search found panes by name only ("theme", "ntfy", "telemetry" found nothing; "xyzzy" left the
// old pane showing). It finds a pane by its rows' words too, and every declared setting by its label, description or
// key, each opening the pane that has it (else All settings, on that setting).
import { expect, test } from 'vitest';
import { foundSettings, settingTarget, type FoundSetting } from '../src/search.ts';
import { filterSections } from '../src/phone-nav.ts';

const groups = [
  { id: 'general', label: 'General', items: [{ id: 'general', label: 'General', icon: 'settings' as const }, { id: 'phone', label: 'Phone', icon: 'phone' as const }] },
  { id: 'advanced', label: 'Advanced', items: [{ id: 'privacy', label: 'Privacy', icon: 'info' as const }, { id: 'all', label: 'All settings', icon: 'settings' as const }] },
];
const words: Record<string, string> = { general: 'theme light dark editor', phone: 'ntfy telegram slack', privacy: 'telemetry usage stats' };

test('a_pane_is_found_by_its_rows_words_audit_a_f11', () => {
  const only = (q: string): string[] => filterSections(groups, q, (id) => words[id] ?? '').flatMap((g) => g.items.map((i) => i.id));
  expect(only('theme')).toEqual(['general']);
  expect(only('ntfy')).toEqual(['phone']);
  expect(only('telemetry')).toEqual(['privacy']);
  expect(only('xyzzy')).toEqual([]);
});

test('declared_settings_are_found_by_label_description_or_key_and_open_their_pane_audit_a_f11', () => {
  const all: FoundSetting[] = [
    { key: 'git.cloneDir', owner: 'wisp.git', label: 'Where a pasted repo URL is cloned', help: 'The first of ~/code…' },
    { key: 'channels.project', owner: 'wisp.channels', label: 'Default project', help: 'Where a task from a channel runs' },
    { key: 'budget.drainWatch', owner: 'wisp.budget', label: 'Drain watch', help: 'Tell the phone when a plan moves fast' },
  ];
  expect(foundSettings(all, 'clone').map((s) => s.key)).toEqual(['git.cloneDir']);
  expect(foundSettings(all, 'channel runs').map((s) => s.key)).toEqual(['channels.project']);
  expect(foundSettings(all, 'drainwatch').map((s) => s.key)).toEqual(['budget.drainWatch']);
  expect(foundSettings(all, '')).toEqual([]);
  // a plugin with one pane opens it; else All settings, filtered to the setting
  const panes = [{ id: 'phone', plugin: 'wisp.channels' }, { id: 'all', plugin: 'wisp.settings' }];
  expect(settingTarget({ key: 'channels.project', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'phone' });
  expect(settingTarget({ key: 'git.cloneDir', owner: 'wisp.git' }, panes)).toEqual({ pane: 'all', filter: 'git.cloneDir' });
});
