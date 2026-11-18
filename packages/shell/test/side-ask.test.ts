// orch #5, tasks #6: "/side …" in a task's reply box is a side discussion of its own (runtime.create under the task),
// never a follow-up into the task's conversation. What the box reads as one, and what the create sends.
import { expect, test } from 'vitest';
import { entityKey } from '@wisp/client';
import { sideAsk, sideStart } from '../src/frame/commands.ts';

test('a_slash_side_is_read_as_the_side_discussions_opening_message', () => {
  expect(sideAsk('/side why sqlite?')).toBe('why sqlite?');
  expect(sideAsk('  /SIDE  is this right?  ')).toBe('is this right?');
  expect(sideAsk('/side')).toBe('');
  expect(sideAsk('/sidebar fix')).toBeUndefined();
  expect(sideAsk('a /side note')).toBeUndefined();
});

test('the_side_discussion_is_created_under_the_task_never_sent_into_it', () => {
  const main = entityKey('peer1' as never, 'main1');
  const id = entityKey('peer1' as never, 'side1');
  const wire = sideStart.wire?.({ id, main, text: 'why sqlite?', at: 1 });
  expect(sideStart.id).toBe('runtime.create');
  expect(wire).toEqual({ id: 'side1', prompt: 'why sqlite?', parentId: 'main1' });
});
