// A task's menu (v1 #786 actions.js taskMenuHtml, design/ux.md → Task menus; docs/ux/2026-11-18-desktop.md #6): the
// row's right-click menu and the open task's More build it here, so both say the same things in one order: each menu's
// own head, then 4 groups (run it again, mark it, its options and copies, Archive and Delete last), the rarer ones in
// submenus, so the menu stays about a dozen rows. Pure.
import { menuSections, type MenuEntry, type MenuItem, type MenuGroup, type MenuSeparator, type MenuSubmenu, type IconName } from '@wisp/ui/pure';
import type { ExtensionProps, ResolvedExtension, TaskMenuSection, TaskMenuSub } from './contract.ts';

/** One entry of a task's menu and where it goes: the shell's, the list's or a plugin's (`bar.section`, `bar.sub`). */
export interface TaskMenuEntry {
  readonly entry: MenuItem | MenuGroup;
  readonly section?: TaskMenuSection;
  readonly sub?: TaskMenuSub;
  /** within its section (or submenu); a plugin's `order` */
  readonly order: number;
}

export interface TaskMenuInput {
  readonly entries: readonly TaskMenuEntry[];
  /** Delete task, alone at the end with Archive */
  readonly danger: readonly MenuItem[];
  /** Pin, Mark read: the list's own, in group 2 with Rename and Save for later */
  readonly after?: readonly MenuItem[];
  /** a submenu's name ('shell.menu.view' …) */
  readonly t: (key: string) => string;
  /** the phone apps (QA r2 #5: 27 rows deep): v1's sections, past the first one of more than two rows one submenu, its
   *  own submenus headed groups inside (iOS HIG / M3). The 2026-11-18 order is the desktop's (docs/ux/2026-11-18-desktop.md
   *  is desktop only); the phone keeps the menu its QA rounds settled. */
  readonly compact?: boolean;
}

/** Where an entry goes (docs/ux/2026-11-18-desktop.md #6, decided by the owner's delegate): one order for More and the
 *  row's right-click, 4 groups and submenus. `go` is each menu's own head (the row's Open, Changes, PR, Shell; More's
 *  Transcript and Shell; the phone's Details, Ship, Done); then 1 · Rerun, Fork, Open in ›; 2 · Rename, Pin, Mark
 *  unread, Save for later; 3 · Task options › (goal, crew, recipe, side question, Thread, Lean runs ›, View ›, Files ›),
 *  Copy ›, Share…, Save to repo; 4 · Archive, Delete, always last. */
type Slot = 'go' | 'run' | 'openin' | 'mark' | 'options' | 'view' | 'files' | 'copy' | 'share' | 'end';

/** the built-ins, by their value, in each slot's order */
const PLACE: readonly (readonly [string, Slot])[] = [
  ['open', 'go'], ['details', 'go'], ['transcript', 'go'], ['changes', 'go'], ['x:shell.row.changes', 'go'], ['x:wisp.ship.rowReview', 'go'],
  ['shell', 'go'], ['x:shell.row.shell', 'go'], ['ship', 'go'], ['done', 'go'], ['next-task', 'go'], ['previous-task', 'go'],
  ['rerun', 'run'], ['x:shell.row.rerun', 'run'], ['fork', 'run'], ['x:shell.row.fork', 'run'],
  ['openIn', 'openin'], ['x:shell.row.editor', 'openin'],
  ['rename', 'mark'], ['pin', 'mark'], ['read', 'mark'], ['later', 'mark'],
  ['x:shell.row.side', 'options'], ['x:wisp.crew.rowCrew', 'options'], ['x:wisp.automations.rowRecipe', 'options'], ['x:wisp.threads.menu', 'options'],
  ['compact', 'options'], ['x:wisp.ship.rowShip', 'options'], ['x:wisp.ship.rowMerge', 'options'], ['lean', 'options'],
  ['copy-id', 'copy'], ['x:shell.row.copyResume', 'copy'],
  ['x:wisp.stuff.rowShare', 'share'], ['x:wisp.history.rowSave', 'share'],
  ['rollback', 'files'], ['x:wisp.worktrees.rowKeep', 'files'],
  ['archive', 'end'], ['unarchive', 'end'],
];
const SLOT = new Map(PLACE.map(([v, s]) => [v, s]));
const RANK = new Map(PLACE.map(([v], i) => [v, i]));
/** a plugin's own entry, by what it declared: its submenu, else its section (the old sections map onto the new groups) */
const BY_SUB: Record<TaskMenuSub, Slot> = { view: 'view', goal: 'options', copy: 'copy', files: 'files' };
const BY_SECTION: Record<TaskMenuSection, Slot> = { open: 'run', task: 'run', waiting: 'options', look: 'options', keep: 'options', place: 'mark' };

const valueOf = (e: MenuEntry): string => (e.kind === 'group' ? e.id : e.kind === 'separator' ? '' : e.value);
const slotOf = (e: TaskMenuEntry): Slot => SLOT.get(valueOf(e.entry)) ?? (e.sub ? BY_SUB[e.sub] : BY_SECTION[e.section ?? 'place']);
/** the built-ins in their listed order, a plugin's after them by its own order; a goal's items lead Task options */
const rank = (e: TaskMenuEntry): number => RANK.get(valueOf(e.entry)) ?? (e.sub === 'goal' ? -1 : 10_000);
const sorted = (xs: readonly TaskMenuEntry[]): TaskMenuEntry[] => [...xs].sort((a, b) => rank(a) - rank(b) || a.order - b.order); // stable

function sub(value: string, label: string, icon: IconName, items: readonly (MenuItem | MenuGroup)[]): (MenuItem | MenuGroup | MenuSubmenu)[] {
  // one item: no submenu for it
  if (items.length <= 1 && items.every((x) => x.kind !== 'group')) return [...items];
  return [{ kind: 'submenu', value, label, icon, items: [...items] }];
}

/** inside a submenu (the kit's menus are one level deep): a headed group, its rows flat (a group's own rows lifted) */
function group(id: string, label: string, rows: readonly (MenuItem | MenuGroup)[]): MenuGroup[] {
  const items = rows.flatMap((r): MenuItem[] => (r.kind === 'group' ? [...r.items] : [r]));
  return items.length ? [{ kind: 'group', id, label, items }] : [];
}

export function taskMenu(m: TaskMenuInput): MenuEntry[] {
  if (m.compact) return phoneMenu(m);
  const all: TaskMenuEntry[] = [...m.entries, ...(m.after ?? []).map((entry) => ({ entry, order: 0 }))];
  const at = (slot: Slot): (MenuItem | MenuGroup)[] => sorted(all.filter((e) => slotOf(e) === slot)).map((e) => e.entry);
  // Task options ›: its rows, then Lean runs (a group already), View and Files as headed groups
  const options = [...at('options'), ...group('view', m.t('shell.menu.view'), at('view')), ...group('files', m.t('shell.menu.files'), at('files'))];
  return menuSections([
    at('go'),
    [...at('run'), ...sub('sub:openin', m.t('shell.menu.openIn'), 'external', at('openin'))],
    at('mark'),
    [...sub('sub:options', m.t('shell.menu.options'), 'dots', options), ...sub('sub:copy', m.t('shell.menu.copy'), 'copy', at('copy')), ...at('share')],
    [...at('end'), ...m.danger],
  ]);
}

// the phone apps' menu (v1 #786's sections, folded): as it was before the desktop's 2026-11-18 order
const SECTIONS: readonly TaskMenuSection[] = ['open', 'task', 'waiting', 'look', 'keep', 'place'];
const SUB_SECTION: Record<TaskMenuSub, TaskMenuSection> = { view: 'look', goal: 'look', copy: 'keep', files: 'keep' };
const SUB_ORDER: Record<TaskMenuSub, number> = { view: 55, goal: 95, copy: -1000, files: 1000 };
const SUB_ICON: Record<TaskMenuSub, IconName> = { view: 'panel', goal: 'target', copy: 'copy', files: 'folder' };
const SECTION_ICON: Record<TaskMenuSection, IconName> = { open: 'external', task: 'pen', waiting: 'clock', look: 'panel', keep: 'bookmark', place: 'archive' };

/** a section's rows as one submenu: a row that was a submenu becomes a group headed with its name */
function folded(section: TaskMenuSection, rows: readonly MenuEntry[], t: (key: string) => string): MenuEntry {
  const items = rows.flatMap((e): (MenuItem | MenuGroup)[] => {
    if (e.kind === 'separator') return [];
    if (e.kind === 'submenu') {
      const inner = e.items.flatMap((x): MenuItem[] => (x.kind === 'separator' ? [] : x.kind === 'group' ? [...x.items] : [x]));
      return [{ kind: 'group', id: e.value, label: e.label, items: inner }];
    }
    return [e];
  });
  return { kind: 'submenu', value: `section:${section}`, label: t(`shell.menu.section.${section}`), icon: SECTION_ICON[section], items };
}

function phoneMenu(m: TaskMenuInput): MenuEntry[] {
  const sectionOf = (e: TaskMenuEntry): TaskMenuSection => (e.sub ? SUB_SECTION[e.sub] : (e.section ?? 'place'));
  const byOrder = <T extends { readonly order: number }>(xs: readonly T[]): T[] => [...xs].sort((a, b) => a.order - b.order);
  const groups = SECTIONS.map((section) => {
    const mine = m.entries.filter((e) => sectionOf(e) === section);
    const rows: { order: number; entry: MenuEntry }[] = mine.filter((e) => !e.sub).map((e) => ({ order: e.order, entry: e.entry }));
    for (const sub of Object.keys(SUB_ORDER) as TaskMenuSub[]) {
      if (SUB_SECTION[sub] !== section) continue;
      const items: (MenuItem | MenuSeparator | MenuGroup)[] = byOrder(mine.filter((e) => e.sub === sub)).map((e) => e.entry);
      if (items.length) rows.push({ order: SUB_ORDER[sub], entry: { kind: 'submenu', value: `sub:${sub}`, label: m.t(`shell.menu.${sub}`), icon: SUB_ICON[sub], items } });
    }
    const out = byOrder(rows).map((r) => r.entry);
    return section !== 'open' && out.length > 2 ? [folded(section, out, m.t)] : out;
  });
  return menuSections([...groups, m.danger, m.after ?? []]);
}

/** A plugin's `shell.row:menu` entry as a task's menu shows it: its words for this task, its icon and key, and where it
 *  goes. `place: 'end'` (before sections) still means just after Save for later. */
export function menuEntryOf(x: ResolvedExtension, p: ExtensionProps, t: (key: string) => string, keys = true): TaskMenuEntry {
  const bar = x.def.bar;
  const entry: MenuItem = { value: `x:${x.id}`, label: bar?.text?.(p) ?? t(bar?.label ?? x.id), icon: (bar?.icon ?? 'dots') as IconName, ...(keys && bar?.keys ? { keys: bar.keys } : {}) };
  const at = bar?.sub ? { sub: bar.sub } : bar?.section ? { section: bar.section } : bar?.place === 'end' ? { section: 'place' as const } : {};
  return { entry, ...at, order: bar?.place === 'end' && !bar.section && !bar.sub ? -15 : x.order };
}
