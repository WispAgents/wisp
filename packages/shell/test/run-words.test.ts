// QA r15 #3: a run in words, everywhere the same: the harness's name, the model as the picker names it (a Claude id in
// words), the thinking level in words; never the raw ids ("claude · opus · high").
import { expect, test } from 'vitest';
import { effortWord, modelWords, runLabel } from '../src/frame/run-words.ts';

test('a_run_reads_as_the_picker_says_it', () => {
  expect(runLabel({ name: 'Claude Code', model: 'Opus', effort: 'high' })).toBe('Claude Code · Opus · High');
  expect(runLabel({ name: 'Codex', model: 'GPT-6.1-Sol', effort: 'xhigh' })).toBe('Codex · GPT-6.1-Sol · X-High');
  expect(runLabel({ name: 'Claude Code' })).toBe('Claude Code');
  expect(effortWord('')).toBe('');
  expect(modelWords('claude-opus-5-5')).toBe('Opus 5.5');
  expect(modelWords('opus')).toBe('Opus');
});
