// A `diff` block that names only its file (the protocol's `{"type":"diff","file":…}`) shows the edits wisp captured
// for that file this turn, as v1's findChange did; v2 showed "No captured edits" for every such block.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { Answer, type CapturedChange } from '../src/index.ts';
import { capturedDiff } from '../src/blocks/content-diff.ts';

const CHANGES: CapturedChange[] = [
  { file: '/repo/packages/shell/src/frame/HomeView.tsx', op: 'edit', edits: [{ before: 'a\nb\n', after: 'a\nc\n' }, { before: 'x', after: 'x\ny' }] },
  { file: '/repo/new.ts', op: 'create', edits: [{ before: '', after: 'one\ntwo' }] },
];

test('captured_diff_matches_by_path_suffix_and_joins_every_edit', () => {
  const d = capturedDiff(CHANGES, 'packages/shell/src/frame/HomeView.tsx');
  expect(d?.additions).toBe(2);
  expect(d?.deletions).toBe(1);
  expect(d?.hunks.length).toBe(2);
  expect(capturedDiff(CHANGES, 'new.ts')?.status).toBe('added');
  expect(capturedDiff(CHANGES, 'other.ts')).toBeUndefined();
  expect(capturedDiff([{ file: 'a.ts', edits: [] }], 'a.ts')).toBeUndefined();
});

test('diff_block_naming_a_file_shows_its_captured_edits_not_missing', () => {
  const root = mount(() => (
    <Answer
      blocks={[
        { type: 'diff', file: 'packages/shell/src/frame/HomeView.tsx' },
        { type: 'diff', file: 'src/untouched.ts' },
        { type: 'files', items: [{ path: 'new.ts', change: 'added' }] },
      ]}
      ctx={{ changes: () => CHANGES }}
    />
  ));
  const [found, missing] = [...root.querySelectorAll('[data-kit="cb-diff"]')];
  expect(found?.hasAttribute('data-missing')).toBe(false);
  expect(found?.querySelector('[data-kit="diff"]')).not.toBeNull();
  expect(missing?.hasAttribute('data-missing')).toBe(true);
  expect(root.querySelector('[data-part="block.files.item"]')?.getAttribute('data-has-diff')).not.toBeNull();
});
