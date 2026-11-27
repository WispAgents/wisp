/// <reference types="vite/client" />
// R4-3, R4-5, R4-6 as the task page wires them: Find and the Markdown serializer load when used (their own chunks), a
// turn's tools hold Copy turn as Markdown, the task's menu holds Find in task (a phone has no ⌘F), and one open store
// per turn goes to both of its transcripts.
import { expect, test } from 'vitest';
import view from '../src/frame/TaskView.tsx?raw';
import turn from '../src/frame/TaskTurn.tsx?raw';
import actions from '../src/frame/task-actions.ts?raw';
import find from '../src/frame/task-find-view.ts?raw';
import turnList from '../src/frame/TaskTurns.tsx?raw';

// the task page and the modules it's split into: its turns, its actions (More, the keys), Find's wiring and the turn list that holds Find
const task = [view, turn, actions, find, turnList].join('\n');

test('find_and_the_markdown_copy_load_when_used_not_with_the_task_page', () => {
  expect(task).toMatch(/lazy\(\(\) => import\('\.\/TaskFind\.tsx'\)\)/);
  expect(task).not.toMatch(/^import [^;]* from '\.\/TaskFind\.tsx'/m);
  expect(task).toMatch(/import\('\.\/turn-markdown\.ts'\)/);
  expect(task).not.toMatch(/^import [^;]* from '\.\/turn-markdown\.ts'/m);
});

test('a_turns_tools_copy_it_as_markdown_and_the_task_menu_finds_in_it', () => {
  expect(task).toMatch(/\{ id: 'copy-md', icon: 'copy', label: t\('shell\.task\.copyMarkdown'\)/);
  expect(task).toMatch(/value: 'find', label: t\('shell\.find\.menu'\)/);
  expect(task).toMatch(/else if \(v === 'find'\) openFind\(\);/);
  expect(task).toMatch(/else if \(act === 'find'\) openFind\(\);/);
});

test('both_transcripts_of_a_turn_get_its_one_open_store', () => {
  expect(task.match(/<TurnTranscript[\s\S]*?\/>/g)?.every((x) => /\bopen=\{open\}/.test(x))).toBe(true);
  expect(task).toMatch(/const open = openOf\(turn\.id\);/);
});
