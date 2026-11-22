import { expect, test } from 'vitest';
import { shouldSendOnKey } from '../src/composer/send-shortcut.ts';

const key = (overrides: Partial<Parameters<typeof shouldSendOnKey>[0]> = {}) => ({
  key: 'Enter',
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  isComposing: false,
  ...overrides,
});

test('enter shortcut sends on Enter and never on Shift+Enter, Alt+Enter or composing', () => {
  expect(shouldSendOnKey(key(), 'hi', 'enter')).toBe(true);
  expect(shouldSendOnKey(key(), 'line 1\nline 2', 'enter')).toBe(true);
  expect(shouldSendOnKey(key({ shiftKey: true }), 'hi', 'enter')).toBe(false);
  expect(shouldSendOnKey(key({ altKey: true }), 'hi', 'enter')).toBe(false);
  expect(shouldSendOnKey(key({ isComposing: true }), 'hi', 'enter')).toBe(false);
  expect(shouldSendOnKey(key({ key: 'a' }), 'hi', 'enter')).toBe(false);
});

test('mod-enter-multiline sends single-line on Enter and requires Cmd/Ctrl+Enter for multiline', () => {
  expect(shouldSendOnKey(key(), 'single line', 'mod-enter-multiline')).toBe(true);
  expect(shouldSendOnKey(key(), 'line 1\nline 2', 'mod-enter-multiline')).toBe(false);
  expect(shouldSendOnKey(key({ metaKey: true }), 'line 1\nline 2', 'mod-enter-multiline')).toBe(true);
  expect(shouldSendOnKey(key({ ctrlKey: true }), 'line 1\nline 2', 'mod-enter-multiline')).toBe(true);
});

test('mod-enter always requires Cmd or Ctrl with Enter', () => {
  expect(shouldSendOnKey(key(), 'single line', 'mod-enter')).toBe(false);
  expect(shouldSendOnKey(key({ metaKey: true }), 'single line', 'mod-enter')).toBe(true);
  expect(shouldSendOnKey(key({ ctrlKey: true }), 'line 1\nline 2', 'mod-enter')).toBe(true);
  expect(shouldSendOnKey(key({ metaKey: true, shiftKey: true }), 'hi', 'mod-enter')).toBe(false);
});
