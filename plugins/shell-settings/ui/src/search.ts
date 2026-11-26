// Settings' search over the settings themselves (audit A F11): every declared setting (settings.list) by its label,
// its description or its key, and where each opens: the pane that draws its row (this plugin's General, Chat and
// Notifications draw other plugins' settings), else the pane of the plugin that declares it when that plugin has one
// pane, else All settings filtered to it. Pure, for tests.

import { AUTO_SETTING } from '@wisp/shell/pure';
import { KEYS } from './types.ts';
// HARNESS_ROWS names lazy keys (frame.tsx, this module's one user, loads them too)
import './i18n/lazy.ts';

/** A declared setting as the search reads it (its title split as All settings shows it). */
export interface FoundSetting {
  readonly key: string;
  readonly owner: string;
  readonly label: string;
  readonly help: string;
  /** a harness detail's row (`agents:<id>:<n>`): the `data-setting-key` its row in Agents carries, when it has one */
  readonly focus?: string;
}

/** The settings with every word typed in their label, description or key (any case; a key's dots and case ignored).
 *  Never Auto's switch: All settings has no row for it while Auto is put away (owner, 2026-11-17; setup audit B6), and
 *  Settings → Auto holds it while it's on. */
export function foundSettings(all: readonly FoundSetting[], query: string): FoundSetting[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return all.filter((s) => {
    if (s.key === AUTO_SETTING) return false;
    const text = `${s.label} ${s.help} ${s.key} ${s.key.replace(/[._]/g, '')}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
}

/** The rows each harness's detail in Settings → Agents holds (harness audit r2), by their title keys: the search finds
 *  "Cursor: Default model" and opens Agents on Cursor. */
export const HARNESS_ROWS = [
  'shell-settings.harness.signIn',
  'shell-settings.harness.program',
  'shell-settings.harness.enabled',
  'shell-settings.harness.model',
  'shell-settings.harness.effort',
  'shell-settings.harness.mode',
  'shell-settings.harness.stop',
] as const;
const AGENTS = 'agents';
const ROW = 'agents:';
/** What each HARNESS_ROWS row of a harness's detail is found by in Agents (wisp.accounts puts `agents.<id>.<part>`
 *  on it, one harness's detail at a time, so a re-pick never finds the last harness's row); its stop line is
 *  wisp.budget's, found by the declared key it sets (`budget.autoStopCursor`). */
const ROW_PARTS = ['signIn', 'program', 'enabled', 'model', 'effort', 'mode'] as const;
const STOP_KEY = 'budget.autoStop';

/** The key a harness detail's row `i` carries in Agents (`declared`: the declared keys, for its stop line's). */
export function harnessRowKey(id: string, i: number, declared: readonly string[] = []): string | undefined {
  const part = ROW_PARTS[i];
  if (part) return `agents.${id}.${part}`;
  if (HARNESS_ROWS[i] === 'shell-settings.harness.stop') return declared.find((k) => k.startsWith(STOP_KEY) && harnessOfKey(k, [id]) === id);
  return undefined;
}

/** One search hit per harness and row of its detail (`titles`: the rows' words), each with the key of its row. */
export function harnessRows(
  harnesses: readonly { readonly id: string; readonly name: string }[],
  titles: readonly string[],
  declared: readonly string[] = [],
): FoundSetting[] {
  return harnesses.flatMap((h) =>
    titles.map((title, i) => {
      const focus = harnessRowKey(h.id, i, declared);
      return { key: `${ROW}${h.id}:${i}`, owner: AGENTS, label: `${h.name}: ${title}`, help: '', ...(focus !== undefined && { focus }) };
    }),
  );
}

/** The row a found setting lands on: its own key, or a harness detail row's (none: the pane opens, no row lit). */
export function focusOf(f: Pick<FoundSetting, 'key' | 'focus'>): string | undefined {
  return f.key.startsWith(ROW) ? f.focus : f.key;
}

/** The harness a declared setting is about, by its key's last part (`budget.autoStopCursor` → cursor), when the
 *  Agents pane has a detail for it. */
export function harnessOfKey(key: string, harnesses: readonly string[]): string | undefined {
  const tail = (key.split('.').pop() ?? '').toLowerCase();
  return harnesses.find((id) => tail !== id && tail.endsWith(id.toLowerCase()));
}

type OwnKey = keyof typeof KEYS;
const DRAWN: Record<string, readonly OwnKey[]> = {
  // not terminal / terminalCommand: General draws them only with more than one terminal app, and the command only
  // for Custom (SOMETIMES below)
  general: ['editor', 'tui', 'blocks', 'worktree', 'review', 'fixConflicts', 'readTimes', 'watch', 'apps', 'crewTint'],
  chat: ['transcript', 'chatToolCalls', 'chatThinking', 'followUp', 'sendShortcut', 'chatWidth', 'timeFormat'],
  notifications: ['notify', 'acceptChannels', 'channelProject'],
};
/** The pane that draws each of these keys' rows, whoever declares the key: channels.acceptChannels is wisp.channels'
 *  (whose one pane is Phone) but its row is in Notifications, so routing by the owner opened a pane without it. */
export const PANE_OF_KEY: ReadonlyMap<string, string> = new Map(Object.entries(DRAWN).flatMap(([pane, keys]) => keys.map((k) => [KEYS[k], pane] as const)));
/** Keys General draws only some of the time: All settings, which always draws them (as before doc924a). */
const SOMETIMES: ReadonlySet<string> = new Set([KEYS.terminal, KEYS.terminalCommand]);

/** Where a found setting opens: a harness's row (or a setting about one harness) on its detail in Agents, else the
 *  pane that draws its row, else its plugin's one pane, else All settings on it. */
export function settingTarget(
  s: Pick<FoundSetting, 'key' | 'owner'>,
  panes: readonly { readonly id: string; readonly plugin: string }[],
  harnesses: readonly string[] = [],
): { pane: string; filter?: string } {
  const hasAgents = panes.some((p) => p.id === AGENTS);
  if (hasAgents && s.key.startsWith(ROW)) return { pane: AGENTS, filter: s.key.slice(ROW.length).split(':')[0] ?? '' };
  const about = hasAgents ? harnessOfKey(s.key, harnesses) : undefined;
  if (about) return { pane: AGENTS, filter: about };
  const drawn = PANE_OF_KEY.get(s.key);
  if (drawn && panes.some((p) => p.id === drawn)) return { pane: drawn };
  if (SOMETIMES.has(s.key)) return { pane: 'all', filter: s.key };
  const own = panes.filter((p) => p.plugin === s.owner);
  if (own.length === 1 && own[0]) return { pane: own[0].id };
  return { pane: 'all', filter: s.key };
}
