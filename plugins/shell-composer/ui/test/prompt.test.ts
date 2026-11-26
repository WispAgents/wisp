import { expect, test } from 'vitest';
import { RETRY_MS, cached, crewItems, slashItems, stuffItems, taskItems, fileItems } from '../src/prompt.ts';
import { HOME_SLASH, SLASH } from '../src/text/slash.ts';

test('the_at_menu_names_tasks_crew_and_stuff_by_what_is_typed_and_the_slash_menu_its_commands', () => {
  const tasks = [
    { id: 'laptop:a', title: 'Fix the login bug', updated_at: 1 },
    { id: 'laptop:b', title: 'Fix the flaky test', updated_at: 5 },
    { id: 'laptop:c', title: 'Fix old thing', updated_at: 9, archived: true },
    { id: 'laptop:d', title: 'Write docs', updated_at: 3 },
  ];
  // by the words, newest first, never archived, never the open task
  expect(taskItems(tasks, 'fix').map((x) => x.label)).toEqual(['Fix the flaky test', 'Fix the login bug']);
  expect(taskItems(tasks, 'fix', 'laptop:b').map((x) => x.label)).toEqual(['Fix the login bug']);
  expect(taskItems(tasks, 'login')[0]).toMatchObject({ insert: '@Fix the login bug ', mention: { scheme: 'wisp', id: 'a', title: 'Fix the login bug' } });
  expect(crewItems([{ id: 'ada', name: 'Ada', role: 'Reviews every PR' }], 'ad')[0]).toMatchObject({ label: 'Ada', detail: 'Reviews every PR', mention: { scheme: 'crew', id: 'ada' } });
  expect(stuffItems([{ id: 's1', title: 'Release checklist', kind: 'app' }], 'release')[0]).toMatchObject({ insert: '@Release checklist ', mention: { scheme: 'stuff', id: 's1' } });
  // commands: the reply box's own, New task's own; a bare command inserts no space
  const t = (k: string): string => k;
  expect(slashItems(SLASH, 'g', t).map((x) => x.label)).toEqual(['/goal']);
  expect(slashItems(SLASH, 'g', t)[0]?.insert).toBe('/goal');
  expect(slashItems(SLASH, 'bt', t)[0]?.insert).toBe('/btw ');
  expect(slashItems(HOME_SLASH, 'bt', t)).toEqual([]);
  expect(slashItems(HOME_SLASH, 'te', t).map((x) => x.label)).toEqual(['/terminal']);
});

test('a_tasks_files_in_the_at_menu_insert_their_path_and_open_in_the_file_view', async () => {
  const opened: string[] = [];
  const items = fileItems(['src/auth/login.ts', 'src/app.ts', 'docs/login-flow.md', 'README.md'], 'login', (f) => opened.push(f));
  expect(items.map((i) => i.value)).toEqual(['file:src/auth/login.ts', 'file:docs/login-flow.md']);
  expect(items[0]).toMatchObject({ label: 'login.ts', detail: 'src/auth', insert: '@src/auth/login.ts ', icon: 'file' });
  items[0]?.open?.();
  expect(opened).toEqual(['src/auth/login.ts']);
  expect(fileItems(['a.ts'], 'zzz', () => undefined)).toEqual([]);
});

// round 324 review: the @ menu reads its sources again on every store change while it's open (a running task's patches
// included); a task whose files can't be listed (its worktree gone) is asked again only after RETRY_MS, not per patch
test('a_refused_file_list_is_asked_again_only_after_a_pause_not_on_every_read', async () => {
  let at = 1_000;
  let asked = 0;
  let ok = false;
  const ask = (): Promise<{ files: string[] }> => {
    asked += 1;
    return ok ? Promise.resolve({ files: ['src/a.ts'] }) : Promise.reject(new Error('no worktree'));
  };
  const read = (): readonly string[] => cached('laptop:gone', ask, () => at);
  expect(read()).toEqual([]);
  await Promise.resolve();
  await Promise.resolve();
  for (let i = 0; i < 50; i++) read(); // a stream's patches while the menu shows
  expect(asked, 'refused once: not asked again on each read').toBe(1);
  at += RETRY_MS;
  ok = true;
  read();
  expect(asked, 'after the pause it asks again').toBe(2);
  await Promise.resolve();
  expect(read()).toEqual(['src/a.ts']);
});

// new1: a mention is read on the computer the task runs on, so @ offers only the target computer's tasks
test('the_at_menu_offers_only_the_tasks_of_the_computer_the_box_sends_to_new1', () => {
  const tasks = [
    { id: 'a:t1', title: 'Fix login', updated_at: 3 },
    { id: 'b:t2', title: 'Fix login on b', updated_at: 9 },
    { id: 'a:t3', title: 'Fix login', updated_at: 1 },
  ];
  const items = taskItems(tasks, 'fix', undefined, 'a', 'Laptop');
  // same-titled tasks on one computer: both
  expect(items.map((x) => x.value)).toEqual(['task:a:t1', 'task:a:t3']);
  expect(items[0]).toMatchObject({ detail: 'Laptop', mention: { scheme: 'wisp', id: 't1', title: 'Fix login' } });
  expect(taskItems(tasks, 'fix', undefined, 'b').map((x) => x.mention?.id)).toEqual(['t2']);
  // one computer: no name on the rows
  expect(taskItems(tasks, 'fix', undefined, 'a')[0]?.detail).toBeUndefined();
});

test('a_task_in_the_at_list_has_the_tasks_icon_not_the_crews_qa_r1_27', () => {
  const tasks = [{ id: 'laptop:a', title: 'Fix the login bug' }];
  // 'agent' is the two-people glyph (crew, delegates): a task is the list's own mark, as the Tasks tab shows it
  expect(taskItems(tasks as never, '').map((x) => x.icon)).toEqual(['feed']);
  expect(crewItems([{ id: 'c1', name: 'Ada' }] as never, '').map((x) => x.icon)).toEqual(['crew']);
});
