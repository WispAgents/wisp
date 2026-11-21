// qa/panel-rules.ts: v1's Preview tab (qa/views.mjs task.preview) in v2 (docs/ux/2026-11-18-desktop.md #5). The side
// panel's strip holds the tabs with something in them: Preview is a tab while the task's app runs, otherwise it waits
// behind the strip's +. parity opens it either way, the way a user does.

import assert from 'node:assert/strict';
import test from 'node:test';
import { PREVIEW_TAB, TABS_MORE, PREVIEW_ITEM, openPreview } from '../../qa/panel-rules.ts';

const page = (has: Record<string, number>) => {
  const clicked: string[] = [];
  return {
    clicked,
    locator: (sel: string) => ({
      count: async () => has[sel] ?? 0,
      first: () => ({ click: async () => void clicked.push(sel) }),
    }),
    waitForTimeout: async () => undefined,
  };
};

test('preview_in_the_strip_is_one_press', async () => {
  const p = page({ [PREVIEW_TAB]: 1 });
  await openPreview(p);
  assert.deepEqual(p.clicked, [PREVIEW_TAB]);
});

test('an_empty_preview_waits_behind_plus_two_presses', async () => {
  const p = page({ [TABS_MORE]: 1, [PREVIEW_ITEM]: 1 });
  await openPreview(p);
  assert.deepEqual(p.clicked, [TABS_MORE, PREVIEW_ITEM]);
});
