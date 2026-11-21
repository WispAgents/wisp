// Audit B #7: the head's ⋯ Save for later saved quietly, while the row menu and the hover button say "Saved for later"
// with Undo (shell-list actions.ts toggleForLater). The head now says the same, and Undo of Unsave puts back the
// time it was saved at; the command carries that time on the wire.
import { expect, test } from 'vitest';
import { laterToggle } from '../src/frame/later-toggle.ts';
import { forLater } from '../src/frame/commands.ts';
import { wireOf, optimisticOf, patchValue, peekReader } from './command.ts';

test('head_save_for_later_says_saved_for_later_with_undo_as_the_row_menu_does', () => {
  const save = laterToggle(undefined);
  expect(save.next).toEqual({ forLater: true });
  expect(save.toast).toBe('shell.toast.savedForLater');
  expect(save.undo).toEqual({ forLater: false });
  // Unsave: "Unsaved", and Undo puts it back at its old time
  const unsave = laterToggle({ at: 42, by: 'you' });
  expect(unsave.next).toEqual({ forLater: false });
  expect(unsave.toast).toBe('shell.toast.unsaved');
  expect(unsave.undo).toEqual({ forLater: true, at: 42 });
  // the time rides the wire and the optimistic row
  expect(wireOf(forLater)({ id: 'local:s1', forLater: true, at: 42 })).toEqual({ id: 's1', forLater: true, at: 42 });
  expect(patchValue(optimisticOf(forLater)(peekReader(undefined), { id: 'local:s1', forLater: true, at: 42 }))).toEqual({ at: 42, by: 'you' });
});
