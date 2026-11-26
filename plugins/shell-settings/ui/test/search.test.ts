// Audit A F11: Settings' search found panes by name only ("theme", "ntfy", "telemetry" found nothing; "xyzzy" left the
// old pane showing). It finds a pane by its rows' words too, and every declared setting by its label, description or
// key, each opening the pane that has it (else All settings, on that setting).
import { expect, test } from 'vitest';
import { foundSettings, harnessOfKey, harnessRows, PANE_OF_KEY, settingTarget, type FoundSetting } from '../src/search.ts';
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
  expect(settingTarget({ key: 'channels.relay', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'phone' });
  expect(settingTarget({ key: 'git.cloneDir', owner: 'wisp.git' }, panes)).toEqual({ pane: 'all', filter: 'git.cloneDir' });
});

// Setup audit B6: with Auto put away (runtime.auto off), typing "auto" or "model" listed Auto's switch, and opening it
// showed All settings with "No setting matches." (All settings hides that row). The search never offers it either.
test('autos_switch_is_never_a_search_hit_setup_audit_b6', () => {
  const all: FoundSetting[] = [
    { key: 'runtime.auto', owner: 'wisp.runtime', label: 'Auto', help: 'Auto picks the harness and model' },
    { key: 'runtime.subagentModel', owner: 'wisp.runtime', label: 'Subagent model', help: 'The model Claude Code subagents run on' },
  ];
  expect(foundSettings(all, 'auto').map((s) => s.key)).toEqual([]);
  expect(foundSettings(all, 'model').map((s) => s.key)).toEqual(['runtime.subagentModel']);
});

// Harness audit r2: Settings → Agents is a list and a detail; the search finds a harness's rows ("cursor model") and
// a setting about one harness (budget.autoStopCursor), and each opens Agents on that harness's detail.
test('a_harness_row_is_found_and_opens_agents_on_that_harness', () => {
  const rows = harnessRows([{ id: 'claude', name: 'Claude Code' }, { id: 'cursor', name: 'Cursor' }], ['Sign in and accounts', 'Default model']);
  expect(foundSettings(rows, 'cursor model').map((r) => r.label)).toEqual(['Cursor: Default model']);
  const panes = [{ id: 'agents', plugin: 'wisp.accounts' }, { id: 'usage', plugin: 'wisp.budget' }, { id: 'all', plugin: 'wisp.settings' }];
  const hit = foundSettings(rows, 'cursor model')[0];
  expect(hit && settingTarget(hit, panes, ['claude', 'cursor'])).toEqual({ pane: 'agents', filter: 'cursor' });
  // a declared setting about one harness goes to its detail too; one about none stays where it was
  expect(settingTarget({ key: 'budget.autoStopCursor', owner: 'wisp.budget' }, panes, ['claude', 'cursor'])).toEqual({ pane: 'agents', filter: 'cursor' });
  expect(settingTarget({ key: 'budget.autoStop', owner: 'wisp.budget' }, panes, ['claude', 'cursor'])).toEqual({ pane: 'usage' });
  expect(harnessOfKey('runtime.cursor', ['cursor']), 'a key that is only the name is no row of it').toBeUndefined();
});

// doc924a: channels.acceptChannels and channels.project are wisp.channels' (whose one pane is Phone), but Notifications
// draws them; General's and Chat's rows are other plugins' too and went to All settings. Each opens the pane that has it.
test('a_setting_opens_the_pane_that_draws_its_row_whoever_declares_it', () => {
  const panes = [
    { id: 'general', plugin: 'wisp.shell-settings' },
    { id: 'chat', plugin: 'wisp.shell-settings' },
    { id: 'notifications', plugin: 'wisp.shell-settings' },
    { id: 'keys', plugin: 'wisp.shell-settings' },
    { id: 'phone', plugin: 'wisp.channels' },
    { id: 'agents', plugin: 'wisp.accounts' },
    { id: 'usage', plugin: 'wisp.budget' },
    { id: 'all', plugin: 'wisp.settings' },
  ];
  expect(settingTarget({ key: 'channels.acceptChannels', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'notifications' });
  expect(settingTarget({ key: 'channels.project', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'notifications' });
  expect(settingTarget({ key: 'settings.editor', owner: 'wisp.settings' }, panes)).toEqual({ pane: 'general' });
  expect(settingTarget({ key: 'crew.tint', owner: 'wisp.crew' }, panes)).toEqual({ pane: 'general' });
  expect(settingTarget({ key: 'settings.chatWidth', owner: 'wisp.settings' }, panes)).toEqual({ pane: 'chat' });
  // a key no pane of this plugin draws: its owner's one pane, else All settings
  expect(settingTarget({ key: 'channels.relay', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'phone' });
  expect(settingTarget({ key: 'git.cloneDir', owner: 'wisp.git' }, panes)).toEqual({ pane: 'all', filter: 'git.cloneDir' });
  // the Agents rules come first, as before
  expect(settingTarget({ key: 'budget.autoStopCursor', owner: 'wisp.budget' }, panes, ['cursor'])).toEqual({ pane: 'agents', filter: 'cursor' });
  // the drawing pane missing (a phone without it): owner routing as before
  expect(settingTarget({ key: 'channels.acceptChannels', owner: 'wisp.channels' }, panes.filter((p) => p.id !== 'notifications'))).toEqual({ pane: 'phone' });
  expect(PANE_OF_KEY.size).toBe(22);
});
