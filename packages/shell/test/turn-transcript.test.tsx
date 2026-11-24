// A transcript step whose label the harness had to cut (frame/TurnTranscript.tsx, v1 #779/#780).
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { TurnTranscript, type TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = '';
});

test('an_opened_step_shows_its_whole_title_not_the_label_cut_for_the_row', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const whole = 'Searching for “a-very-long-pattern-a-very-long-pattern-a-very-long-pattern” in src';
  dispose = render(
    () => (
      <TurnTranscript
        steps={[
          { kind: 'run', label: 'Searching for “a-very-long-pattern-a-very-long…” in src', title: whole },
          { kind: 'read', label: 'Reading src/a.ts' },
        ]}
        changes={[]}
        here={(s) => s}
        status="done"
        t={(k) => k}
      />
    ),
    host,
  );
  const labels = (): string[] => [...host.querySelectorAll('[data-kit="timeline-label"]')].map((e) => e.textContent ?? '');
  expect(labels()[0]).toContain('…');
  // a step with no more to it is a plain row; the cut one opens
  expect(host.querySelectorAll('button[data-kit="timeline-row"]').length).toBe(1);
  host.querySelector<HTMLButtonElement>('button[data-kit="timeline-row"]')?.click();
  expect(labels()[0]).toBe(whole);
});

test('a_finished_turn_collapses_long_exploration_runs_and_keeps_edits_visible', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const tMap: Record<string, string> = {
    'shell.task.group.reads.other': 'Read {n} files',
    'shell.task.group.searches.one': '1 search',
  };
  const t = (k: string, v?: Record<string, string | number>): string => {
    let s = tMap[k] ?? k;
    for (const [a, b] of Object.entries(v ?? {})) s = s.replace(`{${a}}`, String(b));
    return s;
  };
  dispose = render(
    () => (
      <TurnTranscript
        chat
        steps={[
          { kind: 'read', label: 'Reading src/a.ts', file: 'src/a.ts' },
          { kind: 'read', label: 'Reading src/b.ts', file: 'src/b.ts' },
          { kind: 'search', label: 'Searching for “foo”' },
          { kind: 'read', label: 'Reading src/c.ts', file: 'src/c.ts' },
          { kind: 'edit', label: 'Editing src/a.ts', file: 'src/a.ts' },
        ]}
        changes={[]}
        here={(s) => s}
        status="done"
        t={t}
      />
    ),
    host,
  );
  // Collapsed by default: 1 group row + 1 visible edit row (2 rows instead of 5)
  const topSteps = host.querySelectorAll('[data-kit="timeline-list"] > [data-kit="timeline-step"]');
  expect(topSteps.length).toBe(2);
  const groupBtn = host.querySelector<HTMLButtonElement>('[data-part="turn.transcript.group"] > button[data-kit="timeline-row"]');
  expect(groupBtn?.textContent).toContain('Read 3 files · 1 search');
  expect(host.querySelector('[data-part="turn.transcript.group.body"]')).toBeNull();

  // Clicking expands compact file chips + the search row in place
  groupBtn?.click();
  expect(host.querySelectorAll('[data-kit="timeline-file"]').length).toBe(3);
  expect(host.querySelector('[data-kit="timeline-sublist"]')?.textContent).toContain('Searching for “foo”');
});

test('streaming_more_steps_keeps_earlier_mid_turn_markdown_dom_nodes_intact', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const [steps, setSteps] = createSignal<TranscriptStepRow[]>([
    { kind: 'text', label: 'Checking the diff first.', seq: 1 },
    { kind: 'read', label: 'Reading src/a.ts', seq: 2 },
  ]);
  dispose = render(() => <TurnTranscript chat steps={steps()} changes={[]} here={(s) => s} status="running" t={(k) => k} />, host);
  const msgBefore = host.querySelector('[data-kit="markdown"] p');
  expect(msgBefore?.textContent).toBe('Checking the diff first.');
  setSteps([
    { kind: 'text', label: 'Checking the diff first.', seq: 1 },
    { kind: 'read', label: 'Reading src/a.ts', seq: 2 },
    { kind: 'run', label: 'Running tests', seq: 3 },
  ]);
  expect(host.querySelector('[data-kit="markdown"] p')).toBe(msgBefore);
});
