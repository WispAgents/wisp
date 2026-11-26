// R4-5: a step's open state is the turn's, keyed by the step (frame/step-open.ts): it follows the step when rows move,
// and it outlives the switch from the running turn's transcript to the finished one's.
import { afterEach, expect, test } from 'vitest';
import { Show, createRoot, createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { stepIdOf, stepOpen } from '../src/frame/step-open.ts';
import { TurnTranscript, type TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

const mount = (view: () => ReturnType<typeof TurnTranscript>): HTMLElement => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(view, host);
  return host;
};
const step = (host: HTMLElement, anchor: string): HTMLElement | null => host.querySelector(`[data-kit="timeline-step"][data-anchor="${anchor}"]`);
const isOpen = (host: HTMLElement, anchor: string): boolean => step(host, anchor)?.getAttribute('data-open') !== null && step(host, anchor)?.querySelector('[data-kit="timeline-detail"]') !== null;
const click = (host: HTMLElement, anchor: string): void => step(host, anchor)?.querySelector<HTMLButtonElement>(':scope > button[data-kit="timeline-row"]')?.click();

const run = (seq: number, label: string): TranscriptStepRow => ({ kind: 'run', label, out: `out of ${label}`, seq });

test('an_opened_step_stays_open_when_a_row_is_put_before_it_and_the_step_now_in_its_place_stays_closed', () => {
  const [steps, setSteps] = createSignal<TranscriptStepRow[]>([run(10, 'a'), run(20, 'b')]);
  const host = mount(() => <TurnTranscript steps={steps()} changes={[]} here={(s) => s} status="running" t={(k) => k} />);
  click(host, 'step:20');
  expect(isOpen(host, 'step:20')).toBe(true);
  expect(isOpen(host, 'step:10')).toBe(false);
  // Show thinking: a thought comes in first, every row moves down one place
  setSteps([{ kind: 'thinking', label: 'Thought', out: 'Thought about it', seq: 5 }, run(10, 'a'), run(20, 'b')]);
  expect(isOpen(host, 'step:20')).toBe(true);
  // the step now where 20 was (index 1) is 10: still closed
  expect(isOpen(host, 'step:10')).toBe(false);
  // a row before it hidden (hideSuccessful): 20 moves up, still open; 10 gone
  setSteps([run(20, 'b'), run(30, 'c')]);
  expect(isOpen(host, 'step:20')).toBe(true);
  expect(isOpen(host, 'step:30')).toBe(false);
});

test('a_step_and_a_cluster_opened_while_the_turn_runs_are_still_open_once_it_ends', () => {
  // the live transcript and the answer's are two components: one store, made with the turn, is what they share
  const rows: TranscriptStepRow[] = [
    { kind: 'text', label: 'Looking around.', seq: 1 },
    run(2, 'ls'),
    { kind: 'read', label: 'Reading src/a.ts', file: 'src/a.ts' },
    run(4, 'grep'),
    { kind: 'text', label: 'Found it.', seq: 5 },
  ];
  const open = createRoot(() => stepOpen());
  const [status, setStatus] = createSignal('running');
  const live = (): boolean => status() === 'running';
  const host = mount(() => (
    <Show when={live()} fallback={<TurnTranscript steps={rows} changes={[]} open={open} here={(s) => s} status={status()} chat quiet t={(k) => k} />}>
      <TurnTranscript steps={rows} changes={[]} open={open} here={(s) => s} status={status()} chat quiet t={(k) => k} />
    </Show>
  ));
  const group = (): HTMLElement | null => host.querySelector('[data-part="turn.transcript.group"]');
  group()?.querySelector<HTMLButtonElement>(':scope > button')?.click();
  expect(group()?.querySelector('[data-kit="timeline-sublist"]')).not.toBeNull();
  click(host, 'step:4');
  expect(isOpen(host, 'step:4')).toBe(true);
  const before = group();
  setStatus('done');
  expect(group()).not.toBe(before); // the live transcript went, the answer's came
  expect(group()?.querySelector('[data-kit="timeline-sublist"]')).not.toBeNull();
  expect(isOpen(host, 'step:4')).toBe(true);
  expect(isOpen(host, 'step:2')).toBe(false);
});

test('a_step_with_no_event_falls_back_to_its_place_and_still_opens_and_closes', () => {
  const host = mount(() => <TurnTranscript steps={[{ kind: 'error', label: 'It broke', bad: true }]} changes={[]} here={(s) => s} status="done" t={(k) => k} />);
  expect(stepIdOf({}, 0)).toBe('index:0');
  click(host, 'index:0');
  expect(isOpen(host, 'index:0')).toBe(true);
  click(host, 'index:0');
  expect(isOpen(host, 'index:0')).toBe(false);
});

test('reveal_opens_the_step_and_the_folded_cluster_that_holds_it', () => {
  const open = createRoot(() => stepOpen());
  const rows: TranscriptStepRow[] = [run(1, 'a'), run(2, 'b'), run(3, 'c'), { kind: 'edit', label: 'Editing x', file: 'x' }];
  const host = mount(() => <TurnTranscript steps={rows} changes={[]} open={open} here={(s) => s} status="done" t={(k) => k} />);
  expect(step(host, 'step:2')).toBeNull(); // folded away
  open.reveal('step:2');
  expect(isOpen(host, 'step:2')).toBe(true);
  expect(open.wanted()).toBeUndefined();
});
