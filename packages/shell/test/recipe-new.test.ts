import { expect, test } from 'vitest';
import { newRecipeIntent } from '../src/frame/recipe-new.ts';

test('recipes_new_opens_the_recipe_editor_on_a_blank_draft_for_this_project_v1_lr_new', () => {
  expect(newRecipeIntent('/code/wisp', 'auto')).toEqual({
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: 'automations.recipe', params: { name: '', prompt: '', project: '/code/wisp', mode: 'auto', agent: '', model: '', effort: '' } },
  });
  // no folder: any project
  expect((newRecipeIntent(undefined, 'plan') as unknown as { overlay: { params: { project: string } } }).overlay.params.project).toBe('');
});
