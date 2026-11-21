// My Stuff on phones with two computers (r298): the same thing on each read apart by its computer's name.
import { expect, test } from 'vitest';
import { computerOf, withComputer } from '../src/model.ts';

const names: Record<string, string> = { a: 'Studio', b: 'Laptop' };
const name = (p: string): string => names[p] ?? p;

test('a list mixing computers says each items computer on its meta line_r298', () => {
  const computer = computerOf(['a', 'b', 'a'], name);
  expect(withComputer('Document · md', computer('a'))).toBe('Document · md · Studio');
  expect(withComputer('Document · md', computer('b'))).toBe('Document · md · Laptop');
  expect(withComputer('', computer('b'))).toBe('Laptop');
});

test('one computer: the meta line is unchanged_r298', () => {
  const computer = computerOf(['a', 'a'], name);
  expect(computer('a')).toBeUndefined();
  expect(withComputer('Document · md', computer('a'))).toBe('Document · md');
  expect(computerOf([], name)('a')).toBeUndefined();
});
