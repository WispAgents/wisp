// A task's menu (src/task-menu.ts; docs/ux/2026-11-18-desktop.md #6, decided by the owner's delegate): one order for More
// and the row's right-click: each menu's own head, then 4 groups (Rerun, Fork, Open in ›; Rename, Pin, Mark unread,
// Save for later; Task options ›, Copy ›, Share…; Archive, Delete last), a submenu of one inline. (Was v1 #786's six
// sections: every entry is still there, in fewer rows.)
import { expect, test } from 'vitest';
import type { MenuEntry } from '@wisp/ui/pure';
import { menuEntryOf, taskMenu, type TaskMenuEntry } from '../src/task-menu.ts';
import type { ResolvedExtension } from '../src/contract.ts';

const t = (k: string): string => k;
const it = (value: string, at: Omit<TaskMenuEntry, 'entry'>): TaskMenuEntry => ({ entry: { value, label: value }, ...at });
/** the menu as text: separators as '|', a submenu as 'name[items]' */
const shape = (m: readonly MenuEntry[]): string[] =>
  m.map((e) => (e.kind === 'separator' ? '|' : e.kind === 'submenu' ? `${e.label}[${e.items.map((x) => ('value' in x ? x.value : x.kind)).join(',')}]` : e.kind === 'group' ? `(${e.id})` : e.value));

const lean = { entry: { kind: 'group' as const, id: 'lean', label: 'Lean runs', items: [{ value: 'lean:follow', label: 'As in Settings' }, { value: 'lean:on', label: 'On' }, { value: 'lean:off', label: 'Off' }] }, section: 'task' as const, order: 46 };
// the built-ins as they attach (row-menu.ts, ship, goal, crew, threads, stuff, worktrees, automations, shell-task)
const builtIns = (): TaskMenuEntry[] => [
  it('open', { section: 'open', order: -10 }),
  it('x:shell.row.rerun', { section: 'open', order: 10 }),
  it('rename', { section: 'task', order: -10 }),
  it('x:shell.row.fork', { section: 'task', order: 20 }),
  it('x:shell.row.side', { section: 'task', order: 30 }),
  it('x:wisp.stuff.rowShare', { section: 'task', order: 40 }),
  it('x:wisp.threads.menu', { section: 'look', order: 40 }),
  lean,
  it('x:wisp.ship.rowShip', { section: 'waiting', order: 50 }),
  it('x:wisp.ship.rowMerge', { section: 'waiting', order: 52 }),
  it('x:shell.row.changes', { sub: 'view', order: 60 }),
  it('x:shell.row.shell', { sub: 'view', order: 70 }),
  it('x:wisp.ship.rowReview', { sub: 'view', order: 75 }),
  it('x:shell-task.rowRun', { sub: 'view', order: 80 }),
  it('x:shell.row.compare', { sub: 'view', order: 86 }),
  it('x:wisp.goal.set', { sub: 'goal', order: 110 }),
  it('x:wisp.crew.rowCrew', { section: 'look', order: 100 }),
  it('copy-id', { sub: 'copy', order: -20 }),
  it('x:shell.row.copyResume', { sub: 'copy', order: -10 }),
  it('x:wisp.automations.rowRecipe', { section: 'keep', order: 90 }),
  it('x:wisp.history.rowSave', { section: 'keep', order: 95 }),
  it('x:shell.row.editor', { sub: 'files', order: 110 }),
  it('x:wisp.worktrees.rowKeep', { sub: 'files', order: 120 }),
  it('later', { section: 'place', order: -20 }),
  it('archive', { section: 'place', order: -10 }),
];

test('one_order_the_heads_own_then_four_groups_with_submenus_and_archive_and_delete_last', () => {
  const m = taskMenu({ entries: builtIns(), danger: [{ value: 'delete', label: 'delete', danger: true }], after: [{ value: 'pin', label: 'pin' }, { value: 'read', label: 'read' }], t });
  expect(shape(m)).toEqual([
    // the row's own head: Open, then the panes
    'open',
    'x:shell.row.changes',
    'x:wisp.ship.rowReview',
    'x:shell.row.shell',
    '|',
    // 1: run it again, or open it elsewhere
    'x:shell.row.rerun',
    'x:shell.row.fork',
    'x:shell.row.editor',
    '|',
    // 2: mark it
    'rename',
    'pin',
    'read',
    'later',
    '|',
    // 3: its options (one submenu), its copies, Share, Save to repo
    'shell.menu.options[x:wisp.goal.set,x:shell.row.side,x:wisp.crew.rowCrew,x:wisp.automations.rowRecipe,x:wisp.threads.menu,x:wisp.ship.rowShip,x:wisp.ship.rowMerge,group,group,group]',
    'shell.menu.copy[copy-id,x:shell.row.copyResume]',
    'x:wisp.stuff.rowShare',
    'x:wisp.history.rowSave',
    '|',
    // 4: always last
    'archive',
    'delete',
  ]);
  // inside Task options: Lean runs, View and Files as headed groups (the kit's menus are one level deep)
  const options = m.find((e) => e.kind === 'submenu' && e.value === 'sub:options');
  const groups = options?.kind === 'submenu' ? options.items.filter((x) => x.kind === 'group') : [];
  expect(groups.map((g) => (g.kind === 'group' ? `${g.label}:${g.items.map((i) => i.value).join(',')}` : ''))).toEqual([
    'Lean runs:lean:follow,lean:on,lean:off',
    'shell.menu.view:x:shell-task.rowRun,x:shell.row.compare',
    'shell.menu.files:x:wisp.worktrees.rowKeep',
  ]);
});

test('a_plugins_own_entry_goes_by_what_it_declared_and_a_submenu_of_one_stays_inline', () => {
  const m = taskMenu({
    entries: [it('rename', { section: 'task', order: 0 }), it('copy-id', { sub: 'copy', order: 0 }), it('x:p.launch', { section: 'open', order: 5 }), it('x:p.tag', { order: 5 }), it('x:p.note', { section: 'look', order: 5 }), it('x:p.peek', { sub: 'view', order: 5 })],
    danger: [{ value: 'delete', label: 'delete' }],
    t,
  });
  // open → with Rerun; no section → with the marks; look → Task options (here one row, so inline); copy of one inline
  expect(shape(m)).toEqual(['x:p.launch', '|', 'rename', 'x:p.tag', '|', 'shell.menu.options[x:p.note,group]', 'copy-id', '|', 'delete']);
});

test('menuEntryOf: section and sub from bar; legacy place end sits after Save for later_v1_786_task_menu_entry_of', () => {
  const ext = (bar: Record<string, unknown>, order = 50): ResolvedExtension => ({ id: 'p.x', plugin: 'p', config: {}, order, def: { id: 'p.x', attachTo: { id: 'shell.row:menu' }, component: () => null, bar: { icon: 'x', label: 'p.label', priority: order, ...bar } } as never });
  expect(menuEntryOf(ext({ sub: 'files' }, 120), {} as never, t)).toMatchObject({ sub: 'files', order: 120, entry: { value: 'x:p.x', label: 'p.label' } });
  expect(menuEntryOf(ext({ section: 'task', keys: 'K' }), {} as never, t, false).entry).not.toHaveProperty('keys');
  expect(menuEntryOf(ext({ place: 'end' }, 120), {} as never, t)).toMatchObject({ section: 'place', order: -15 });
  expect(menuEntryOf(ext({}), {} as never, t)).not.toHaveProperty('section');
});

test('on_a_phone_the_menu_stays_short_qa_r2_5', () => {
  // the phone's ⋯ ran 27 rows deep (iOS HIG / M3: sections and submenus keep a menu short); the 2026-11-18 order is the
  // desktop's, the phone keeps its folded sections
  const m = taskMenu({ entries: builtIns(), danger: [{ value: 'delete', label: 'delete', danger: true }], t, compact: true });
  expect(m.filter((e) => e.kind !== 'separator').length).toBeLessThanOrEqual(16);
});

test('the_phone_keeps_its_folded_sections_not_the_desktops_order', () => {
  const m = taskMenu({ entries: builtIns(), danger: [{ value: 'delete', label: 'delete', danger: true }], t, compact: true });
  expect(m.some((e) => e.kind === 'submenu' && e.value.startsWith('section:'))).toBe(true);
  expect(m.some((e) => e.kind === 'submenu' && e.value === 'sub:options')).toBe(false);
});
