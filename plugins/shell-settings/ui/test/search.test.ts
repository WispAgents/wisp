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
// a glob's hits in this package come back as `../src/…` (Vite shortens them), the others' as `../../../<dir>/…`
const dirOf = (path: string): string => (path.startsWith('../src/') ? 'shell-settings' : (path.split('/')[3] ?? ''));
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

// Settings audit r1 review: a pane listed settings it drew no landable row for (Chat, Usage, Worktrees, Updates,
// Memory, Automations, General's terminal rows): the search opened the pane at its top, nothing flashed or focused.
// Every key a pane lists has a row with that `settingKey`, in the pane's plugin or in a plugin that draws rows into a
// Settings slot (wisp.machine's load rows in Usage, wisp.threads' Check compactions in General).
const sources = import.meta.glob<string>(['../../../*/ui/src/**/*.ts', '../../../*/ui/src/**/*.tsx'], { query: '?raw', import: 'default', eager: true });

/** The settings each plugin dir draws a landable row for (`settingKey=` read from its source, its constants resolved). */
function rowsByDir(): Map<string, Set<string>> {
  const byDir = new Map<string, string[]>();
  for (const [path, text] of Object.entries(sources)) byDir.set(dirOf(path), [...(byDir.get(dirOf(path)) ?? []), text]);
  const out = new Map<string, Set<string>>();
  for (const [dir, texts] of byDir) {
    const all = texts.join('\n');
    const resolve = (expr: string, file: string, depth = 0): string[] => {
      const e = expr.trim();
      const lit = /^['"]([^'"]+)['"]$/.exec(e);
      if (lit) return [lit[1] as string];
      const field = /^KEYS\.(\w+)$/.exec(e);
      if (field) {
        const m = new RegExp(`^\\s*${field[1]}:\\s*'([^']+)'`, 'm').exec(all);
        return m ? [m[1] as string] : [];
      }
      const named = /^([A-Z_][A-Z0-9_]*)$/.exec(e);
      if (named) {
        const m = new RegExp(`const ${named[1]}\\s*=\\s*'([^']+)'`).exec(file) ?? new RegExp(`const ${named[1]}\\s*=\\s*'([^']+)'`).exec(all);
        return m ? [m[1] as string] : [];
      }
      // a row component's prop (General's OnOffRow k={KEYS.tui}): every value the file passes it
      const prop = /^props\.(\w+)$/.exec(e);
      if (prop && depth === 0) return [...file.matchAll(new RegExp(`\\b${prop[1]}=\\{([^}]+)\\}`, 'g'))].flatMap((m) => resolve(m[1] as string, file, 1));
      return [];
    };
    const keys = new Set<string>();
    for (const file of texts) for (const m of file.matchAll(/settingKey=(?:"([^"]+)"|\{([^}]+)\})/g)) for (const k of m[1] !== undefined ? [m[1]] : resolve(m[2] as string, file)) keys.add(k);
    out.set(dir, keys);
  }
  return out;
}

test('every_key_a_pane_lists_has_a_row_with_that_setting_key_settings_audit_r1', () => {
  const rows = rowsByDir();
  // plugins that draw rows into a Settings pane's slot (attachTo shell.settings:…)
  const slotted = Object.entries(indexes)
    .filter(([, text]) => /attachTo:\s*\{\s*id:\s*'shell\.settings:/.test(text))
    .map(([p]) => dirOf(p));
  const missing: string[] = [];
  for (const [path, lists] of Object.entries(paneKeys)) {
    const dir = dirOf(path);
    for (const [pane, keys] of Object.entries(lists))
      for (const key of keys) if (![dir, ...slotted].some((d) => rows.get(d)?.has(key))) missing.push(`${pane}: ${key}`);
  }
  expect(missing).toEqual([]);
  // the check reads what it says: General's OnOffRow keys and wisp.threads' constant resolve
  expect(rows.get('shell-settings')?.has('terminal.terminalSessions')).toBe(true);
  expect(rows.get('threads')?.has('threads.compactAudit')).toBe(true);
});

// Settings audit r1 review: a harness's Defaults and Limits rows exist only with a harness picked, so the search,
// which can't know which harness, opens runtime.launch and budget.limitHoldBy on All settings, filtered.
test('per_harness_settings_open_on_all_settings_settings_audit_r1', () => {
  const panes = realPanes();
  expect(settingTarget({ key: 'runtime.launch', owner: 'wisp.runtime' }, panes)).toEqual({ pane: 'all', filter: 'runtime.launch' });
  expect(settingTarget({ key: 'budget.limitHoldBy', owner: 'wisp.budget' }, panes)).toEqual({ pane: 'all', filter: 'budget.limitHoldBy' });
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
