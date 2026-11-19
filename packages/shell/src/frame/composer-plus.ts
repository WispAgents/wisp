// The composer's `+` (docs/ux/2026-11-18-desktop.md #3, decided by the owner's delegate 2026-11-18): one menu for what
// most launches don't use, so the footer holds `+`, the mode, the harness · model, Where and Send. Rows come in
// sections (a line between them, none for an empty one); each keeps its control's part, so a spell aimed at
// `launcher.goal` still finds Goal; a control with its own menu (Start later's times, Recipes) is a submenu, and a pick
// does what the control did. Pure: rows in, a tool out.
import type { Act, IconName, LauncherTool, MenuEntry, MenuItem, PartName } from '@wisp/ui/pure';

export interface PlusRow {
  readonly id: string;
  readonly part: PartName;
  readonly icon: IconName;
  readonly label: string;
  /** its shortcut, shown at the row's end */
  readonly keys?: string;
  readonly act?: Act | undefined;
}

export interface PlusInput {
  readonly part: PartName;
  readonly label: string;
  readonly title: string;
  readonly sections: readonly (readonly (PlusRow | false | undefined)[])[];
  /** the box's text gets the caret back after a choice inside a row's own menu (a Start later time, a recipe), so
   *  Enter sends (audit C #2: it stayed on `+`, and Enter opened the menu again) */
  readonly refocus?: () => void;
}

const press = (act: Act | undefined): void => {
  if (!act?.pending) act?.onClick?.();
};

export function plusTool(x: PlusInput): LauncherTool {
  const sections = x.sections.map((s) => s.filter((r): r is PlusRow => !!r)).filter((s) => s.length > 0);
  const rows = sections.flat();
  const entry = (r: PlusRow): MenuEntry => {
    const menu = r.act?.menu;
    if (menu) return { kind: 'submenu', value: r.id, label: r.label, icon: r.icon, part: r.part, items: menu.items.filter((e: MenuEntry): e is MenuItem => e.kind === undefined || e.kind === 'item') };
    return { value: r.id, label: r.label, icon: r.icon, part: r.part, ...(r.keys && { keys: r.keys }), ...(r.act?.pending && { disabled: true }) };
  };
  const items: MenuEntry[] = sections.flatMap((s, i) => [...(i > 0 ? [{ kind: 'separator' as const }] : []), ...s.map(entry)]);
  const onSelect = (v: string): void => {
    const row = rows.find((r) => r.id === v);
    if (row) return press(row.act);
    // a choice inside a row's own menu (Start later's times, a recipe)
    const owner = rows.find((r) => r.act?.menu?.items.some((e: MenuEntry) => (e.kind === undefined || e.kind === 'item') && e.value === v));
    owner?.act?.menu?.onSelect(v);
    // after the menu has closed and handed focus back to `+`
    if (owner && x.refocus) setTimeout(x.refocus, 0);
  };
  return { id: 'plus', part: x.part, icon: 'plus', label: x.label, title: x.title, act: { menu: { items, onSelect } } };
}
