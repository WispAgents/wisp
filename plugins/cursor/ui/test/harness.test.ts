import { expect, test } from 'vitest';
import def, { HARNESS } from '../src/index.ts';

test('cursor_is_described_to_the_screens_by_its_plugin_with_its_mark', () => {
  expect(def.harnesses).toEqual([HARNESS]);
  expect(HARNESS.id).toBe('cursor');
  // its own image file, not a data: URL in the first page's JS; a mono mark, drawn in the app theme's ink (#mono)
  expect(HARNESS.logo.endsWith('/logo.svg#mono')).toBe(true);
});
