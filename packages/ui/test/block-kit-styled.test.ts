/// <reference types="vite/client" />
// Render audit r1: the chart (rb-chart*) and tree (cb-tree*) blocks shipped with data-kit hooks that no sheet
// selected, so they drew as bare text. Every literal data-kit a block writes (and every kit handed to Folded)
// must be selected by some kit sheet, or be named below with the reason it is left to its parent.
import { expect, test } from 'vitest';

const blocks = import.meta.glob<string>('../src/blocks/*.tsx', { query: '?raw', import: 'default', eager: true });
const sheets = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });

/** Keys deliberately without a rule of their own. */
const UNSTYLED: Readonly<Record<string, string>> = {
  'cb-callout': 'a hook around the kit Callout, which draws the box',
  'cb-stats': 'a hook around the kit Stats, which draws the tiles',
  'cb-md': 'a hook around the kit Markdown, which styles the prose',
  'cb-more': 'the Show all row holds a kit Button; the row itself is plain flow',
  'cb-commit-files': "takes cb-commit-sub's mono line; data-text='control' keeps it to one line",
  'md-em': "the browser's own italic",
  'cb-title-text': "a span in cb-title's flex row, which sets its font",
  'rb-title-text': "a span in rb-title's flex row, which sets its font",
  'rb-cant-text': "a span in rb-cant's flex row, which sets its font",
};

function blockKits(): Map<string, string> {
  const out = new Map<string, string>();
  for (const [path, src] of Object.entries(blocks)) {
    const file = path.replace('../src/blocks/', '');
    for (const m of src.matchAll(/data-kit="([^"{}]+)"/g)) out.set(m[1] ?? '', file);
    for (const m of src.matchAll(/<Folded\b[^>]*?\skit="([^"{}]+)"/g)) out.set(m[1] ?? '', file);
  }
  return out;
}

function selectedKits(): Set<string> {
  const css = Object.values(sheets).join('\n').replace(/\/\*[^]*?\*\//g, '');
  return new Set([...css.matchAll(/\[data-kit=(?:'([^']+)'|"([^"]+)")\]/g)].map((m) => m[1] ?? m[2] ?? ''));
}

test('every_data_kit_a_block_writes_has_a_css_selector_render_r1', () => {
  const kits = blockKits();
  expect(kits.has('rb-chart-bar')).toBe(true);
  expect(kits.has('cb-tree')).toBe(true);
  const styled = selectedKits();
  const missing = [...kits].filter(([k]) => !styled.has(k) && !(k in UNSTYLED)).map(([k, file]) => `${file}: ${k}`);
  expect(missing).toEqual([]);
});
