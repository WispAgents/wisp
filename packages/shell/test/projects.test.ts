// v1 #772: one stored project order used wherever projects show; a task belongs to the project that holds its folder.
import { expect, test } from 'vitest';
import { movedOrder, projectCounts, projectList, projectOf } from '../src/frame/rows-view.ts';
import { projectRemoveAsk } from '../src/frame/project-remove.ts';
import { addedName, folderOf, nameFor, whereIs } from '../src/frame/project-names.ts';

test('projects_read_from_their_rows_in_your_order_and_a_task_is_in_the_project_holding_its_folder', () => {
  const rows = [
    { id: 'web', body: { path: '/w/web', color: 'teal' } },
    { id: 'api', body: { path: '/w/api' } },
    { id: 'api-docs', body: { path: '/w/api/docs' } },
  ];
  // never placed: by name
  expect(projectList(rows, undefined).map((p) => p.id)).toEqual(['api', 'api-docs', 'web']);
  // your order first, the rest after by name; a stale name in the order is ignored
  const list = projectList(rows, ['web', 'gone', 'api-docs']);
  expect(list.map((p) => p.id)).toEqual(['web', 'api-docs', 'api']);
  expect(list[0]).toEqual({ id: 'web', name: 'web', path: '/w/web', color: 'teal' });
  // the deepest project that holds the folder; a sibling with the same prefix doesn't
  expect(projectOf('/w/api/docs/src', list)?.id).toBe('api-docs');
  expect(projectOf('/w/api', list)?.id).toBe('api');
  expect(projectOf('/w/apiary', list)).toBeUndefined();
  expect(projectOf(undefined, list)).toBeUndefined();
  // drag: before the one dropped on, or to the end
  expect(movedOrder(list, 'api', 'web')).toEqual(['api', 'web', 'api-docs']);
  expect(movedOrder(list, 'web', undefined)).toEqual(['api-docs', 'api', 'web']);
});

test('a_new_projects_name_comes_from_its_folder_or_url_and_never_takes_one_in_use', () => {
  expect(nameFor('/Users/me/code/web-app/', [])).toBe('web-app');
  expect(nameFor('https://github.com/acme/api.git', [])).toBe('api');
  expect(nameFor('git@github.com:acme/api.git', ['api', 'api-2'])).toBe('api-3');
  expect(nameFor('/w/My App', [])).toBe('My-App');
});

test('a_found_repo_shows_where_it_is_as_v1_did', () => {
  expect(whereIs('/Users/me/code/web')).toBe('~/code');
  expect(whereIs('/home/me/src/api/')).toBe('~/src');
  expect(whereIs('/opt/repos/x')).toBe('/opt/repos');
});

test('the optimistic row of an added folder guesses runtime.projectAdd s name, never a taken one_adr0077_guess', () => {
  expect(addedName('/a/wisp', [])).toBe('wisp');
  expect(addedName('/b/wisp/', ['wisp'])).toBe('wisp-2');
  expect(addedName('/w/My App', [])).toBe('my-app');
  expect(addedName('C:\\work\\Api.git', ['api'])).toBe('api-2');
  expect(addedName('/c/server', ['api'], ' api ')).toBe('api-2');
  expect(addedName('/', [])).toBe('project');
  // `/repo/.` is `/repo`: the guess is `repo`, never `.` (and Projects finds it's a project already)
  expect(addedName('/w/repo/.', [])).toBe('repo');
  expect(folderOf('/w/repo/./')).toBe('/w/repo');
  expect(folderOf('C:\\w\\repo\\')).toBe('C:\\w\\repo');
});

// audit C1: Manage projects read "0 tasks" for projects with tasks: it matched each task's cwd, and a worktree task's
// cwd is outside the project (home/worktrees/<project>-<id>). A task counts for the project it belongs to.
test('manage_projects_counts_worktree_tasks_by_the_project_they_belong_to', () => {
  const list = projectList([{ id: 'repo', body: { path: '/w/repo' } }, { id: 'repo2', body: { path: '/w/repo2' } }], undefined);
  const sessions = [
    { summary: { cwd: '/h/worktrees/repo-a1', project: 'repo' }, body: { ext: { 'wisp.worktrees': { root: '/w/repo' } } } },
    { summary: { cwd: '/h/worktrees/repo2-b2', project: 'repo2' }, body: {} },
    { summary: { cwd: '/w/repo/src' }, body: {} },
    { summary: { cwd: '/elsewhere' }, body: {} },
  ];
  const n = projectCounts(sessions, list);
  expect(n.get('repo')).toBe(2);
  expect(n.get('repo2')).toBe(1);
  expect([...n.keys()].sort()).toEqual(['repo', 'repo2']);
});

// audit C2: Remove took one click. v1 asked first (projects.js: "Remove “x” from wisp? Its tasks and files are kept.").
test('removing_a_project_asks_first_and_says_its_tasks_and_files_are_kept', () => {
  const say = (k: string, v?: Readonly<Record<string, string | number>>): string => `${k}${v ? JSON.stringify(v) : ''}`;
  expect(projectRemoveAsk('repo2', 3, say)).toEqual({
    title: 'shell.projects.removeAsk.title{"name":"repo2"}',
    body: 'shell.projects.removeAsk.body{"count":3}',
    ok: 'shell.projects.removeAsk.ok',
  });
  expect(projectRemoveAsk('one', 1, say).body).toBe('shell.projects.removeAsk.bodyOne');
  expect(projectRemoveAsk('empty', 0, say).body).toBe('shell.projects.removeAsk.bodyNone');
});
