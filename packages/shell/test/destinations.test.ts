// ADR-0065 step 8: the phone apps' destinations and the routes under each (design/mobile.md §A.1).
import { expect, test } from 'vitest';
import type { ExtensionId, Json, Main, SessionId } from '@wisp/client';
import { COMPUTERS_VIEW, DESTINATIONS, destinationOf, destinationsOf, intentOf } from '../src/frame/destinations.ts';

const view = (id: string): Main => ({ kind: 'view', id: id as ExtensionId, params: null as Json });

test('the_phone_routes_map_to_their_destinations', () => {
  expect(DESTINATIONS.map((d) => d.id), 'v1’s words, design/mobile.md §A.1’s order').toEqual(['tasks', 'crew', 'stuff', 'computers']);
  expect(destinationOf({ kind: 'home' })).toBe('tasks');
  expect(destinationOf({ kind: 'task', id: 'studio:t1' as SessionId }), 'a task is under Tasks').toBe('tasks');
  expect(destinationOf({ kind: 'crew', id: 'm1' as never }), 'a member’s thread is Crew’s').toBe('crew');
  expect(destinationOf(view('crew.org'))).toBe('crew');
  expect(destinationOf(view('stuff.page'))).toBe('stuff');
  expect(destinationOf(view(COMPUTERS_VIEW))).toBe('computers');
  expect(destinationOf(view('machine.this')), 'another view stays under Tasks').toBe('tasks');
  for (const d of DESTINATIONS) {
    const go = intentOf(d);
    const main: Main = go.t === 'openView' ? view(String(go.id)) : { kind: 'home' };
    expect(destinationOf(main), `${d.id}: selecting it lands on it`).toBe(d.id);
  }
});

test('a_destination_without_its_view_is_not_offered', () => {
  expect(destinationsOf((v) => v === COMPUTERS_VIEW).map((d) => d.id)).toEqual(['tasks', 'computers']);
  expect(destinationsOf(() => false).map((d) => d.id)).toEqual(['tasks']);
});
