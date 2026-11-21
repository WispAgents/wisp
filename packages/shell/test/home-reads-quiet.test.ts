// QA r2 phone #1: reads Home makes on every load must never toast when a peer refuses them (an older wisp without
// the command, a remote peer): their callers already fall back to "none".
import { expect, test } from 'vitest';
import { list } from '../src/frame/recipes.ts';
import { agentsOf } from '../src/frame/setup.ts';

test('homes_startup_reads_are_quiet', () => {
  expect([list.quiet, agentsOf.quiet]).toEqual([true, true]);
});
