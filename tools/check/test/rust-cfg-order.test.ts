import assert from 'node:assert/strict';
import test from 'node:test';
import { rustCfgOrder } from '../rules/rust-cfg-order.ts';
import { fixture, messages, run } from './helpers.ts';

test("rust-cfg-order: a test file's #![allow]/#![expect] below its #![cfg] fails; allow first, a cfg in a comment and src files pass", async () => {
  const root = fixture('rust-cfg-order');
  assert.deepEqual(await run(rustCfgOrder, root), [
    'crates/a/tests/bad.rs:3 rust-cfg-order', // the allow under #![cfg(unix)]
    'crates/a/tests/bad.rs:4 rust-cfg-order', // an expect too
  ]);
  const [m] = await messages(rustCfgOrder, root);
  assert.match(m ?? '', /Put the #!\[allow\] first/);
});
