// A recipe from the launcher (v1 runRecipe, F-comp-37): the dialog asks each `{field}` of its prompt and steps once,
// in order, and takes the recipe's name and the launcher's project from its params.
import { expect, test } from 'vitest';
import { fieldsOf, parseRun, runProject } from '../src/recipe.ts';

test('the run dialog asks each field of the prompt and steps once_v1_run_recipe', () => {
  expect(fieldsOf({ prompt: 'Prepare release {version} for {app}', steps: ['tag {version}', 'tell {who}'] })).toEqual(['version', 'app', 'who']);
  expect(fieldsOf({ prompt: 'no fields' })).toEqual([]);
  expect(parseRun({ name: 'Prepare release', project: '/code/app' })).toEqual({ name: 'Prepare release', project: '/code/app' });
  expect(parseRun(null)).toEqual({ name: '', project: '' });
});

// Audit B #11: Run from Settings on a recipe tied to no project said "No folder" and ran in an empty scratch folder.
test('a_recipe_for_any_project_runs_in_the_launchers_project_unless_another_is_picked', () => {
  const projects = [
    { id: 'p1', path: '/code/proj' },
    { id: 'p2', path: '/code/api' },
  ];
  expect(runProject('', 'all', projects)).toBe('/code/proj');
  expect(runProject('', undefined, projects)).toBe('/code/proj');
  expect(runProject('', 'p2,p1', projects)).toBe('/code/api');
  expect(runProject('', 'gone', projects)).toBe('/code/proj');
  expect(runProject('/code/given', 'p2', projects)).toBe('/code/given');
  expect(runProject('', 'all', [])).toBe('');
});
