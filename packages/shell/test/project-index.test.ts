// perf-r1 #2: a task's project is looked up through one index per projects list, not a regex, a filter and a sort over
// every project per task (⌘K, the Projects counts and the need-you counts ask once per task on every rebuild).
import { expect, test } from 'vitest';
import { projectCounts, projectOf, projectOfRow, type ProjectView } from '../src/frame/rows-view.ts';

const p = (id: string, path: string): ProjectView => ({ id, name: id, path, color: '' });

/** The lookup as it was: every project's root normalized, filtered and sorted on every call. */
function projectOfBefore(cwd: string | undefined, projects: readonly ProjectView[]): ProjectView | undefined {
  if (!cwd) return undefined;
  const inside = (root: string): boolean => {
    const r = root.replace(/\/+$/, '');
    return !!r && (cwd === r || cwd.startsWith(`${r}/`));
  };
  return projects.filter((x) => inside(x.path)).sort((a, b) => b.path.length - a.path.length)[0];
}

test('the_project_of_a_folder_is_the_deepest_that_holds_it_with_trailing_slashes_and_no_prefix_match', () => {
  const list = [p('w', '/w'), p('api', '/w/api/'), p('docs', '/w/api/docs'), p('root', '/'), p('none', '')];
  expect(projectOf('/w/api/docs/src', list)?.id).toBe('docs');
  expect(projectOf('/w/api/docs', list)?.id).toBe('docs');
  expect(projectOf('/w/api', list)?.id).toBe('api');
  expect(projectOf('/w/api/', list)?.id).toBe('api');
  expect(projectOf('/w/apiary', list)?.id).toBe('w');
  expect(projectOf('/wx', list)).toBeUndefined();
  expect(projectOf('', list)).toBeUndefined();
  expect(projectOf(undefined, list)).toBeUndefined();
  expect(projectOf('/elsewhere', list)).toBeUndefined();
  // asked twice: the same answer from what's kept
  expect(projectOf('/w/api/docs/src', list)?.id).toBe('docs');
  expect(projectOf('/wx', list)).toBeUndefined();
});

test('a_worktree_root_beats_the_folder_and_the_name_is_the_last_resort', () => {
  const list = [p('web', '/w/web'), p('api', '/w/api')];
  const wt = { summary: { cwd: '/tmp/wt/abc' }, body: { ext: { 'wisp.worktrees': { root: '/w/api' } } } };
  expect(projectOfRow(wt, list)?.id).toBe('api');
  expect(projectOfRow({ summary: { cwd: '/w/web/src', project: 'api' } }, list)?.id).toBe('web');
  expect(projectOfRow({ summary: { cwd: '/nowhere', project: 'api' } }, list)?.id).toBe('api');
  expect(projectOfRow({ summary: { project: 'gone' } }, list)).toBeUndefined();
});

test('the_index_answers_as_the_old_lookup_did_on_many_folders', () => {
  const projects: ProjectView[] = [];
  for (let i = 0; i < 40; i++) projects.push(p(`p${i}`, `/r/${i % 7}${i % 3 ? `/s${i}` : ''}${i % 5 ? '' : '/'}`));
  for (let i = 0; i < 500; i++) {
    const cwd = `/r/${i % 9}${i % 4 ? `/s${i % 45}` : ''}${i % 6 ? '/deep' : ''}${i % 11 ? '' : 'er'}`;
    expect(projectOf(cwd, projects)?.id, cwd).toBe(projectOfBefore(cwd, projects)?.id);
  }
});

test('a_new_projects_list_is_never_answered_from_the_old_lists_index', () => {
  const before = [p('web', '/w/web')];
  expect(projectOf('/w/api/src', before)).toBeUndefined();
  const after = [...before, p('api', '/w/api')];
  expect(projectOf('/w/api/src', after)?.id).toBe('api');
  expect(projectOf('/w/api/src', before)).toBeUndefined();
});

test('five_thousand_tasks_across_two_hundred_projects_are_counted_in_a_few_ms', () => {
  const projects = Array.from({ length: 200 }, (_, i) => p(`p${i}`, `/Users/me/code/group${i % 20}/project-${i}`));
  const sessions = Array.from({ length: 5000 }, (_, i) => ({
    summary: { cwd: `/Users/me/code/group${i % 20}/project-${i % 300}/src/${i % 2 ? 'lib' : 'app'}` },
  }));
  // warm the JIT on another list, so the measured run is the lookup, not the first compile
  projectCounts(sessions.slice(0, 200), [...projects]);
  const at = performance.now();
  const counts = projectCounts(sessions, projects);
  const ms = performance.now() - at;
  expect([...counts.values()].reduce((a, b) => a + b, 0)).toBe(sessions.filter((s) => projectOfBefore(s.summary.cwd, projects)).length);
  // the old task × project walk took ~170 ms here; 50 ms keeps that regression caught while a loaded CI runner
  // (seen at 12.7 ms) doesn't fail the suite on noise
  expect(ms).toBeLessThan(50);
});
