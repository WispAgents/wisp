import { expect, test } from 'vitest';
import { MAX_ZEN, asZenList, withZen, zenSet } from '../src/frame/task-zen.ts';
import { matches } from '../src/keys.ts';
import type { ChatSettings } from '../src/frame/chat-settings.ts';

const base: ChatSettings = {
  chatMode: () => false,
  toolCalls: () => 'showAll',
  showThinking: () => true,
  sendShortcut: () => 'enter' as never,
  chatWidth: () => 'comfortable',
  timeFormat: () => 'locale',
};

test('zen on one task forces Chat mode and zen tool calls; off, Settings wins', () => {
  let on = false;
  const chat = withZen(base, () => on);
  expect([chat.chatMode(), chat.toolCalls()]).toEqual([false, 'showAll']);
  on = true;
  expect([chat.chatMode(), chat.toolCalls(), chat.showThinking(), chat.chatWidth()]).toEqual([true, 'zen', true, 'comfortable']);
});

test('zenSet toggles one task, keeps others, caps the oldest out', () => {
  expect(zenSet(['a', 'b'], 'a', true)).toEqual(['b', 'a']);
  expect(zenSet(['a', 'b'], 'a', false)).toEqual(['b']);
  expect(zenSet([], 'x', false)).toEqual([]);
  const many = Array.from({ length: MAX_ZEN }, (_, i) => `t${i}`);
  const next = zenSet(many, 'new', true);
  expect(next).toHaveLength(MAX_ZEN);
  expect(next[0]).toBe('t1');
  expect(next.at(-1)).toBe('new');
  expect(asZenList({ bad: 1 })).toEqual([]);
  expect(asZenList(['a', 2, 'b'])).toEqual(['a', 'b']);
});

test('ctrl+. is literal Control on a Mac too, not ⌘', () => {
  const ev = { key: '.', metaKey: false, ctrlKey: true, altKey: false, shiftKey: false };
  expect(matches('ctrl+.', ev, true)).toBe(true);
  expect(matches('ctrl+.', ev, false)).toBe(true);
  expect(matches('ctrl+.', { ...ev, ctrlKey: false, metaKey: true }, true)).toBe(false);
});
