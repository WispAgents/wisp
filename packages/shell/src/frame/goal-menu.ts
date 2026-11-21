// Goal…'s menu and every menu that grows with the formula (owner-list #63, owner 2026-11-19: "what if a user has a lot
// of goals?"): the goals this project used most recently first, then A–Z; about seven show, then "All goals…" (a
// searchable picker), "New goal…" last; past seven a filter box over the rows narrows them by name and description.
// Recipes take the same cap, filter and "All recipes…" in their own order. Off the first screen (ADR-0105): loaded
// when `+` is warmed or pressed (HomeView, TaskView), with the goals' answer. Pure.
import type { Act, MenuFilter, MenuItem } from '@wisp/ui/pure';
import type { ShellContext } from '../contract.ts';
import { defineCommand } from '@wisp/client';
import type { DeclaredGoal } from './goal-gate.ts';
import { projectOfRow, type ProjectView } from './rows-view.ts';
import { openPick } from './pick-list.ts';
import '../i18n/lazy.ts';

/** How many rows show before "All …" (and, past it, the filter box). */
export const SHOWN = 7;
export const GOAL_ALL = 'goal:all';
export const GOAL_NEW = 'goal:new';

type Say = (key: string, vars?: Readonly<Record<string, string | number>>) => string;

/** The declared goals as `goal.declared` answers (each with a name and what done means), the rest left out. */
export const declaredGoals = (rows: unknown): DeclaredGoal[] =>
  (Array.isArray(rows) ? rows : [])
    .map((r: unknown) => (typeof r === 'object' && r !== null ? (r as Record<string, unknown>) : {}))
    .filter((r): r is { name: string; text: string } => typeof r['name'] === 'string' && r['name'] !== '' && typeof r['text'] === 'string')
    .map((r) => ({ name: r.name, text: r.text }));

/** The declared goal Goal…'s menu (or All goals…) picked (`goal:<name>`); undefined for a new one. */
export const pickedGoal = (declared: readonly DeclaredGoal[], value: string): DeclaredGoal | undefined =>
  value === GOAL_NEW || value === GOAL_ALL ? undefined : declared.find((g) => `goal:${g.name}` === value);

const obj = (v: unknown): Record<string, unknown> | undefined => (typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : undefined);

/** The declared goals the tasks of project `project` were given, the most recent first, each once: a task's goal
 *  summary names the declared goal it was set by (`name`, wisp.goal's list row), set at `setAt` (else the task's
 *  last change). */
export function recentGoals(
  rows: readonly { readonly summary?: unknown; readonly body?: unknown; readonly updated_at?: number }[],
  projects: readonly ProjectView[],
  project: string | undefined,
): string[] {
  if (!project) return [];
  const used: { name: string; at: number }[] = [];
  for (const r of rows) {
    const g = obj(obj(r.summary)?.['goal']);
    const name = g?.['name'];
    if (typeof name !== 'string' || !name) continue;
    if (projectOfRow(r, projects)?.id !== project) continue;
    const at = typeof g?.['setAt'] === 'number' ? g['setAt'] : (r.updated_at ?? 0);
    used.push({ name, at });
  }
  used.sort((a, b) => b.at - a.at);
  return [...new Set(used.map((u) => u.name))];
}

/** The goals in the menu's order: the ones `recent` names, most recent first, then the rest A–Z (any case). */
export function goalOrder(declared: readonly DeclaredGoal[], recent: readonly string[]): DeclaredGoal[] {
  const byName = new Map(declared.map((g) => [g.name, g]));
  const first = recent.flatMap((n) => byName.get(n) ?? []);
  const seen = new Set(first.map((g) => g.name));
  const rest = declared.filter((g) => !seen.has(g.name)).sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }));
  return [...first, ...rest];
}

export interface LongMenu {
  readonly items: MenuItem[];
  readonly filter?: MenuFilter;
}

/** A menu that grows with the formula: every row while there are `SHOWN` or fewer; past that the first `SHOWN`, then
 *  `all` (its picker), with a filter over all of them. `tail` (New …) always last, kept whatever is typed. */
export function longMenu(rows: readonly MenuItem[], o: { readonly all: MenuItem; readonly tail: readonly MenuItem[]; readonly filter: Omit<MenuFilter, 'all' | 'keep'> }): LongMenu {
  if (rows.length <= SHOWN) return { items: [...rows, ...o.tail] };
  return { items: [...rows.slice(0, SHOWN), o.all, ...o.tail], filter: { ...o.filter, all: rows, keep: [o.all.value, ...o.tail.map((t) => t.value)] } };
}

/** A goal's row: its name, what done means under it. */
export const goalRow = (g: DeclaredGoal): MenuItem => ({ value: `goal:${g.name}`, label: g.name, hint: g.text, icon: 'target' });

/** Goal…'s menu: the declared goals in their order (goalOrder), capped with All goals…, then New goal…. */
export function goalMenu(declared: readonly DeclaredGoal[], recent: readonly string[], t: Say): LongMenu {
  return longMenu(goalOrder(declared, recent).map(goalRow), {
    all: { value: GOAL_ALL, label: t('shell.home.goalAll', { n: declared.length }), icon: 'search' },
    tail: [{ value: GOAL_NEW, label: t('shell.home.goalNew'), icon: 'plus' }],
    filter: { label: t('shell.home.goalFilter'), placeholder: t('shell.home.goalFilterHint'), empty: t('shell.home.goalNone') },
  });
}

export const RECIPE_ALL = 'recipe:all';

/** Recipes' menu: the project's recipes as they come, capped with All recipes…, then New. */
export function recipeMenu(recipes: readonly MenuItem[], newLabel: string, t: Say): LongMenu {
  return longMenu(recipes, {
    all: { value: RECIPE_ALL, label: t('shell.home.recipesAll', { n: recipes.length }), icon: 'search' },
    tail: [{ value: 'recipe:new', label: newLabel, icon: 'plus' }],
    filter: { label: t('shell.home.recipesFilter'), placeholder: t('shell.home.recipesFilterHint'), empty: t('shell.home.recipesNone') },
  });
}

type Navigate = Parameters<typeof openPick>[0];

/** All goals…: every declared goal in the menu's order, searched; a pick is `take`n as the menu's would be. */
export function pickGoal(shell: Navigate, declared: readonly DeclaredGoal[], recent: readonly string[], t: Say, take: (g: DeclaredGoal) => void): void {
  openPick(shell, {
    title: t('shell.pick.goals'),
    placeholder: t('shell.pick.goalsHint'),
    rows: goalOrder(declared, recent).map((g) => ({ value: `goal:${g.name}`, label: g.name, hint: g.text })),
    onPick: (v) => {
      const g = pickedGoal(declared, v);
      if (g) take(g);
    },
  });
}

/** All recipes…: every recipe of the project, searched; a pick runs it as the menu's would. */
export function pickRecipe(shell: Navigate, recipes: readonly MenuItem[], t: Say, run: (value: string) => void): void {
  openPick(shell, { title: t('shell.pick.recipes'), placeholder: t('shell.pick.recipesHint'), rows: recipes.map((r) => ({ value: r.value, label: r.label, ...(r.hint && { hint: r.hint }) })), onPick: run });
}

/** The goals declared by name on the peer (`goals:` in a formula): Goal… offers them (ADR-0110). Quiet: a peer without
 *  wisp.goal (or an older one) has none, never an error toast. Here, not in ./commands.ts: off the first screen. */
export const goalsDeclared = defineCommand<{ peer: string }, unknown>({ id: 'goal.declared', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the goals', quiet: true });

/** Asked again at most this often while `+` is warmed. */
const AGAIN_MS = 15_000;
const asked = new WeakMap<object, { peer: string; at: number }>();

/** Peer `peer()`'s declared goals into `set` (an answer for another peer than it is by then is dropped), asked again
 *  after AGAIN_MS; a peer without wisp.goal (or an older one) has none, quietly. */
export function askGoals(client: Pick<ShellContext['client'], 'commands'>, peer: () => string, set: (goals: readonly DeclaredGoal[]) => void): void {
  const p = peer();
  const was = asked.get(set);
  if (was?.peer === p && Date.now() - was.at < AGAIN_MS) return;
  asked.set(set, { peer: p, at: Date.now() });
  void client.commands.run(goalsDeclared, { peer: p }).then(
    (r) => peer() === p && set(declaredGoals(r)),
    () => asked.delete(set),
  );
}

/** Goal…'s row in `+`: its menu (goalMenu) once goals are declared, else a press that opens Done when (`fresh`). */
export function goalAct(o: {
  readonly shell: Navigate;
  readonly declared: readonly DeclaredGoal[];
  readonly recent: () => readonly string[];
  readonly t: Say;
  readonly take: (g: DeclaredGoal) => void;
  readonly fresh: () => void;
}): Act {
  if (o.declared.length === 0) return { onClick: o.fresh };
  const m = goalMenu(o.declared, o.recent(), o.t);
  return {
    menu: {
      items: m.items,
      ...(m.filter && { filter: m.filter }),
      onSelect: (v) => {
        if (v === GOAL_ALL) return pickGoal(o.shell, o.declared, o.recent(), o.t, o.take);
        const g = pickedGoal(o.declared, v);
        return g ? o.take(g) : o.fresh();
      },
    },
  };
}

/** Recipes' row in `+`: the project's recipes (recipeMenu), All recipes… past seven, then New. */
export function recipeAct(o: { readonly shell: Navigate; readonly rows: readonly MenuItem[]; readonly newLabel: string; readonly t: Say; readonly run: (value: string) => void; readonly fresh: () => void }): Act {
  const m = recipeMenu(o.rows, o.newLabel, o.t);
  return {
    menu: {
      items: m.items,
      ...(m.filter && { filter: m.filter }),
      onSelect: (v) => (v === 'recipe:new' ? o.fresh() : v === RECIPE_ALL ? pickRecipe(o.shell, o.rows, o.t, o.run) : o.run(v)),
    },
  };
}
