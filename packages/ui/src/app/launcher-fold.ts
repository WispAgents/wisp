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
