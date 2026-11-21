// docs/ux/2026-11-18-phone.md #6, QA r6 #6: the phone's task menu is three groups of nine rows: act (Ship it…, Mark
// done), work on it (Rerun…, This task ›, Show ›, Keep and copy ›), put it away (Save for later, Archive, Delete).
// Every entry of the browser's menu is in it once, in the submenu its section says; a choice keeps its name.
import { expect, test } from 'vitest';
import type { MenuEntry, MenuItem, MenuSubmenu } from '@wisp/ui/pure';
import { phoneTaskMenu } from '../src/frame/phone-menu.ts';

const item = (value: string, label = value): MenuItem => ({ value, label });
const T: Record<string, string> = { 'shell.phone.thisTask': 'This task', 'shell.phone.show': 'Show', 'shell.phone.keep': 'Keep and copy' };
const t = (k: string): string => T[k] ?? k;

/** what the browser's More holds on a phone today (task-menu.ts compact: sections past the first folded) */
const FULL: MenuEntry[] = [
  item('details', 'Details'),
  item('transcript', 'Transcript'),
  item('changes', 'Changes'),
  item('shell', 'Shell'),
  item('ship', 'Ship it…'),
  item('done', 'Mark done'),
  item('rerun', 'Rerun…'),
  item('fork', 'Fork…'),
  item('openIn', 'Open in Codex'),
  { kind: 'separator' },
  {
    kind: 'submenu',
    value: 'section:task',
    label: 'This task',
    items: [item('rename', 'Rename'), item('x:wisp.threads.side', 'Side discussion'), item('x:wisp.share.task', 'Share…'), { kind: 'group', id: 'lean', label: 'Lean runs', items: [{ ...item('lean:follow', 'As in Settings'), checked: true }, { ...item('lean:on', 'On for this task'), checked: false }] }, item('x:wisp.ship.rowMerge', 'Merge PR')],
  },
  {
    kind: 'submenu',
    value: 'section:look',
    label: 'Show',
    items: [item('x:wisp.threads.thread', 'Thread'), item('x:wisp.ship.reviewPr', 'Review PR'), item('x:wisp.preview.run', 'Run app'), item('x:wisp.goal.set', 'Set goal…'), item('x:wisp.crew.add', 'Add to crew…'), item('next-task', 'Next task'), item('previous-task', 'Previous task')],
  },
  { kind: 'submenu', value: 'section:keep', label: 'Keep and copy', items: [{ kind: 'group', id: 'sub:copy', label: 'Copy', items: [item('copy-id', 'Copy task ID'), item('x:wisp.shell.resume', 'Copy resume command')] }, item('x:wisp.worktrees.keep', 'Keep worktree')] },
  { kind: 'separator' },
  item('later', 'Save for later'),
  item('archive', 'Archive'),
  { kind: 'separator' },
  item('delete', 'Delete task'),
];

const flat = (es: readonly MenuEntry[]): string[] => es.flatMap((e) => (e.kind === 'separator' ? [] : e.kind === 'group' ? e.items.map((x) => x.value) : e.kind === 'submenu' ? flat(e.items) : [e.value]));
const top = (es: readonly MenuEntry[]): string[] => es.map((e) => (e.kind === 'separator' ? '—' : e.kind === 'group' ? `group:${e.id}` : e.value));
const subOf = (es: readonly MenuEntry[], v: string): MenuSubmenu | undefined => es.find((e): e is MenuSubmenu => e.kind === 'submenu' && e.value === v);

test('the_phone_task_menu_is_three_groups_of_nine_rows', () => {
  const menu = phoneTaskMenu(FULL, t);
  expect(top(menu)).toEqual(['ship', 'done', '—', 'rerun', 'section:task', 'section:show', 'section:keep', '—', 'later', 'archive', 'delete']);
  expect(top(menu).filter((x) => x !== '—')).toHaveLength(9);
  expect(menu.flatMap((e) => (e.kind === 'submenu' ? [e.label] : []))).toEqual(['This task', 'Show', 'Keep and copy']);
});

test('each_entry_lands_in_the_submenu_its_section_says_the_goal_and_crew_with_the_task', () => {
  const menu = phoneTaskMenu(FULL, t);
  expect(flat(subOf(menu, 'section:task')?.items ?? [])).toEqual(['details', 'openIn', 'rename', 'x:wisp.threads.side', 'lean:follow', 'lean:on', 'x:wisp.ship.rowMerge', 'x:wisp.goal.set', 'x:wisp.crew.add']);
  expect(flat(subOf(menu, 'section:show')?.items ?? [])).toEqual(['transcript', 'changes', 'shell', 'x:wisp.threads.thread', 'x:wisp.ship.reviewPr', 'x:wisp.preview.run', 'next-task', 'previous-task']);
  expect(flat(subOf(menu, 'section:keep')?.items ?? [])).toEqual(['fork', 'x:wisp.share.task', 'copy-id', 'x:wisp.shell.resume', 'x:wisp.worktrees.keep']);
  // a choice of several keeps its name on each row (a phone's drill-in draws no group heading)
  const labels = (subOf(menu, 'section:task')?.items ?? []).flatMap((e) => (e.kind === undefined || e.kind === 'item' ? [e.label] : []));
  expect(labels).toContain('Lean runs · As in Settings');
  // a plain group (Copy, View) is just its rows: no "Copy · Copy task ID"
  const kept = (subOf(menu, 'section:keep')?.items ?? []).flatMap((e) => (e.kind === undefined || e.kind === 'item' ? [e.label] : []));
  expect(kept).toContain('Copy task ID');
  expect(kept.filter((l) => l.includes(' · '))).toEqual([]);
});

test('every_entry_of_the_browsers_menu_is_in_the_phones_once', () => {
  const before = flat(FULL).sort();
  const after = flat(phoneTaskMenu(FULL, t)).sort();
  expect(after).toEqual(before);
  expect(new Set(after).size).toBe(after.length);
});

test('a_short_menu_has_no_empty_submenu', () => {
  expect(top(phoneTaskMenu([item('ship'), item('done'), { kind: 'separator' }, item('delete')], t))).toEqual(['ship', 'done', '—', 'delete']);
  expect(phoneTaskMenu([], t)).toEqual([]);
});
