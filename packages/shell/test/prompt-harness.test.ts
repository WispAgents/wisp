// Audit B #6: picking another harness in the reply box changed the mark on earlier prompt bubbles: the bubble read the
// reply box's pick. Each bubble shows the harness its own turn ran on (runtime's Turn.agent); a turn not yet started
// (the follow-up just sent) shows the one it was sent to, else the task's.
import { expect, test } from 'vitest';
import { promptHarness } from '../src/frame/prompt-harness.ts';

test('a_prompt_bubble_shows_its_own_turns_harness_not_the_reply_boxs_pick', () => {
  // turn 1 ran on Claude Code; Codex is picked in the reply box: the bubble stays Claude Code
  expect(promptHarness({ agent: 'claude' }, 'codex', 'claude')).toBe('claude');
  expect(promptHarness({ agent: 'claude' }, undefined, 'codex')).toBe('claude');
  // the follow-up just sent (no agent on its row yet): the harness it was sent with
  expect(promptHarness({}, 'codex', 'claude')).toBe('codex');
  expect(promptHarness({ agent: '' }, undefined, 'claude')).toBe('claude');
});
