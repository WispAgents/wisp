import assert from 'node:assert/strict';
import test from 'node:test';
import { adrNumbers } from '../rules/adr-numbers.ts';
import { fixture, run, writeTree } from './helpers.ts';

test('adr-numbers: a_number_two_files_share_a_heading_that_names_another_and_an_index_row_off_its_link_fail', async () => {
  assert.deepEqual(await run(adrNumbers, fixture('adr-numbers')), [
    'docs/adr/0002-workflows.md:1 adr-numbers', // the second file with 0002, in name order
    'docs/adr/0003-heading-says-another.md:1 adr-numbers', // heading says ADR-0004
    'docs/adr/README.md:6 adr-numbers', // row 0004 links an 0003 file
    'docs/adr/README.md:7 warn:adr-numbers', // links a file that is gone
  ]);
});

test('adr-numbers: renumbered_to_the_next_free_number_passes', async () => {
  const tree = new Map([
    ['docs/adr/0119-workflows.md', '# ADR-0119: workflows\n'],
    ['docs/adr/0120-prices.md', '# ADR-0120: prices\n'],
    ['docs/adr/0121-handoff.md', '# ADR-0121: handoff\n'],
    ['docs/adr/README.md', '| [0119](0119-workflows.md) | a |\n| [0120](0120-prices.md) | b |\n| [0121](0121-handoff.md) | c |\n'],
  ]);
  assert.deepEqual(await run(adrNumbers, writeTree(tree)), []);
});

test('adr-numbers: a_tree_with_no_adrs_has_nothing_to_say', async () => {
  assert.deepEqual(await run(adrNumbers, writeTree(new Map())), []);
});
