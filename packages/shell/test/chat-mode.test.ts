import { expect, test } from 'vitest';
import { buildChatSteps, turnSteps, type RawTurnEvent } from '../src/frame/chat-steps.ts';
import { parseInlineChatBlocks } from '../src/frame/chat-blocks.ts';
import { asChatMode } from '../src/frame/chat-parse.ts';
import { turnClock } from '../src/frame/turn-clock.ts';

const EVENTS: RawTurnEvent[] = [
  { seq: 1, kind: 'thinking', body: { text: 'Inspect the auth handler first.\nThen check tests.' } },
  { seq: 2, kind: 'tool', body: { tool: 'read', label: 'Reading auth.ts', callId: 'c1' } },
  { seq: 3, kind: 'tool_output', body: { callId: 'c1', text: 'export function login() {}' } },
  { seq: 4, kind: 'text', body: { text: 'Found the timeout issue, patching now.' } },
  { seq: 5, kind: 'tool', body: { tool: 'run', label: 'Running tests', callId: 'c2' } },
  { seq: 6, kind: 'tool_error', body: { name: 'Bash', text: '1 test failed' } },
  { seq: 7, kind: 'text', body: { text: 'Fixed the timeout in auth.ts.' } },
];

test('chat mode toolCallVisibility filters tool calls while keeping errors', () => {
  const showAll = buildChatSteps(EVENTS, [], { chatMode: true, toolCalls: 'showAll', showThinking: true, running: false, finalText: 'Fixed the timeout in auth.ts.' });
  expect(showAll.map((s) => s.kind)).toEqual(['thinking', 'read', 'text', 'run', 'error']);
  expect(showAll[0]?.label).toBe('Inspect the auth handler first.');
  expect(showAll[0]?.out).toContain('Then check tests.');

  // hideSuccessful hides finished tools when not running, keeps errors + thinking + intermediate text
  const hideOkDone = buildChatSteps(EVENTS, [], { chatMode: true, toolCalls: 'hideSuccessful', showThinking: true, running: false, finalText: 'Fixed the timeout in auth.ts.' });
  expect(hideOkDone.map((s) => s.kind)).toEqual(['thinking', 'text', 'error']);

  // hideSuccessful keeps the currently active (unfinished) tool while running
  const hideOkLive = buildChatSteps(EVENTS.slice(0, 5), [], { chatMode: true, toolCalls: 'hideSuccessful', showThinking: false, running: true });
  expect(hideOkLive.map((s) => s.kind)).toEqual(['text', 'run']);

  // hideAll hides every routine tool call even while running, keeping errors + text
  const hideAllLive = buildChatSteps(EVENTS, [], { chatMode: true, toolCalls: 'hideAll', showThinking: false, running: true });
  expect(hideAllLive.map((s) => s.kind)).toEqual(['text', 'error', 'text']);

  // zen hides tools, thinking, and intermediate messages — only errors stay in the stream
  const zen = buildChatSteps(EVENTS, [], { chatMode: true, toolCalls: 'zen', showThinking: true, running: false, finalText: 'Fixed the timeout in auth.ts.' });
  expect(zen.map((s) => s.kind)).toEqual(['error']);
});

test('turnClock respects 12-hour and 24-hour timeFormat settings', () => {
  const afternoon = new Date(2026, 10, 22, 15, 7).getTime();
  expect(turnClock(afternoon, 'en-US', '12-hour')).toMatch(/3:07\s*PM/i);
  expect(turnClock(afternoon, 'en-GB', '24-hour')).toBe('15:07');
});

test('parseInlineChatBlocks renders wisp blocks mid-chat while leaving plain messages untouched', () => {
  expect(parseInlineChatBlocks('Just checking login.ts now.')).toBeUndefined();
  const mid = 'Here is the current test run:\n\n```wisp\n{"blocks":[{"type":"tests","command":"npm test","passed":12,"failed":0}]}\n```\n\nNow cleaning up the docs.';
  expect(parseInlineChatBlocks(mid)).toEqual([
    { type: 'markdown', text: 'Here is the current test run:' },
    { type: 'tests', command: 'npm test', passed: 12, failed: 0 },
    { type: 'markdown', text: 'Now cleaning up the docs.' },
  ]);
});

test('chat mode is on by default and only an explicit off brings back the cards view', () => {
  expect([undefined, null, 'on', true].map(asChatMode)).toEqual([true, true, true, true]);
  expect(['off', false].map(asChatMode)).toEqual([false, false]);
});

test('chat mode drops the last message when it is the answer, even when the answer carries wisp blocks', () => {
  const answer = 'Fixed the timeout.\n\n```wisp\n{"blocks":[{"type":"hero","title":"Done"}]}\n```\n\nThe test passes now.';
  const events: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { kind: 'tool', tool: 'read', label: 'Read auth.ts', output: 'x' } },
    { seq: 2, kind: 'text', body: { kind: 'text', text: answer } },
  ];
  const opts = { chatMode: true, toolCalls: 'showAll' as const, showThinking: true, running: false };
  // the last markdown block alone ("The test passes now.") never matched: the answer showed twice
  expect(buildChatSteps(events, [], { ...opts, finalText: 'The test passes now.' }).map((s) => s.kind)).toEqual(['read', 'text']);
  expect(buildChatSteps(events, [], { ...opts, finalText: [answer, 'The test passes now.'] }).map((s) => s.kind)).toEqual(['read']);
  // whitespace aside
  expect(buildChatSteps(events, [], { ...opts, finalText: `${answer.replace(/\n\n/g, '\n')}  ` }).map((s) => s.kind)).toEqual(['read']);
  // while it runs, the message stays (the answer isn't there yet)
  expect(buildChatSteps(events, [], { ...opts, running: true, finalText: answer }).map((s) => s.kind)).toEqual(['read', 'text']);
  // pure wisp-block answer (no surrounding prose)
  const blockOnly = '```wisp\n{"blocks":[{"type":"hero","title":"Done"}]}\n```';
  const blockEvents: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { kind: 'tool', tool: 'read', label: 'Read nav.ts', output: 'x' } },
    { seq: 2, kind: 'text', body: { kind: 'text', text: blockOnly } },
  ];
  expect(buildChatSteps(blockEvents, [], { ...opts, finalText: [blockOnly, undefined] }).map((s) => s.kind)).toEqual(['read']);
  // clipped at MAX_TEXT (20,000 UTF-16 code units) while turn.answer holds the full unclipped message
  const longAnswer = `${'a'.repeat(20_050)} end`;
  const clippedEvents: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { kind: 'tool', tool: 'read', label: 'Read big.ts', output: 'x' } },
    { seq: 2, kind: 'text', body: { kind: 'text', text: longAnswer.slice(0, 20_000) } },
  ];
  expect(buildChatSteps(clippedEvents, [], { ...opts, finalText: longAnswer }).map((s) => s.kind)).toEqual(['read']);
  // fallback activity (turns without runtime_event rows) also deduplicates trailing text matching the answer
  expect(
    buildChatSteps([], [{ kind: 'read', label: 'Read nav.ts' }, { kind: 'text', label: blockOnly }], { ...opts, finalText: blockOnly }).map((s) => s.kind),
  ).toEqual(['read']);
});

// audit F3: a failed call's error named its call, but showed as a row of its own after a call that looked fine
test('a tool error that names its call marks that call failed, with the error as its output', () => {
  const events: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { tool: 'run', label: 'Running tests', callId: 'a' } },
    { seq: 2, kind: 'tool_error', body: { name: 'Bash', text: 'assertion failed', callId: 'a' } },
  ];
  for (const toolCalls of ['showAll', 'hideSuccessful', 'hideAll', 'zen'] as const) {
    const rows = buildChatSteps(events, [], { chatMode: true, toolCalls, showThinking: true, running: false });
    expect(rows, toolCalls).toEqual([{ kind: 'run', label: 'Running tests', out: 'assertion failed', seq: 2, at: 1, bad: true }]);
  }
  expect(turnSteps(events, [])).toEqual([{ kind: 'run', label: 'Running tests', out: 'assertion failed', seq: 2, at: 1, bad: true }]);
  // while it runs, the failed call is done: not the active one
  expect(buildChatSteps(events, [], { chatMode: true, toolCalls: 'hideSuccessful', showThinking: true, running: true }).length).toBe(1);
});

// audit F8: the row names the event whose whole output the server keeps
test('a clipped output carries its whole text count on the row it shows on', () => {
  const events: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { tool: 'run', label: 'npm test', callId: 'a' } },
    { seq: 2, kind: 'tool_output', body: { callId: 'a', text: 'head … cut … tail', full: { lines: 900 } } },
    { seq: 3, kind: 'tool', body: { tool: 'run', label: 'make', callId: 'b' } },
    { seq: 4, kind: 'tool_error', body: { name: 'Bash', text: 'head … cut … error', callId: 'b', full: { lines: 40 } } },
  ];
  const rows = buildChatSteps(events, [], { chatMode: true, toolCalls: 'showAll', showThinking: true, running: false });
  expect(rows.map((r) => [r.seq, r.full?.lines])).toEqual([[2, 900], [4, 40]]);
  expect(turnSteps(events, []).map((r) => [r.seq, r.full?.lines])).toEqual([[2, 900], [4, 40]]);
});

test('a tool error naming no call it follows stays a row of its own', () => {
  const events: RawTurnEvent[] = [
    { seq: 1, kind: 'tool', body: { tool: 'run', label: 'Running tests', callId: 'a', output: 'ok' } },
    { seq: 2, kind: 'tool_error', body: { name: 'apply_patch', text: 'The patch did not apply.', callId: 'zz' } },
    { seq: 3, kind: 'tool_error', body: { name: 'Bash', text: 'exit 1' } },
  ];
  const rows = buildChatSteps(events, [], { chatMode: true, toolCalls: 'showAll', showThinking: true, running: false });
  expect(rows.map((r) => [r.kind, r.bad ?? false])).toEqual([['run', false], ['error', true], ['error', true]]);
  expect(turnSteps(events, []).map((r) => [r.kind, r.label, r.bad ?? false])).toEqual([
    ['run', 'Running tests', false],
    ['error', 'The patch did not apply.', true],
    ['error', 'exit 1', true],
  ]);
});
