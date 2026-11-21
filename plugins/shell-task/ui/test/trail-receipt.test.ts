// The receipt rides on the trail's line after the token count (ADR-0095, v1 receiptHtml in .trail-bar): on a phone the
// turn's foot was three lines, the receipt alone on the last (QA r1 #19).
import { expect, test } from 'vitest';
import { trailWords } from '../src/cards/trail.ts';
import def from '../src/index.ts';

test('the_receipt_follows_the_token_count_on_the_trails_line_qa_r1_19', () => {
  expect(trailWords(['90% cache', '3% of weekly'])).toBe(' · 90% cache · 3% of weekly');
  expect(trailWords([])).toBe('');
  const ext = (def.extensions ?? []).map((e) => [e.id, e.attachTo.id]);
  expect(ext).toContainEqual(['shell-task.receipt', 'shell.turn:trail']);
});
