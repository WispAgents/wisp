import { expect, test } from 'vitest';
import { titleParts } from '../src/words.ts';

test('all_settings_shows_a_declared_title_as_a_plain_label_and_its_help', () => {
  // manifests write "Label: what it does"; the row shows the label, and the help under it
  expect(titleParts('Check compactions: after a compaction, wisp asks a copy of the conversation what it still remembers')).toEqual({
    label: 'Check compactions',
    help: 'After a compaction, wisp asks a copy of the conversation what it still remembers',
  });
  expect(titleParts('Drain watch: tell the phone when a plan moves much faster than usual')).toEqual({ label: 'Drain watch', help: 'Tell the phone when a plan moves much faster than usual' });
  // no "label: " up front (a sentence, a time, a long first clause): all label
  expect(titleParts('Theme')).toEqual({ label: 'Theme', help: '' });
  expect(titleParts('Runs at 09:30 each day')).toEqual({ label: 'Runs at 09:30 each day', help: '' });
  expect(titleParts('A long sentence. Then this: and that')).toEqual({ label: 'A long sentence. Then this: and that', help: '' });
  expect(titleParts('x'.repeat(60) + ': help')).toEqual({ label: `${'x'.repeat(60)}: help`, help: '' });
});
