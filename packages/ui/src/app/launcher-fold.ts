// The prompt box's toolbar running out of room (design/layout.md §2, ADR-0012): a bar never clips or runs off its
// edge. Its labels go to their icons first (the container queries in app-home.css, v1's order); then the tail's tools
// (Goal, Worktree, Start later) move into one More menu, the last first, as many as the measured widths need. The
// phone apps fold the whole tail into More with Start from an issue and Recipes (QA r2 #2, the shell's phone-bar.ts).
// Pure: widths and tools in, a count or a tool out.
import type { IconName } from '../content/icons.ts';
import type { MenuEntry, MenuItem } from '../controls/menu-entries.ts';
import type { Act } from './act.tsx';

/** One button on the prompt box's toolbar. */
export interface LauncherTool {
  readonly id: string;
  readonly part: string;
  readonly icon: IconName;
  readonly label: string;
  readonly title: string;
  /** shows its label beside the icon */
  readonly text?: boolean;
  readonly on?: boolean;
  readonly disabled?: boolean;
  readonly act?: Act;
}

export interface FoldExtras {
  readonly label: string;
  readonly title: string;
  /** Try another agent (the pill's +): in More too, so a narrow bar folds the + and loses nothing (QA phone r4 #1) */
  readonly compare?: { readonly label: string; readonly act?: Act | undefined } | undefined;
  readonly issue?: { readonly label: string; readonly act?: Act | undefined } | undefined;
  readonly recipes?:
    | {
        readonly label: string;
        readonly items: readonly { readonly id: string; readonly label: string; readonly act: Act }[];
        readonly add: { readonly label: string; readonly act?: Act | undefined };
      }
    | undefined;
}

const TOOL = 'tool:';
const RECIPE = 'recipe:';

const press = (act: Act | undefined): void => {
  if (!act?.pending) act?.onClick?.();
};

/** `tools` as one More tool: a toggle with its state, a tool with a menu as a submenu, then the extras (Start from an
 *  issue, Recipes with each recipe and New recipe). */
export function foldTail(tools: readonly LauncherTool[], x: FoldExtras): LauncherTool {
  const entries: MenuEntry[] = tools.map((t): MenuEntry => {
    const menu = t.act?.menu;
    if (menu) return { kind: 'submenu', value: `${TOOL}${t.id}`, label: t.label, icon: t.icon, items: menu.items.filter((e): e is MenuItem => e.kind === undefined || e.kind === 'item') };
    return { value: `${TOOL}${t.id}`, label: t.label, icon: t.icon, ...(t.on !== undefined && { checked: t.on }), ...(t.disabled && { disabled: true }) };
  });
  const extras: MenuEntry[] = [
    ...(x.compare ? [{ value: 'compare', label: x.compare.label, icon: 'plus' as const }] : []),
    ...(x.issue ? [{ value: 'issue', label: x.issue.label, icon: 'issue' as const }] : []),
    ...(x.recipes
      ? [
          {
            kind: 'submenu' as const,
            value: 'recipes',
            label: x.recipes.label,
            icon: 'bolt' as const,
            items: [...x.recipes.items.map((r) => ({ value: `${RECIPE}${r.id}`, label: r.label })), { value: `${RECIPE}new`, label: x.recipes.add.label, icon: 'plus' as const }],
          },
        ]
      : []),
  ];
  const onSelect = (v: string): void => {
    if (v.startsWith(TOOL)) return press(tools.find((t) => `${TOOL}${t.id}` === v)?.act);
    if (v === 'compare') return press(x.compare?.act);
    if (v === 'issue') return press(x.issue?.act);
    if (v === `${RECIPE}new`) return press(x.recipes?.add.act);
    if (v.startsWith(RECIPE)) return press(x.recipes?.items.find((r) => `${RECIPE}${r.id}` === v)?.act);
    // a choice inside a tool's own menu (Start later's times)
    const owner = tools.find((t) => t.act?.menu?.items.some((e) => (e.kind === undefined || e.kind === 'item') && e.value === v));
    owner?.act?.menu?.onSelect(v);
  };
  const on = tools.some((t) => t.on === true);
  return {
    id: 'more',
    part: 'launcher.more',
    icon: 'dots',
    label: x.label,
    title: x.title,
    ...(on && { on: true }),
    act: { menu: { items: [...entries, ...(entries.length && extras.length ? [{ kind: 'separator' as const }] : []), ...extras], onSelect } },
  };
}

export interface FoldInput {
  /** the bar's inner width */
  readonly available: number;
  /** every other item on the bar, with the gaps between them (Send, the harness, Delegate…) */
  readonly others: number;
  /** each tail tool's width as last seen, in the bar's order */
  readonly tail: readonly number[];
  readonly gap: number;
  /** the More button's width */
  readonly more: number;
}

/** How many of the tail's tools, from its end, go into More so the bar fits; all of them when nothing else does. The
 *  widths are the tools' own (remembered while folded), so a bar at the edge never flaps between two counts. */
export function tailFold(i: FoldInput): number {
  const n = i.tail.length;
  for (let k = 0; k <= n; k++) {
    const kept = i.tail.slice(0, n - k).reduce((s, w) => s + i.gap + w, 0);
    if (i.others + kept + (k > 0 ? i.gap + i.more : 0) <= i.available) return k;
  }
  return n;
}

/** The bar measured at one of its squeeze levels (`SQUEEZE`). `others`: every item but the tail's tools, with the gaps
 *  between them; `tail`: each tool's width at that level, as last seen. */
export interface BarForm {
  readonly others: number;
  readonly gap: number;
  readonly tail: ReadonlyMap<string, number>;
}

/** How far the bar's labels have gone to their icons, one step at a time, in v1's order (its 720 and 640 px steps,
 *  Worktree's and Delegate's, already happen by the box's width): 0 every label; 1 Goal's goes; 2 the mode's (Full
 *  auto); 3 v1's narrow-box form, the pick down to its model, the bar scrolling under a pinned Send if it must. */
export const SQUEEZE = 4;

/** a tool not seen yet at a level: an icon button's width */
const UNSEEN = 30;

/** How the prompt box's toolbar fits (design/layout.md §2, QA desktop r3 #3 #4): the fewest labels gone to their
 *  icons that holds every tool, one step at a time; at the last step the tail's last tools go into More, as many as it
 *  takes. Nothing truncates (the pick keeps its words while a label can still go). A level not measured yet is tried as
 *  is, so the next measure knows it. */
export function barPlan(i: { readonly available: number; readonly ids: readonly string[]; readonly forms: readonly (BarForm | undefined)[]; readonly more: number }): { level: number; fold: number } {
  const fold = (f: BarForm): number => tailFold({ available: i.available, others: f.others, tail: i.ids.map((id) => f.tail.get(id) ?? UNSEEN), gap: f.gap, more: i.more });
  for (let level = 0; level < SQUEEZE; level++) {
    const f = i.forms[level];
    if (f === undefined || fold(f) === 0) return { level, fold: 0 };
  }
  const last = i.forms[SQUEEZE - 1];
  return { level: SQUEEZE - 1, fold: last ? fold(last) : 0 };
}
