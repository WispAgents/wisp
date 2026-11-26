// R4-3: Find in a task. What the words match (frame/task-find.ts: the step rows, never the page, a .env file's value
// only once revealed), mod+f kept for wisp, and a hit in a folded cluster's closed step opened and marked.
import { afterEach, expect, test, vi } from 'vitest';
import { createRoot } from 'solid-js';
import { render } from 'solid-js/web';
import type { ShellContext } from '../src/contract.ts';
import { allowed } from '../src/keys.ts';
import { stepOpen } from '../src/frame/step-open.ts';
import TaskFind from '../src/frame/TaskFind.tsx';
import { findInTurns, landed, mergeHits, type FindEnv, type FindHit, type FindTurn } from '../src/frame/task-find.ts';
import { TurnTranscript, type TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

const envStep: TranscriptStepRow = { kind: 'read', label: 'Reading .env', file: '.env', out: 'API_KEY=sk-live-zebra', seq: 7 };
const rows: TranscriptStepRow[] = [
  { kind: 'run', label: 'Running the Zebra suite', seq: 1 },
  { kind: 'run', label: 'Searching for “x”', title: 'Searching for “zebra-long-title” in src', seq: 2 },
  { kind: 'run', label: 'ls', out: 'zebra.txt\nlion.txt', seq: 3 },
  envStep,
  { kind: 'text', label: 'No zebras were harmed.', seq: 9 },
];
const env = (revealed: boolean): FindEnv => ({ on: () => true, shown: (id) => revealed && id === 'step:7', raw: (id) => (revealed && id === 'step:7' ? envStep.out : undefined) });

test('words_match_a_steps_label_title_output_and_message_case_insensitive_one_hit_per_step', () => {
  const hits = findInTurns([{ turnId: 't1', stub: false, steps: rows, env: env(false) }], 'ZEBRA');
  expect(hits.map((h) => h.stepId)).toEqual(['step:1', 'step:2', 'step:3', 'step:9']);
  expect(findInTurns([{ turnId: 't1', stub: false, steps: rows }], '  ')).toEqual([]);
});

test('a_env_files_value_is_not_matched_while_masked_and_is_once_revealed', () => {
  expect(findInTurns([{ turnId: 't1', stub: false, steps: [envStep], env: env(false) }], 'sk-live')).toEqual([]);
  // with no Reveal state at all it stays masked (fail closed)
  expect(findInTurns([{ turnId: 't1', stub: false, steps: [envStep] }], 'sk-live')).toEqual([]);
  expect(findInTurns([{ turnId: 't1', stub: false, steps: [envStep], env: env(true) }], 'sk-live').map((h) => h.stepId)).toEqual(['step:7']);
});

test('a_stubs_hits_are_the_servers_in_turn_order_and_its_own_take_over_once_it_lands', () => {
  const turns: FindTurn[] = [
    { turnId: 'old', stub: true, steps: [] },
    { turnId: 'new', stub: false, steps: rows },
  ];
  const remote: FindHit[] = [{ turnId: 'old', seq: 3, snippet: 'zebra' }, { turnId: 'gone', seq: 1 }];
  const all = mergeHits(turns, findInTurns(turns, 'zebra'), remote);
  expect(all[0]).toBe(remote[0]);
  expect(all.map((h) => h.turnId)).toEqual(['old', 'new', 'new', 'new', 'new']);
  const now = mergeHits([{ turnId: 'old', stub: false, steps: rows }], findInTurns([{ turnId: 'old', stub: false, steps: rows }], 'zebra'), remote);
  expect(now[landed(now, remote[0]!)]?.stepId).toBe('step:3');
});

test('mod_f_is_wisps_own_a_user_plugin_cannot_take_it', () => {
  expect(allowed({ id: 'x', keys: 'mod+f', label: 'x', plugin: 'someone.plugin', run: () => {} })).toBe(false);
  expect(allowed({ id: 'x', keys: 'mod+f', label: 'x', plugin: 'shell', run: () => {} })).toBe(true);
});

test('going_to_a_hit_in_a_folded_clusters_closed_step_opens_both_and_marks_the_words', async () => {
  const sets: { name: string; ranges: Range[] }[] = [];
  vi.stubGlobal('Highlight', class {
    readonly ranges: Range[];
    constructor(...r: Range[]) {
      this.ranges = r;
    }
  });
  vi.stubGlobal('CSS', { ...globalThis.CSS, escape: (s: string) => s.replace(/"/g, '\\"'), highlights: { set: (name: string, h: { ranges: Range[] }) => sets.push({ name, ranges: h.ranges }), delete: () => true } });
  const open = createRoot(() => stepOpen());
  // three routine steps fold on a finished turn; the hit is in the second one's output
  const steps: TranscriptStepRow[] = [
    { kind: 'run', label: 'a', out: 'nothing', seq: 1 },
    { kind: 'run', label: 'b', out: 'here is the needle', seq: 2 },
    { kind: 'run', label: 'c', out: 'nothing', seq: 3 },
    { kind: 'edit', label: 'Editing x', file: 'x' },
  ];
  const shell = { t: (k: string) => k, client: { commands: { run: vi.fn() } } } as unknown as ShellContext;
  const go = (hit: FindHit): Promise<Element | undefined> => {
    open.reveal(hit.stepId ?? '');
    return Promise.resolve(document.querySelector(`[data-turn="t1"] [data-anchor="${hit.stepId}"]`) ?? undefined);
  };
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <>
        <TaskFind shell={shell} task="p:task" turns={() => [{ turnId: 't1', stub: false, steps }]} go={go} scope={() => host.querySelector('[data-turn]') ?? undefined} onClose={() => {}} />
        <article data-kit="turn" data-turn="t1">
          <TurnTranscript steps={steps} changes={[]} open={open} here={(s) => s} status="done" t={(k) => k} />
        </article>
      </>
    ),
    host,
  );
  expect(host.querySelector('[data-anchor="step:2"]')).toBeNull(); // folded away
  const input = host.querySelector<HTMLInputElement>('[data-part="task.find.input"]')!;
  input.value = 'NEEDLE';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  expect(host.querySelector('[data-kit="find-count"]')?.textContent).toBe('shell.find.count.one');
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await Promise.resolve();
  await Promise.resolve();
  const hit = host.querySelector('[data-anchor="step:2"]');
  expect(hit?.closest('[data-part="turn.transcript.group"]')?.getAttribute('data-open')).not.toBeNull();
  expect(hit?.querySelector('[data-kit="timeline-detail"] pre')?.textContent).toBe('here is the needle');
  const current = sets.findLast((s) => s.name === 'wisp-find-current');
  expect(current?.ranges.map((r) => r.toString())).toEqual(['needle']);
  expect(host.querySelector('[data-kit="find-count"]')?.textContent).toBe('shell.find.of');
});
