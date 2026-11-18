// docs/ux/2026-11-18-phone.md #6: the phone's task menu is the browser's entries in three groups (act, work on it, put
// it away), everything else one *This task* submenu away; no entry lost, none twice, in any shape more() takes.
import { expect, test } from 'vitest';
import type { MenuEntry, MenuItem } from '@wisp/ui/pure';
import { phoneTaskMenu } from '../src/frame/phone-menu.ts';

const item = (value: string, label = value): MenuItem => ({ value, label });
const t = (k: string): string => (k === 'shell.phone.thisTask' ? 'This task' : k);

/** what the browser's More holds today: sections, submenus, a headed group (Lean runs), Delete last */
const FULL: MenuEntry[] = [
  item('details'),
  item('transcript'),
  item('changes'),
  item('shell'),
  item('ship', 'Ship it…'),
  item('done', 'Mark done'),
  item('next-task'),
  item('previous-task'),
  { kind: 'separator' },
  item('rerun', 'Rerun…'),
  item('fork'),
  item('openIn', 'Open in Codex'),
  { kind: 'separator' },
  item('rename'),
  {
    kind: 'group',
    id: 'lean',
    label: 'Lean runs',
    items: [item('lean:on'), item('lean:off')],
  },
  item('later', 'Save for later'),
  {
    kind: 'submenu',
    value: 'sub:copy',
    label: 'Copy',
    items: [item('copy-id'), item('x:wisp.shell.resume')],
  },
  {
    kind: 'submenu',
    value: 'section:options',
    label: 'Task options',
    items: [
      {
        kind: 'group',
        id: 'goal',
        label: 'Goal',
        items: [item('x:wisp.goal.set')],
      },
      item('x:wisp.crew.add'),
    ],
  },
  { kind: 'separator' },
  item('archive', 'Archive'),
  { kind: 'separator' },
  item('delete', 'Delete task'),
];

const values = (es: readonly MenuEntry[]): string[] =>
  es.flatMap((e) => (e.kind === 'separator' ? [] : e.kind === 'group' ? e.items.map((x) => x.value) : e.kind === 'submenu' ? values(e.items) : [e.value]));
const top = (es: readonly MenuEntry[]): string[] => es.map((e) => (e.kind === 'separator' ? '—' : e.kind === 'group' ? `group:${e.id}` : e.value));

test('the_phone_task_menu_is_three_groups_act_work_put_away_with_the_rest_in_this_task', () => {
  const menu = phoneTaskMenu(FULL, t);
  expect(top(menu)).toEqual(['ship', 'done', '—', 'rerun', 'openIn', 'section:task', '—', 'later', 'archive', 'delete']);
  const task = menu.find((e) => e.kind === 'submenu' && e.value === 'section:task');
  expect(task && task.kind === 'submenu' && task.label).toBe('This task');
  // a submenu or headed group that was there stays a headed group inside This task (Copy, Task options, Lean runs)
  const heads = task && task.kind === 'submenu' ? task.items.flatMap((x) => (x.kind === 'group' ? [x.label] : [])) : [];
  expect(heads).toEqual(['Lean runs', 'Copy', 'Task options']);
});

test('every_entry_of_the_browsers_menu_is_in_the_phones_once', () => {
  const before = values(FULL).sort();
  const after = values(phoneTaskMenu(FULL, t)).sort();
  expect(after).toEqual(before);
  expect(new Set(after).size).toBe(after.length);
});

test('a_menu_without_this_tasks_rest_has_no_empty_submenu_and_no_empty_group', () => {
  const menu = phoneTaskMenu([item('ship'), item('done'), { kind: 'separator' }, item('delete')], t);
  expect(top(menu)).toEqual(['ship', 'done', '—', 'delete']);
  expect(phoneTaskMenu([], t)).toEqual([]);
});
