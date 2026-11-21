// Save as recipe… (src/recipe.ts, v1 automations.js recipeParams / saveAsRecipe).
import { expect, test } from 'vitest';
import { draftOf, parseDraft, recipeBody, recipeName, recipeParams } from '../src/recipe.ts';

test('a prompt’s {fields} once each in order; the draft from a task and from view params_v1_save_as_recipe', () => {
  expect(recipeParams('Release {version}: bump {version}, tag {tag}; not { x } or {1x}')).toEqual(['version', 'tag']);
  expect(recipeParams('as written')).toEqual([]);
  expect(draftOf({ title: 'Ship the release', mode: 'edits', agent: 'codex' }, 'Prepare {version}')).toEqual({
    name: 'Ship-the-release',
    prompt: 'Prepare {version}',
    project: '',
    mode: 'edits',
    agent: 'codex',
    model: '',
    effort: '',
  });
  expect(parseDraft({ session: 'local:s1', name: 'X', prompt: '', mode: 7 })).toEqual({ name: 'X', prompt: '', project: '', mode: '', agent: '', model: '', effort: '', session: 'local:s1' });
});

test('Save as recipe… fills in a name the server takes, and Save keeps what the editor does not show_audit_A_tasks_7', () => {
  expect(recipeName('Victim E renamed')).toBe('Victim-E-renamed');
  expect(recipeName('  Add fix!  ')).toBe('Add-fix');
  expect(recipeName('release.v2_beta')).toBe('release.v2_beta');
  expect(recipeName('café crème')).toBe('café-crème');
  expect(recipeName('…')).toBe('');
  expect(draftOf({ title: 'Ship the release' }, 'x').name).toBe('Ship-the-release');
  const base = { name: 'old', prompt: 'p', steps: ['then'], schedule: '0 9 * * *', project: '/a', mode: 'plan' };
  expect(recipeBody({ name: 'New name', prompt: 'q', project: '', mode: '' }, base)).toEqual({ name: 'New-name', prompt: 'q', steps: ['then'], schedule: '0 9 * * *' });
  expect(recipeBody({ name: 'n', prompt: 'q', project: '/b', mode: 'auto', agent: 'codex' })).toEqual({ name: 'n', prompt: 'q', project: '/b', mode: 'auto', agent: 'codex' });
});
