// What this wisp is called (src/frame/here.ts, ADR-0148): "This computer", or a team hub's own name.
import { expect, test } from 'vitest';
import { hereIcon, hereName, onHub, type HereClient } from '../src/frame/here.ts';

import { whereItems } from '../src/frame/where.ts';

const t = (k: string): string => (k === 'shell.card.thisComputer' ? 'This computer' : k);
const client = (info: { kind?: string; name: string } | undefined): HereClient => ({
  home: () => 'h',
  peers: { info: () => () => info, name: () => info?.name ?? '' },
});

test('a person’s own wisp is This computer, a laptop', () => {
  const c = client({ name: 'Studio' });
  expect([onHub(c), hereName(c, t), hereIcon(c)]).toEqual([false, 'This computer', 'laptop']);
  expect(hereName(client(undefined), t), 'before its welcome').toBe('This computer');
});

test('a team’s hub is called by its name, a server, never This computer', () => {
  const c = client({ kind: 'hub', name: 'Acme hub' });
  expect([onHub(c), hereName(c, t), hereIcon(c)]).toEqual([true, 'Acme hub', 'server']);
  const items = whereItems(['h'], 'h', 'h', (p) => p, t, [], { label: hereName(c, t), icon: hereIcon(c) });
  expect(items[0]).toMatchObject({ value: 'h', label: 'Acme hub', icon: 'server', checked: true });
});
