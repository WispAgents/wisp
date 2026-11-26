// The grid's per-user sandbox cap (grid/sandboxes.js): refused with 409 and a typed code, so wisp tells a full grid
// (a crew job waits and tries again) from any other refusal by the code, not by the sentence.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SANDBOX_CAP, createSandboxes } from '../grid/sandboxes.js';

// A db where the user already has `have` sandboxes, and secrets that keep nothing.
function grid(have, perUser) {
  const db = { get: (sql) => (sql.includes('count(*)') ? { n: have } : null), all: () => [], run: () => {} };
  const secrets = { ensure: () => 's', open: () => '', seal: () => {}, drop: () => {} };
  return createSandboxes({ db, cfg: { sandboxes: { perUser }, instanceUrl: (l) => `https://${l}` }, secrets });
}

test('the cap is a 409 whose body says sandbox_cap, how many the user has and the most', () => {
  const sb = grid(3, 3);
  let err;
  try {
    sb.create({ id: 'u1', handle: 'ada' }, 'work');
  } catch (e) {
    err = e;
  }
  assert.ok(err, 'refused');
  assert.equal(err.status, 409);
  // the body the server sends: `{ error, ...err.body }`
  const body = { error: err.message, ...err.body };
  assert.deepEqual({ code: body.code, have: body.have, max: body.max }, { code: SANDBOX_CAP, have: 3, max: 3 });
  assert.equal(SANDBOX_CAP, 'sandbox_cap');
  assert.match(body.error, /the most this grid keeps/);
  assert.match(body.error, /Settings → Computers → Sandboxes/);
  assert.doesNotMatch(body.error, /Delete a sandboxed task/);
});

test('under the cap a sandbox is made', () => {
  const made = grid(1, 3).create({ id: 'u1', handle: 'ada' }, 'work');
  assert.equal(made.row.name, 'work');
});
