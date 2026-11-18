// QA phone r3 #6: "Merge PR #806…" in the task's ⋯ menu wore a ✓, which reads as done or picked. It wears the merge
// mark; Review PR, which had it, looks at the PR (an eye), so the two never share one.
import { expect, test } from 'vitest';
import later from '../src/later.ts';

test('merge_pr_wears_the_merge_mark_and_review_pr_an_eye_qa_phone_r3_6', () => {
  const icon = (id: string): string | undefined => later.extensions?.find((e) => e.id === id)?.bar?.icon;
  expect(icon('wisp.ship.rowMerge')).toBe('merge');
  expect(icon('wisp.ship.rowReview')).toBe('eye');
  expect(icon('wisp.ship.rowMerge')).not.toBe('check');
});
