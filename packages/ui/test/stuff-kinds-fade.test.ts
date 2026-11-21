// QA desktop r3 #7: My Stuff's kind chips were cut mid-chip at 900 and 1024 ("Files" half shown, "Links" out of sight)
// in a row that scrolls with no scrollbar on macOS: nothing said there was more. The row fades out at its end.
import { expect, test } from 'vitest';
import CSS from '../src/css/stuff-frame.css?raw';

test('the_stuff_kinds_row_fades_at_its_end', () => {
  const rule = CSS.match(/\[data-kit='sf-kinds'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/overflow-x:\s*auto/);
  expect(rule).toMatch(/mask-image:\s*linear-gradient\(/);
});
