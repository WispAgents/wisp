import { expect, test } from 'vitest';
import { gridUrl } from '../src/commands.ts';
import { gridField } from '../src/grid-field.ts';

const row = (x: object) => ({ signedIn: false, grid: '', handle: '', machine: '', computer: null, signin: null, ...x }) as never;

// Setup audit F2: after a declined sign-in to your own grid the address read grid.trywisp.dev again, and Sign in asked
// the hosted grid; Add an account offered it too while you were on another. The grid you chose stays.
test('the_address_is_the_grid_you_chose_never_quietly_the_hosted_one', () => {
  // a sign-in under way, or one just declined: its grid, scheme kept for a plain-http one
  expect(gridField(row({ signin: { grid: 'http://wisp.localhost:4971', state: 'denied' } }))).toBe('http://wisp.localhost:4971');
  expect(gridField(row({ signin: { grid: 'https://grid.acme.dev', state: 'waiting' } }))).toBe('grid.acme.dev');
  // signed in elsewhere: Add an account starts on that grid
  expect(gridField(row({ signedIn: true, grid: 'https://grid.acme.dev' }))).toBe('grid.acme.dev');
  // nothing yet: wisp Cloud
  expect(gridField(undefined)).toBe('grid.trywisp.dev');
  expect(gridField(row({}))).toBe('grid.trywisp.dev');
  // what's typed goes where it says; a local grid is plain http, as the server reads it
  expect(gridUrl('http://wisp.localhost:4971')).toBe('http://wisp.localhost:4971');
  expect(gridUrl('wisp.localhost:4971')).toBe('http://wisp.localhost:4971');
  expect(gridUrl('127.0.0.1:8080')).toBe('http://127.0.0.1:8080');
  expect(gridUrl('grid.acme.dev')).toBe('https://grid.acme.dev');
});
