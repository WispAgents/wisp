// Pure tests for signal-only folding of finished turn transcripts (frame/transcript-group.ts).
import { expect, test } from 'vitest';
import { clusterLabel, groupFinishedSteps, MIN_CLUSTER } from '../src/frame/transcript-group.ts';
import type { TranscriptStepRow } from '../src/frame/TurnTranscript.tsx';

const T_STRINGS: Record<string, string> = {
  'shell.task.group.reads.one': 'Read 1 file',
  'shell.task.group.reads.other': 'Read {n} files',
  'shell.task.group.edits.one': 'Edited 1 file',
  'shell.task.group.edits.other': 'Edited {n} files',
  'shell.task.group.searches.one': '1 search',
  'shell.task.group.searches.other': '{n} searches',
  'shell.task.group.commands.one': '1 command',
  'shell.task.group.commands.other': '{n} commands',
  'shell.task.group.thoughts.one': '1 thought',
  'shell.task.group.thoughts.other': '{n} thoughts',
  'shell.task.group.actions.one': '1 action',
  'shell.task.group.actions.other': '{n} actions',
  'shell.task.steps.one': '1 step',
  'shell.task.steps.other': '{n} steps',
};

const t = (key: string, vars?: Record<string, string | number>): string => {
  let s = T_STRINGS[key] ?? key;
  for (const [k, v] of Object.entries(vars ?? {})) s = s.replace(`{${k}}`, String(v));
  return s;
};

test('running turns keep every step flat without collapsing', () => {
  const steps: TranscriptStepRow[] = [
    { kind: 'read', label: 'Reading a.ts', file: 'a.ts' },
    { kind: 'read', label: 'Reading b.ts', file: 'b.ts' },
    { kind: 'read', label: 'Reading c.ts', file: 'c.ts' },
    { kind: 'read', label: 'Reading d.ts', file: 'd.ts' },
  ];
  const live = groupFinishedSteps(steps, true);
  expect(live.every((e) => e.kind === 'step')).toBe(true);
  expect(live).toHaveLength(4);
});

test('finished turns collapse 3+ routine steps while surfacing edits, errors, and messages', () => {
  const steps: TranscriptStepRow[] = [
    { kind: 'read', label: 'Reading design/layout.md', file: 'design/layout.md' },
    { kind: 'read', label: 'Reading design/feedback.md', file: 'design/feedback.md' },
    { kind: 'run', label: 'Searching for “trail” in packages/', detail: 'grep -n trail packages/' },
    { kind: 'read', label: 'Reading packages/shell/src/keys.ts', file: 'packages/shell/src/keys.ts' },
    { kind: 'edit', label: 'Editing docs/spec.md', file: 'docs/spec.md' },
    { kind: 'run', label: 'Checking git status', detail: 'git status --short' },
    { kind: 'error', label: 'tsc failed', bad: true },
    { kind: 'text', label: 'Fixed the type error.' },
  ];
  const entries = groupFinishedSteps(steps, false);
  expect(entries.map((e) => e.kind)).toEqual(['group', 'step', 'step', 'step', 'step']);

  const first = entries[0];
  expect(first?.kind).toBe('group');
  if (first?.kind === 'group') {
    expect(first.items).toHaveLength(4);
    expect(first.counts).toEqual({ reads: 3, searches: 1, commands: 0, thoughts: 0, other: 0 });
    expect(clusterLabel(first.counts, first.items.length, t)).toBe('Read 3 files · 1 search');
    expect(first.files).toEqual([
      { name: 'layout.md', title: 'design/layout.md' },
      { name: 'feedback.md', title: 'design/feedback.md' },
      { name: 'keys.ts', title: 'packages/shell/src/keys.ts' },
    ]);
    expect(first.rest).toHaveLength(1);
    expect(first.rest[0]?.step.label).toContain('Searching for');
  }
  expect(entries[1]?.kind === 'step' && entries[1].step.kind).toBe('edit');
  expect(entries[2]?.kind === 'step' && entries[2].step.kind).toBe('run'); // single trailing command stays ungrouped (< MIN_CLUSTER)
  expect(entries[3]?.kind === 'step' && entries[3].step.bad).toBe(true);
  expect(entries[4]?.kind === 'step' && entries[4].step.kind).toBe('text');
});

test('legacy "Using read file" tool rows classify as reads and coalesce duplicates', () => {
  const steps: TranscriptStepRow[] = [
    { kind: 'tool', label: 'Using read file' },
    { kind: 'tool', label: 'Using read file' },
    { kind: 'tool', label: 'Using read file' },
    { kind: 'tool', label: 'Using read file' },
    { kind: 'run', label: 'Searching for “trailActs”', detail: 'grep trailActs' },
    { kind: 'tool', label: 'Using read file' },
    { kind: 'tool', label: 'Using read file' },
  ];
  const entries = groupFinishedSteps(steps, false);
  expect(entries).toHaveLength(1);
  const g = entries[0];
  expect(g?.kind).toBe('group');
  if (g?.kind === 'group') {
    expect(g.counts).toEqual({ reads: 6, searches: 1, commands: 0, thoughts: 0, other: 0 });
    expect(clusterLabel(g.counts, g.items.length, t)).toBe('Read 6 files · 1 search');
    expect(g.rest.map((r) => r.step.label)).toEqual(['Using read file ×4', 'Searching for “trailActs”', 'Using read file ×2']);
  }
});

test('runs shorter than MIN_CLUSTER stay ungrouped when transcript is on', () => {
  expect(MIN_CLUSTER).toBe(3);
  const two: TranscriptStepRow[] = [
    { kind: 'run', label: 'Searching for foo' },
    { kind: 'read', label: 'Reading a.ts', file: 'a.ts' },
  ];
  expect(groupFinishedSteps(two, false).map((e) => e.kind)).toEqual(['step', 'step']);
});

test('quiet mode (Transcript off) folds every run of 1+ actions between messages, including edits and while running', () => {
  const steps: TranscriptStepRow[] = [
    { kind: 'text', label: 'Inspecting the auth handler and tests.' },
    { kind: 'read', label: 'Reading src/auth.ts', file: 'src/auth.ts', seq: 2 },
    { kind: 'run', label: 'Searching for “login” in src/', detail: 'grep -n login src/', seq: 3 },
    { kind: 'text', label: 'Found the timeout issue, patching now.' },
    { kind: 'edit', label: 'Editing src/auth.ts', file: 'src/auth.ts', seq: 5 },
    { kind: 'run', label: 'Running tests', detail: 'npm test', seq: 6 },
    { kind: 'error', label: '1 test failed', bad: true, seq: 7 },
    { kind: 'edit', label: 'Editing test/auth.test.ts', file: 'test/auth.test.ts', seq: 8 },
  ];

  // Even while running, quiet mode folds contiguous actions between agent messages/errors into compact expandable pills
  const liveQuiet = groupFinishedSteps(steps, true, (s) => s, true);
  expect(liveQuiet.map((e) => e.kind)).toEqual(['step', 'group', 'step', 'group', 'step', 'group']);

  const g1 = liveQuiet[1];
  expect(g1?.kind).toBe('group');
  if (g1?.kind === 'group') {
    expect(g1.items).toHaveLength(2);
    expect(clusterLabel(g1.counts, g1.items.length, t)).toBe('Read 1 file · 1 search');
    expect(g1.files).toEqual([{ name: 'auth.ts', title: 'src/auth.ts' }]);
    expect(g1.rest).toHaveLength(1);
    // stable key based on first step so streaming new steps into an open group preserves its expanded state
    expect(g1.key).toBe('group:2');
  }

  const g2 = liveQuiet[3];
  expect(g2?.kind).toBe('group');
  if (g2?.kind === 'group') {
    expect(g2.items).toHaveLength(2);
    expect(clusterLabel(g2.counts, g2.items.length, t)).toBe('Edited 1 file · 1 command');
    expect(g2.rest.map((r) => r.step.kind)).toEqual(['edit', 'run']);
  }

  const errStep = liveQuiet[4];
  expect(errStep?.kind === 'step' && errStep.step.bad).toBe(true);

  const g3 = liveQuiet[5];
  expect(g3?.kind).toBe('group');
  if (g3?.kind === 'group') {
    expect(g3.items).toHaveLength(1);
    expect(clusterLabel(g3.counts, g3.items.length, t)).toBe('Edited 1 file');
  }
});
