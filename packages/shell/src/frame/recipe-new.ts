// Recipes' + New on New task (v1 launcher.js #lrNew): the recipe editor (wisp.automations' `automations.recipe` dialog)
// on a blank draft for this project in the launcher's mode. It opened Settings on a pane called "recipes", which no
// plugin has, so Settings showed General (QA r1 #5).
import type { Intent } from '@wisp/client';

export function newRecipeIntent(projectPath: string | undefined, mode: string): Intent {
  return {
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: 'automations.recipe' as never, params: { name: '', prompt: '', project: projectPath ?? '', mode, agent: '', model: '', effort: '' } },
  };
}
