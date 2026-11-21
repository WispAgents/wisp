// QA round 2 desktop #12: the list's code arrived before its rows and drew "No tasks yet" until they did. An empty list
// says it's empty only once the peer's list topic has landed (entities.loaded); until then, rows' shapes.
import { expect, test } from 'vitest';
import { listFace } from '../src/model/list-face.ts';

test('an_empty_list_is_loading_until_its_topic_has_landed', () => {
  expect(listFace(0, false)).toBe('loading');
  expect(listFace(0, true)).toBe('empty');
  expect(listFace(3, false)).toBe('rows');
  expect(listFace(3, true)).toBe('rows');
});
