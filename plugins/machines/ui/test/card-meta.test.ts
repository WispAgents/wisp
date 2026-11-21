// QA desktop r3 #8: a machine's "wisp v1 (0.58.0) · Update to v2" line started at the dialog's edge, left of the
// machine's icon, and read as a row of its own between two machines: it sat beside the row, not in it. It goes in the
// row's `below` (the kit lays that from the words' edge, as v1 .cg-agents did).
import { expect, test } from 'vitest';
import CARD from '../src/panes/Card.tsx?raw';

test('a_machines_version_line_sits_in_its_row_under_its_name', () => {
  const row = CARD.indexOf('part="machines.row"');
  const rowEnd = CARD.indexOf('\n      />', row);
  const meta = CARD.indexOf('part="machines.meta"');
  expect(CARD.slice(row, rowEnd)).toContain('below={');
  expect(meta > row && meta < rowEnd, 'the version line is inside the row').toBe(true);
});
