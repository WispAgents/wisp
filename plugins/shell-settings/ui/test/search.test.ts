// Audit A F11: Settings' search found panes by name only ("theme", "ntfy", "telemetry" found nothing; "xyzzy" left the
// old pane showing). It finds a pane by its rows' words too, and every declared setting by its label, description or
// key, each opening the pane that has it (else All settings, on that setting).
import { expect, test } from 'vitest';
import { foundHarnesses, foundSettings, harnessTarget, settingTarget, type FoundSetting, type RoutePane } from '../src/search.ts';
import { filterSections } from '../src/phone-nav.ts';
// Agents' words, as its plugin's catalogue has them (read as text, as the manifests are: no plugin code)
const accountsCatalogue = import.meta.glob<string>('../../../accounts/ui/src/i18n/en.json', { query: '?raw', import: 'default', eager: true });
const accountsWords = JSON.parse(Object.values(accountsCatalogue)[0] ?? '{}') as Record<string, string>;

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
  // a plugin with one pane that doesn't say what it shows opens it; else All settings, filtered to the setting
  const panes = [{ id: 'phone', plugin: 'wisp.channels' }, { id: 'all', plugin: 'wisp.settings' }];
  expect(settingTarget({ key: 'channels.project', owner: 'wisp.channels' }, panes)).toEqual({ pane: 'phone', focus: 'channels.project' });
  expect(settingTarget({ key: 'git.cloneDir', owner: 'wisp.git' }, panes)).toEqual({ pane: 'all', filter: 'git.cloneDir' });
});

// Settings audit r1: the search opened a setting on its plugin's one pane, whatever pane had its row: channels.project
// opened Phone (its row is in Notifications), worktrees.projectAsk Worktrees (Privacy has it), worktrees.worktree
// Worktrees (General has it). Each pane says which settings it has a row for; the search opens the one that does, on
// that row, and All settings (filtered) for a setting no pane shows.
const paneKeys = import.meta.glob<Record<string, readonly string[]>>('../../../*/ui/src/pane-keys.ts', { import: 'PANE_KEYS', eager: true });
const indexes = import.meta.glob<string>(['../../../*/ui/src/index.ts', '../../../*/ui/src/index.tsx'], { query: '?raw', import: 'default', eager: true });
const manifests = import.meta.glob<string>('../../../*/host/wisp-plugin.json', { query: '?raw', import: 'default', eager: true });
const dirOf = (path: string): string => path.split('/')[3] ?? '';
const owners = new Map(Object.entries(manifests).map(([p, text]) => [dirOf(p), (JSON.parse(text) as { id: string }).id]));
/** every registered pane (its id in its plugin's index), its plugin and the settings it says it shows */
function realPanes(): RoutePane[] {
  const out: RoutePane[] = [];
  for (const [path, text] of Object.entries(indexes)) {
    const dir = dirOf(path);
    const keys = Object.entries(paneKeys).find(([p]) => dirOf(p) === dir)?.[1];
    for (const m of text.matchAll(/\{\s*id:\s*'([^']+)',\s*group:\s*'(?:general|agents|usage|integrations|computers|advanced)'/g)) {
      const id = m[1] as string;
      out.push({ id, plugin: owners.get(dir) ?? `wisp.${dir}`, keys: keys?.[id] });
    }
  }
  return out;
}

test('a_setting_opens_on_the_pane_that_has_its_row_and_lands_on_it_settings_audit_r1', () => {
  const panes = realPanes();
  expect(panes.find((p) => p.id === 'agents')?.keys).toContain('runtime.leanRuns');
  const at = (key: string, owner: string) => settingTarget({ key, owner }, panes);
  expect(at('channels.project', 'wisp.channels')).toEqual({ pane: 'notifications', focus: 'channels.project' });
  expect(at('channels.acceptChannels', 'wisp.channels')).toEqual({ pane: 'notifications', focus: 'channels.acceptChannels' });
  expect(at('worktrees.projectAsk', 'wisp.worktrees')).toEqual({ pane: 'privacy', focus: 'worktrees.projectAsk' });
  expect(at('worktrees.worktree', 'wisp.worktrees')).toEqual({ pane: 'general', focus: 'worktrees.worktree' });
  expect(at('runtime.leanRuns', 'wisp.runtime')).toEqual({ pane: 'agents', focus: 'runtime.leanRuns' });
  expect(at('accounts.updateCheck', 'wisp.accounts')).toEqual({ pane: 'agents', focus: 'accounts.updateCheck' });
  // no pane has its row: All settings, on it (never the Machine pane, which hasn't)
  expect(at('machine.batteryHold', 'wisp.machine')).toEqual({ pane: 'all', filter: 'machine.batteryHold' });
  // the owner's one pane said nothing (a plugin of yours): it opens there, as before
  expect(settingTarget({ key: 'mine.x', owner: 'user.mine' }, [{ id: 'mine', plugin: 'user.mine' }])).toEqual({ pane: 'mine', focus: 'mine.x' });
});

test('every_declared_setting_opens_on_a_pane_that_lists_it_or_on_all_settings_settings_audit_r1', () => {
  const panes = realPanes();
  expect(panes.length).toBeGreaterThan(15);
  const wrong: string[] = [];
  for (const [path, text] of Object.entries(manifests)) {
    const m = JSON.parse(text) as { id: string; contributes?: { settings?: { key: string }[] } };
    for (const s of m.contributes?.settings ?? []) {
      const to = settingTarget({ key: s.key, owner: m.id }, panes);
      const ok = to.pane === 'all' ? to.filter === s.key : panes.find((p) => p.id === to.pane)?.keys?.includes(s.key) === true;
      if (!ok) wrong.push(`${s.key} (${dirOf(path)}) → ${to.pane}`);
    }
  }
  expect(wrong).toEqual([]);
});

// Settings audit r1: "codex" found nothing (Agents had no words, and no harness was a result). Each harness is a result
// of its own, opening Settings → Agents on it; Agents is found by what its rows do.
test('a_harness_is_found_by_name_and_agents_by_its_words_settings_audit_r1', () => {
  const hs = [
    { id: 'claude', name: 'Claude Code' },
    { id: 'codex', name: 'Codex' },
  ];
  expect(foundHarnesses(hs, 'codex').map((h) => h.id)).toEqual(['codex']);
  expect(foundHarnesses(hs, 'claude code').map((h) => h.id)).toEqual(['claude']);
  expect(foundHarnesses(hs, '')).toEqual([]);
  expect(harnessTarget('codex')).toEqual({ pane: 'agents', filter: 'harness:codex' });
  const agents = [{ id: 'agents', label: 'Agents', items: [{ id: 'agents', label: 'Agents', icon: 'agent' as const }] }, ...groups];
  const about = (id: string): string => (id === 'agents' ? accountsWords['accounts.pane.keywords'] ?? '' : (words[id] ?? ''));
  const only = (q: string): string[] => filterSections(agents, q, about).flatMap((g) => g.items.map((i) => i.id));
  expect(only('api key')).toContain('agents');
  expect(only('sign in')).toContain('agents');
  expect(only('install')).toContain('agents');
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
