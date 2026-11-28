// What this wisp is called (src/frame/here.ts, ADR-0148): "This computer", or a team hub's own name.
import { expect, test } from 'vitest';
import { hubName } from '../src/frame/here.ts';
import { whereItems } from '../src/frame/where.ts';

const client = (info: { kind?: string; name: string } | undefined) => ({ home: () => 'h', peers: { info: () => () => info } });

test('a person’s own wisp has no hub name, before or after its welcome', () => {
  expect(hubName(client({ name: 'Studio' }))).toBeUndefined();
  expect(hubName(client(undefined))).toBeUndefined();
});

test('a team’s hub is called by its name, a server, never This computer', () => {
  expect(hubName(client({ kind: 'hub', name: 'Acme hub' }))).toBe('Acme hub');
  const t = (k: string): string => (k === 'shell.card.thisComputer' ? 'This computer' : k);
  const items = whereItems(['h'], 'h', 'h', (p) => p, t, [], { label: 'Acme hub', icon: 'server' });
  expect(items[0]).toMatchObject({ value: 'h', label: 'Acme hub', icon: 'server', checked: true });
});
