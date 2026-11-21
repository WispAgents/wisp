import { expect, test } from 'vitest';
import def, { HARNESS } from '../src/index.ts';

test('Claude Code is described to the screens by its plugin, with its mark', () => {
  expect(def.harnesses).toEqual([HARNESS]);
  expect(HARNESS.id).toBe('claude');
  // its own image file, not a data: URL in the first page's JS
  expect(HARNESS.logo.endsWith('/logo.svg')).toBe(true);
});
