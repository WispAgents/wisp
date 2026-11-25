// `/workflow` in a prompt box (ADR-0119): `/` offers it (many agents on one job), and `/workflow ` offers the saved
// workflows by name, each with what it's for; a pick puts `/workflow <name> ` in the box. Anything else after it is a
// job in words: the task's agent plans the workflow (plugins/workflows/host slash.rs reads the prompt either way).
import { defineCommand, signal, type PeerId, type PluginClient, type WritableSignal } from '@wisp/client';
import type { PromptItem } from '@wisp/shell';
import { t } from './i18n.ts';

export interface Saved {
  readonly name: string;
  readonly description: string;
}

export const MAX = 8;

/** `workflows.list`: the saved workflows (asked once per peer per page). */
const listWorkflows = defineCommand<{ peer: PeerId }, unknown>({ id: 'workflows.list', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the workflows', quiet: true });

/** What the menu offers for what's typed after `/`: the command itself while the word could still be it, then the saved
 *  workflows for `/workflow <word>` (names that start with it first, then the ones holding it). */
export function suggestions(query: string, saved: readonly Saved[]): PromptItem[] {
  const q = query.toLowerCase();
  if (!q.includes(' ')) {
    if (!'workflow'.startsWith(q) && !(q.length >= 2 && 'wf'.startsWith(q))) return [];
    return [{ value: '/workflow', label: '/workflow', hint: t('wisp.workflows.slash.hint'), icon: 'org', insert: '/workflow ' }];
  }
  const m = /^(?:workflow|wf) (\S*)$/.exec(q);
  if (!m) return [];
  const w = m[1] ?? '';
  if (w && saved.some((s) => s.name.toLowerCase() === w)) return [];
  const byName = (a: Saved, b: Saved): number => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
  const starts = saved.filter((s) => s.name.toLowerCase().startsWith(w)).sort(byName);
  const holds = w ? saved.filter((s) => !s.name.toLowerCase().startsWith(w) && `${s.name}\n${s.description}`.toLowerCase().includes(w)).sort(byName) : [];
  return [...starts, ...holds].slice(0, MAX).map((s) => ({ value: `/workflow ${s.name}`, label: s.name, hint: s.description || t('wisp.workflows.slash.saved'), icon: 'org', insert: `/workflow ${s.name} ` }));
}

const known = new Map<string, WritableSignal<readonly Saved[]>>();

const savedOf = (rows: unknown): Saved[] =>
  (Array.isArray(rows) ? rows : []).flatMap((r: unknown) => {
    const o = typeof r === 'object' && r !== null ? (r as Record<string, unknown>) : {};
    return typeof o['name'] === 'string' && o['name'] ? [{ name: o['name'], description: typeof o['description'] === 'string' ? o['description'] : '' }] : [];
  });

/** Peer `peer`'s saved workflows: none until its answer lands, then the menu reads them. */
export function savedOn(c: PluginClient, peer: string): readonly Saved[] {
  let s = known.get(peer);
  if (!s) {
    const mine = signal<readonly Saved[]>([]);
    s = mine;
    known.set(peer, mine);
    void c.run(listWorkflows, { peer: peer as PeerId }).then(
      (r) => mine(savedOf(r)),
      () => known.delete(peer),
    );
  }
  return s();
}
