// r285: the phone's project chips read "All · wisp · wisp" for two projects of one name; each now says where it is.
import { expect, test } from 'vitest';
import { projectLabels } from '../src/frame/project-labels.ts';
import { projectList } from '../src/frame/rows-view.ts';

const names: Record<string, string> = { local: 'This computer', studio: 'Studio' };
const computer = (p: string): string => names[p] ?? p;

test('same_named_projects_on_two_computers_name_their_computer', () => {
  const ps = [
    { name: 'wisp', path: '/Users/me/code/wisp', peer: 'local' },
    { name: 'wisp', path: '/Users/me/code/wisp', peer: 'studio' },
    { name: 'api', path: '/Users/me/code/api', peer: 'local' },
  ];
  expect(projectLabels(ps, computer)).toEqual(['wisp (This computer)', 'wisp (Studio)', 'api']);
});

test('same_named_projects_in_two_folders_name_the_folder_they_sit_in_as_v1_showed_it', () => {
  const ps = [
    { name: 'wisp', path: '/Users/me/code/wisp', peer: 'local' },
    { name: 'wisp', path: '/Users/me/Downloads/wisp', peer: 'local' },
  ];
  expect(projectLabels(ps, computer)).toEqual(['wisp (~/code)', 'wisp (~/Downloads)']);
});

test('same_named_projects_apart_by_computer_and_folder_name_both', () => {
  const ps = [
    { name: 'wisp', path: '/Users/me/code/wisp', peer: 'local' },
    { name: 'wisp', path: '/srv/wisp', peer: 'studio' },
  ];
  expect(projectLabels(ps, computer)).toEqual(['wisp (This computer · ~/code)', 'wisp (Studio · /srv)']);
});

test('a_name_only_one_project_has_stays_as_it_is', () => {
  expect(projectLabels([{ name: 'web', path: '/w/web' }, { name: 'api', path: '/w/api' }], computer)).toEqual(['web', 'api']);
});

test('a_project_knows_its_computer_from_its_entity_key', () => {
  const rows = [
    { id: 'wisp', name: 'wisp', path: '/a/wisp' },
    { id: 'wisp', name: 'wisp', path: '/a/wisp' },
  ];
  expect(projectList(rows, undefined, ['local:wisp', 'studio:wisp']).map((p) => p.peer)).toEqual(['local', 'studio']);
  expect(projectList(rows, undefined).map((p) => 'peer' in p)).toEqual([false, false]);
});
