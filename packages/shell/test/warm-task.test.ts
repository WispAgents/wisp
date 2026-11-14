import { expect, test } from 'vitest';
import { TASK_PAGE_INPUTS, taskPagePreloads } from '../src/frame/warm-task.ts';

test('the_task_page_inputs_are_its_screen_and_every_task_and_turn_input', () => {
  expect(TASK_PAGE_INPUTS).toContain('shell.screen:task');
  expect(TASK_PAGE_INPUTS).toContain('shell.task:foot');
  expect(TASK_PAGE_INPUTS).toContain('shell.turn:foot');
  expect(TASK_PAGE_INPUTS.some((i) => i.startsWith('shell.list:') || i.startsWith('shell.topbar:'))).toBe(false);
});

test('each_lazy_component_preloads_once_and_plain_ones_are_skipped', async () => {
  const calls: string[] = [];
  const lazyA = Object.assign(() => null, { preload: () => (calls.push('a'), Promise.resolve()) });
  const lazyB = Object.assign(() => null, { preload: () => (calls.push('b'), Promise.reject(new Error('offline'))) });
  const plain = () => null;
  const runs = taskPagePreloads([lazyA, plain, lazyB, lazyA, undefined, null]);
  expect(runs.length).toBe(2);
  await Promise.all(runs.map((r) => r())); // a failed chunk never throws out of the warm-up
  expect(calls).toEqual(['a', 'b']);
});
