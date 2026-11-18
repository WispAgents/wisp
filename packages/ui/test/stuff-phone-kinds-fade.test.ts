// QA r2 phone #13: My Stuff's kind chips ("All / Apps / Pictures / Docs / Code / Fil…") were cut at the right edge
// with nothing saying the row scrolls. The row fades out at its end, over its own end padding when the chips fit.
import { expect, test } from 'vitest';
import CSS from '../src/css/phone-pages.css?raw';

test('the_stuff_kind_row_fades_at_its_end_so_it_reads_as_scrolling', () => {
  const rule = CSS.match(/\[data-kit='stuff-phone-kinds'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/overflow-x:\s*auto/);
  expect(rule).toMatch(/mask-image:\s*linear-gradient\(/);
});

// QA phone r3 #5: a tile's preview (the card's own: an app's window, a doc's page, code's lines) is placed against its
// thumbnail as on desktop, clipped by it, never spilling over the tile's name
test('a_phone_tiles_thumbnail_holds_its_preview', () => {
  const rule = CSS.match(/\[data-kit='stuff-tile-thumb'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/position:\s*relative/);
  expect(rule).toMatch(/overflow:\s*hidden/);
});
