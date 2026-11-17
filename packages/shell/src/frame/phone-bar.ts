// The phone apps' New task bar (QA r2 #2; ADR-0012: a bar collapses into More, it never runs off its edge): Goal,
// Worktree and Start later sat past a 390 px screen's edge or under Send, and Recipes were gone with their empty row.
// On a phone the tail folds into one More tool whose menu holds them all (a toggle with its state, a tool with a menu
// as a submenu), then Start from an issue and Recipes (each recipe, and New recipe). Pure: tools in, a tool out.
import type { Act, LauncherTool, MenuEntry, MenuItem } from '@wisp/ui';

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

/** The tail as one More tool. */
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
