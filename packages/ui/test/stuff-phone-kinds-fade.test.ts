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

// QA phone r4 #3: the tile's ⋯ sat on its own preview (over the app window's title, on the doc page's corner), and the
// app preview's description was cut mid-line. The ⋯ goes beside the name under the preview (as the Files apps put it),
// the name keeps room for it, and the description is one line with an ellipsis.
test('a_tiles_menu_sits_by_its_name_and_the_app_preview_says_one_line', () => {
  const menu = CSS.match(/\[data-kit='stuff-tile-menu'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(menu).toMatch(/bottom:\s*0/);
  expect(menu).not.toMatch(/top:\s*var/);
  const name = CSS.match(/\[data-kit='stuff-tile-name'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(name).toMatch(/padding:[^;]*var\(--size-touch/);
  const desc = CSS.match(/\[data-kit='stuff-tile-thumb'\] \[data-kit='st-win-body'\] > span\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(desc).toMatch(/-webkit-line-clamp:\s*1/);
});
