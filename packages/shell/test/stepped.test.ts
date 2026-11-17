// j/k open the next task but keep the keys on the list (src/frame/stepped.ts, v1 keys.js; TRY-26).
import { expect, test } from 'vitest';
import { markStepped, openedByStep } from '../src/frame/stepped.ts';

test('a task opened right after j or k was opened by stepping; one opened later, by a click, was not', () => {
  markStepped(10_000);
  expect(openedByStep(10_050)).toBe(true);
  expect(openedByStep(10_499)).toBe(true);
  expect(openedByStep(10_600)).toBe(false);
});
