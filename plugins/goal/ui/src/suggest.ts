// `/goal ` in a prompt box suggests the goals declared by name (owner-list #63): the names that start with (or hold)
// what's typed after it, each with what done means; a pick puts `/goal <name>` in the box, so Enter sends it and the
// peer sets that goal as declared (a text that is exactly a declared name is that goal, ADR-0110). Only while the word
// after `/goal ` is one word: past a space it's a goal in words, and nothing gets in the way of its Enter.
import { ownerOf, signal, type PluginClient, type WritableSignal } from '@wisp/client';
import type { PromptItem } from '@wisp/shell';
import { goalsDeclared } from './commands.ts';

export interface Declared {
  readonly name: string;
  readonly text: string;
}

/** How many it suggests (the box's list shows about this many). */
export const MAX = 8;

/** The query the `/` menu reads after `/goal ` (prompt-menu menuAt: `goal <word>`), or undefined for any other. */
export const goalWord = (query: string): string | undefined => /^goal (\S*)$/i.exec(query)?.[1];

/** The declared goals for what's typed after `/goal `: names that start with it first, then the ones holding it in the
 *  name or in what done means, each A–Z; at most MAX. */
export function goalSuggestions(goals: readonly Declared[], word: string): PromptItem[] {
  const w = word.toLowerCase();
  const byName = (a: Declared, b: Declared): number => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
  const starts = goals.filter((g) => g.name.toLowerCase().startsWith(w)).sort(byName);
  const holds = w ? goals.filter((g) => !g.name.toLowerCase().startsWith(w) && `${g.name}\n${g.text}`.toLowerCase().includes(w)).sort(byName) : [];
  return [...starts, ...holds].slice(0, MAX).map((g) => ({ value: `/goal ${g.name}`, label: g.name, hint: g.text, icon: 'target', insert: `/goal ${g.name}` }));
}

/** each peer's declared goals as it last said (a signal: the menu redraws when they land), asked once per page */
const known = new Map<string, WritableSignal<readonly Declared[]>>();

const declaredOf = (rows: unknown): Declared[] =>
  (Array.isArray(rows) ? rows : []).flatMap((r: unknown) => {
    const o = typeof r === 'object' && r !== null ? (r as Record<string, unknown>) : {};
    return typeof o['name'] === 'string' && o['name'] && typeof o['text'] === 'string' ? [{ name: o['name'], text: o['text'] }] : [];
  });

/** Peer `peer`'s declared goals: what's known now (none until its answer lands, then the menu reads them). */
export function goalsOf(c: PluginClient, peer: string): readonly Declared[] {
  let s = known.get(peer);
  if (!s) {
    const mine = signal<readonly Declared[]>([]);
    s = mine;
    known.set(peer, mine);
    void c.run(goalsDeclared, { peer: peer as never }).then(
      (r) => mine(declaredOf(r)),
      // an older peer (or one without wisp.goal) has none; asked again on the next page
      () => undefined,
    );
  }
  return s();
}

/** The `/` menu's items for `/goal <word>` in a box: the task's peer's goals in a reply, home's in New task. */
export function goalItems(query: string, ctx: { readonly client: PluginClient; readonly task?: string }): readonly PromptItem[] {
  const word = goalWord(query);
  if (word === undefined) return [];
  return goalSuggestions(goalsOf(ctx.client, ctx.task ? ownerOf(ctx.task) : ctx.client.home), word);
}
