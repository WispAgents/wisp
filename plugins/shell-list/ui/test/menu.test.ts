// A task's row menu (model/menu.ts; docs/ux/2026-11-18-desktop.md #6): the list's own entries and the ones plugins attach
// to `shell.row:menu` in the task menu's one order (task-menu.ts): Open and the panes, Rerun/Fork/Open in ›, Rename/Pin/
// Mark unread/Save for later, Task options ›/Copy ›/Share…, Archive and Delete last.
import { expect, test } from 'vitest';
import type { TaskMenuEntry } from '@wisp/shell/pure';
import type { MenuEntry } from '@wisp/ui/pure';
import { rowMenu } from '../src/model/menu.ts';
import type { SessionRow } from '../src/types.ts';

const t = (k: string): string => k.replace('shell-list.menu.', '').replace('shell.menu.', '');
const row = (o: Partial<SessionRow> = {}): SessionRow => ({ id: 's1', title: 'Write docs', agent: 'claude', state: 'review', createdAt: 0, updatedAt: 0, ...o }) as SessionRow;
const values = (m: readonly MenuEntry[]): string[] =>
  m.map((e) => (e.kind === 'separator' ? '|' : e.kind === 'submenu' ? `${e.label}[${e.items.map((x) => ('value' in x ? x.value : x.kind)).join(',')}]` : 'value' in e ? e.value : e.kind));
const x = (v: string, at: Omit<TaskMenuEntry, 'entry'>): TaskMenuEntry => ({ entry: { value: `x:${v}`, label: v }, ...at });
// the built-ins' real ids (row-menu.ts, ship, goal, crew, worktrees…): the menu places them by id

// what the built-ins attach, with their bar.section / bar.sub and order (row-menu.ts, ship, goal, crew, worktrees…)
const plugins: TaskMenuEntry[] = [
  x('shell.row.rerun', { section: 'open', order: 10 }),
  x('shell.row.fork', { section: 'task', order: 20 }),
  x('shell.row.side', { section: 'task', order: 30 }),
  x('wisp.stuff.rowShare', { section: 'task', order: 40 }),
  x('wisp.ship.rowShip', { section: 'waiting', order: 50 }),
  x('wisp.ship.rowMerge', { section: 'waiting', order: 52 }),
  x('shell.row.changes', { sub: 'view', order: 60 }),
  x('shell.row.shell', { sub: 'view', order: 70 }),
  x('wisp.ship.rowReview', { sub: 'view', order: 75 }),
  x('shell-task.rowRun', { sub: 'view', order: 80 }),
  x('wisp.automations.rowRecipe', { section: 'keep', order: 90 }),
  x('wisp.crew.rowCrew', { section: 'look', order: 100 }),
  x('wisp.goal.set', { sub: 'goal', order: 110 }),
  x('wisp.goal.checks', { sub: 'goal', order: 111 }),
  x('wisp.worktrees.rowKeep', { sub: 'files', order: 120 }),
];

test('one_order_open_and_the_panes_then_four_groups_archive_and_delete_last_ux_6', () => {
  expect(values(rowMenu({ s: row(), t, plugins, keys: true }))).toEqual([
    'open',
    'x:shell.row.changes',
    'x:wisp.ship.rowReview',
    'x:shell.row.shell',
    '|',
    'x:shell.row.rerun',
    'x:shell.row.fork',
    '|',
    'rename',
    'pin',
    'read',
    'later',
    '|',
    'options[x:wisp.goal.set,x:wisp.goal.checks,x:shell.row.side,x:wisp.crew.rowCrew,x:wisp.automations.rowRecipe,x:wisp.ship.rowShip,x:wisp.ship.rowMerge,group,group]',
    'copy-id',
    'x:wisp.stuff.rowShare',
    '|',
    'archive',
    'delete',
  ]);
});

test('a busy task: no Archive, no Save for later, no Mark read; Delete still last_ux_6_row_menu_busy', () => {
  expect(values(rowMenu({ s: row({ state: 'running' }), t, plugins: [], keys: true }))).toEqual(['open', '|', 'rename', 'pin', '|', 'copy-id', '|', 'delete']);
});

test('no key hints on touch screens_v1_786_row_menu_keys', () => {
  const m = rowMenu({ s: row(), t, plugins: [], keys: false });
  expect(m.some((e) => 'keys' in e && e.keys)).toBe(false);
});

// TRY-17 #30: the open row's menu got the server's row (its view under `summary`), so a pinned task still offered Pin
// (and a saved or unread one read as neither); the menu reads the row as the list does
test('a_server_rows_menu_reads_its_summary_a_pinned_task_offers_unpin', () => {
  const server = { id: 's1', title: 'T', agent: 'codex', state: 'review', created_at: 0, updated_at: 0, body: {}, summary: { status: 'done', pinned: true, unread: true } } as unknown as SessionRow;
  const flat = (m: readonly MenuEntry[]): string[] => m.flatMap((e) => (e.kind === 'submenu' ? e.items : [e])).flatMap((e) => ('label' in e && 'value' in e ? [String(e.label)] : []));
  const labels = flat(rowMenu({ s: server, t, plugins: [], keys: false }));
  expect(labels).toContain('unpin');
  expect(labels).toContain('mark-read');
});
