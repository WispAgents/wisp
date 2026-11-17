import { expect, test } from 'vitest';
import { ownerTitle } from '../src/words.ts';

test('all_settings_heads_each_group_with_its_plugins_name_in_words_qa_r1_28', () => {
  // the groups were headed by plugin ids, lowercase ("automations", "budget"), where every other pane is Title case
  expect(['wisp.automations', 'wisp.budget', 'wisp.shell-list', 'wisp.runtime', 'acme.my_plugin'].map(ownerTitle)).toEqual(['Automations', 'Budget', 'Shell list', 'Runtime', 'Acme.my plugin']);
});
