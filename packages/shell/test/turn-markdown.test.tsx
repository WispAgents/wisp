// R4-6: Copy turn as Markdown (frame/turn-markdown.ts) and a step's Copy (TurnTranscript → Timeline's detail): what
// they copy is what the step shows, a .env file's masked text until Reveal.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import type { EnvReveal } from '@wisp/ui';
import { codeSpan, fenceFor, turnMarkdown } from '../src/frame/turn-markdown.ts';
import { TurnTranscript, type TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';
import { stepOpen } from '../src/frame/step-open.ts';
import { findInTurns, type FindEnv } from '../src/frame/task-find.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

const words = { prompt: 'Prompt', steps: 'Steps', answer: 'Answer' };
const envStep: TranscriptStepRow = { kind: 'read', label: 'Reading .env', file: '.env', out: 'API_KEY=sk-live-1', seq: 7 };
const steps: TranscriptStepRow[] = [
  { kind: 'run', label: 'Running tests', detail: 'npm test', out: 'ok 3 tests', seq: 2 },
  { kind: 'edit', label: 'Editing src/a.ts', file: 'src/a.ts' },
  envStep,
  { kind: 'run', label: 'Show the doc', detail: 'cat README.md', out: 'Use it so:\n```sh\nwisp go\n```\nand ````four````', seq: 9 },
];
const env = (shown: boolean): FindEnv => ({ on: () => true, shown: (id) => shown && id === 'step:7', raw: (id) => (shown && id === 'step:7' ? 'API_KEY=sk-live-1' : undefined) });

test('a_turn_as_markdown_has_its_prompt_each_step_with_its_output_fenced_and_the_answers_source', () => {
  const md = turnMarkdown({ prompt: 'Fix the tests', steps, answer: '**Done.** All green.', env: env(false), words });
  expect(md).toContain('## Prompt\n\nFix the tests');
  expect(md).toContain('- **Running tests**\n\n  `npm test`\n\n```\nok 3 tests\n```');
  expect(md).toContain('- **Editing src/a.ts**');
  // masked, not the value
  expect(md).toContain('API_KEY=••••');
  expect(md).not.toContain('sk-live-1');
  // a fence longer than any run of backticks in the output
  expect(md).toContain('`````\nUse it so:\n```sh\nwisp go\n```\nand ````four````\n`````');
  expect(md.trim().endsWith('## Answer\n\n**Done.** All green.')).toBe(true);
});

test('a_revealed_env_output_is_copied_as_revealed', () => {
  const md = turnMarkdown({ prompt: 'p', steps: [envStep], env: env(true), words });
  expect(md).toContain('API_KEY=sk-live-1');
});

test('the_fence_is_three_backticks_at_least_and_one_more_than_the_longest_run', () => {
  expect(fenceFor('plain')).toBe('```');
  expect(fenceFor('a ``` b')).toBe('````');
  expect(fenceFor('``````')).toBe('```````');
});

test('a_command_holding_backticks_is_a_code_span_its_backticks_cannot_close', () => {
  // Markdown takes no backslash escape inside a code span: a longer run of backticks around it is what keeps it whole
  expect(codeSpan('npm test')).toBe('`npm test`');
  // ends in a backtick: a space each side, so the closing run stays two long
  expect(codeSpan('echo `date`')).toBe('`` echo `date` ``');
  expect(codeSpan('a `` b')).toBe('```a `` b```');
  const md = turnMarkdown({ prompt: 'p', steps: [{ kind: 'run', label: 'Printing the date', detail: 'echo `date`', out: 'Tue', seq: 1 }], words });
  expect(md).toContain('\n\n  `` echo `date` ``');
  expect(md).not.toContain('\\`');
});

test('a_steps_copy_copies_the_masked_text_while_hidden_and_the_value_after_reveal', async () => {
  const writeText = vi.fn(() => Promise.resolve());
  vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
  let shown = false;
  const reveal: EnvReveal = { on: () => true, shown: () => shown, set: () => undefined, raw: () => (shown ? { output: 'API_KEY=sk-live-1' } : undefined) };
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <TurnTranscript steps={[envStep]} changes={[]} env={reveal} here={(s) => s} status="done" t={(k) => k} />, host);
  host.querySelector<HTMLButtonElement>('button[data-kit="timeline-row"]')?.click();
  const copy = host.querySelector<HTMLButtonElement>('[data-part="turn.transcript.step.copy"]') ?? host.querySelector<HTMLButtonElement>('[data-kit="timeline-copy"] button');
  expect(copy?.getAttribute('aria-label')).toBeTruthy();
  copy?.click();
  expect(writeText).toHaveBeenLastCalledWith('API_KEY=••••');
  // Reveal: what the step shows now is the server's copy, and Copy copies that (the detail is drawn again)
  shown = true;
  dispose();
  host.innerHTML = '';
  dispose = render(() => <TurnTranscript steps={[envStep]} changes={[]} env={reveal} here={(s) => s} status="done" t={(k) => k} />, host);
  host.querySelector<HTMLButtonElement>('button[data-kit="timeline-row"]')?.click();
  host.querySelector<HTMLButtonElement>('[data-kit="timeline-copy"] button')?.click();
  expect(writeText).toHaveBeenLastCalledWith('API_KEY=sk-live-1');
});

// #961 x #959: a clipped step's whole output, once Show all fetched it, is what its Copy, Copy turn and Find read
test('a_clipped_steps_whole_output_once_fetched_is_what_copy_copy_turn_and_find_read_and_its_finished_transcript_keeps', async () => {
  const writeText = vi.fn(() => Promise.resolve());
  vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
  const clipped: TranscriptStepRow = { kind: 'run', label: 'npm test', out: 'HEAD\n… 1 lines cut …\nTAIL', seq: 7, full: { lines: 3 } };
  const open = stepOpen();
  const host = document.createElement('div');
  document.body.append(host);
  const draw = (): void => {
    dispose = render(
      () => <TurnTranscript steps={[clipped]} changes={[]} here={(s) => s} status="done" open={open} whole={() => Promise.resolve('HEAD\nthe needle\nTAIL')} t={(k) => k} />,
      host,
    );
  };
  draw();
  host.querySelector<HTMLButtonElement>('button[data-kit="timeline-row"]')?.click();
  const fenv: FindEnv = { on: () => true, shown: () => false, raw: () => undefined, whole: open.whole };
  expect(findInTurns([{ turnId: 't', stub: false, steps: [clipped], env: fenv }], 'needle'), 'the clipped text is all Find sees').toEqual([]);
  host.querySelector<HTMLButtonElement>('[data-kit="timeline-copy"] button')?.click();
  expect(writeText).toHaveBeenLastCalledWith('HEAD\n… 1 lines cut …\nTAIL');
  host.querySelector<HTMLButtonElement>('[data-part="turn.transcript.showAll"]')?.click();
  await Promise.resolve();
  await Promise.resolve();
  host.querySelector<HTMLButtonElement>('[data-kit="timeline-copy"] button')?.click();
  expect(writeText).toHaveBeenLastCalledWith('HEAD\nthe needle\nTAIL');
  expect(findInTurns([{ turnId: 't', stub: false, steps: [clipped], env: fenv }], 'needle').map((h) => h.stepId)).toEqual(['step:7']);
  expect(turnMarkdown({ prompt: 'p', steps: [clipped], env: fenv, words })).toContain('the needle');
  // the turn's other transcript (live, then finished) draws it whole too, with no Show all again
  dispose?.();
  host.innerHTML = '';
  draw();
  // still open: the turn's store keeps it (step-open.ts)
  expect(host.textContent).toContain('the needle');
  expect(host.querySelector('[data-part="turn.transcript.showAll"]')).toBeNull();
});
