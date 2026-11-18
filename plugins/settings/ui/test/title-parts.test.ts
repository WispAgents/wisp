import { expect, test } from 'vitest';
import { titleParts } from '../src/words.ts';

test('all_settings_shows_a_declared_title_as_a_plain_label_and_its_help', () => {
  // manifests write "Label: what it does"; the row shows the label, and the help under it
  expect(titleParts('Check compactions: after a compaction, wisp asks a copy of the conversation what it still remembers')).toEqual({
    label: 'Check compactions',
    help: 'After a compaction, wisp asks a copy of the conversation what it still remembers',
  });
  expect(titleParts('Drain watch: tell the phone when a plan moves much faster than usual')).toEqual({ label: 'Drain watch', help: 'Tell the phone when a plan moves much faster than usual' });
  // no "label: " up front (a sentence, a time, a long first clause): all label
  expect(titleParts('Theme')).toEqual({ label: 'Theme', help: '' });
  expect(titleParts('Runs at 09:30 each day')).toEqual({ label: 'Runs at 09:30 each day', help: '' });
  expect(titleParts('A long sentence. Then this: and that')).toEqual({ label: 'A long sentence. Then this: and that', help: '' });
  expect(titleParts('x'.repeat(60) + ': help')).toEqual({ label: `${'x'.repeat(60)}: help`, help: '' });
});

// Audit A F12: a ": " inside parentheses split the sentence ("Where a pasted repo URL is cloned (''" over "The first
// of ~/code…)"), ".env" read as a full stop, and a long label in front of a parenthesis stayed all label
test('a_parenthesis_is_help_and_a_colon_inside_it_never_splits_audit_a_f12', () => {
  expect(titleParts("Where a pasted repo URL is cloned ('': the first of ~/code, ~/dev that exists, else ~/code)")).toEqual({
    label: 'Where a pasted repo URL is cloned',
    help: "'': the first of ~/code, ~/dev that exists, else ~/code",
  });
  expect(titleParts("The model Claude Code's subagents run on (task: the task's own)")).toEqual({ label: "The model Claude Code's subagents run on", help: "Task: the task's own" });
  expect(titleParts('Idle worktrees go after this many days (unset: 7, counted from when wisp got it)')).toEqual({ label: 'Idle worktrees go after this many days', help: 'Unset: 7, counted from when wisp got it' });
  expect(titleParts("Lean Claude Code runs (no telemetry or update checks, and no claude.ai connectors): on, only wisp's own side runs; all, every run; off, none")).toEqual({
    label: 'Lean Claude Code runs',
    help: "No telemetry or update checks, and no claude.ai connectors. On, only wisp's own side runs; all, every run; off, none",
  });
  expect(titleParts('Hide .env values: what a task shows of a .env file leaves wisp as dots')).toEqual({ label: 'Hide .env values', help: 'What a task shows of a .env file leaves wisp as dots' });
  expect(titleParts("Default project: where a task from a channel runs when its message names none ('': the last used)")).toEqual({
    label: 'Default project',
    help: "Where a task from a channel runs when its message names none ('': the last used)",
  });
});
