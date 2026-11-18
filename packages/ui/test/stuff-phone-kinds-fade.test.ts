// QA r2 phone #13: My Stuff's kind chips ("All / Apps / Pictures / Docs / Code / Fil…") were cut at the right edge
// with nothing saying the row scrolls. The row fades out at its end, over its own end padding when the chips fit.
import { expect, test } from 'vitest';
import CSS from '../src/css/phone-pages.css?raw';

test('the_stuff_kind_row_fades_at_its_end_so_it_reads_as_scrolling', () => {
  const rule = CSS.match(/\[data-kit='stuff-phone-kinds'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/overflow-x:\s*auto/);
  expect(rule).toMatch(/mask-image:\s*linear-gradient\(/);
});
