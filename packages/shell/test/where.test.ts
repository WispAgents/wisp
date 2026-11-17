// New task's Run on (src/frame/where.ts, v1 launcher.js where-btn): this computer, then the others, one picked.
import { expect, test } from 'vitest';
import { sandboxPeers, targetValue, whereItems } from '../src/frame/where.ts';

test('Run on lists this computer first and the others after, only the picked one checked_v1_run_on', () => {
  const t = (k: string): string => (k === 'shell.card.thisComputer' ? 'This computer' : k);
  const name = (p: string): string => ({ studio: 'Studio', box: 'Cloud' })[p] ?? p;
  const items = whereItems(['laptop', 'studio', 'box'], 'studio', 'laptop', name, t);
  expect(items.map((i) => [(i as { value: string }).value, (i as { label: string }).label, (i as { checked?: boolean }).checked])).toEqual([
    ['laptop', 'This computer', false],
    ['studio', 'Studio', true],
    ['box', 'Cloud', false],
  ]);
  expect(whereItems(['laptop'], 'laptop', 'laptop', name, t)).toHaveLength(1);
});

test('a sandbox is never a computer to run on, and a plugin place comes after the computers, checked when picked (TRY-19)', () => {
  const t = (k: string): string => (k === 'shell.card.thisComputer' ? 'This computer' : k);
  const rows = [
    { id: 'laptop:studio', body: { data: {} } },
    { id: 'laptop:a1b2c3', body: { data: { sandbox: 'sb-1', owner: { task: 'laptop:t1' } } } },
    { id: 'laptop:nodata' },
  ];
  const boxes = sandboxPeers(rows);
  expect([...boxes]).toEqual(['a1b2c3']);
  const peers = ['laptop', 'studio', 'a1b2c3'].filter((p) => !boxes.has(p));
  const items = whereItems(peers, targetValue('sandbox.box'), 'laptop', (p) => p, t, [{ id: 'sandbox.box', label: 'Sandbox', icon: 'box' }]);
  expect(items.map((i) => [(i as { value: string }).value, (i as { checked?: boolean }).checked])).toEqual([
    ['laptop', false],
    ['studio', false],
    ['launch:sandbox.box', true],
  ]);
});
