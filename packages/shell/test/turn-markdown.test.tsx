// R4-6: Copy turn as Markdown (frame/turn-markdown.ts) and a step's Copy (TurnTranscript → Timeline's detail): what
// they copy is what the step shows, a .env file's masked text until Reveal.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import type { EnvReveal } from '@wisp/ui';
import { fenceFor, turnMarkdown } from '../src/frame/turn-markdown.ts';
import { TurnTranscript, type TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';
import type { FindEnv } from '../src/frame/task-find.ts';

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
