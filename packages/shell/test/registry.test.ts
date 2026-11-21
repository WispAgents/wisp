// The extension tree's rules (plan §6.6): overrides take the replaced one's place, tree config
// disables / moves / orders / configures, a refused config keeps the default, inputs only change
// when something on them does.
import { expect, test } from 'vitest';
import type { ExtensionDef, PluginUiDef } from '../src/contract.ts';
import { createRegistry } from '../src/registry.ts';

const C = (() => null) as unknown as ExtensionDef['component'];
const ext = (id: string, input: string, more: Partial<ExtensionDef> = {}): ExtensionDef => ({ id, attachTo: { id: input }, component: C, ...more });
const plugin = (id: string, extensions: ExtensionDef[], more: Partial<PluginUiDef> = {}): PluginUiDef => ({ id, extensions, ...more });
const ids = (r: ReturnType<typeof createRegistry>, input: string): string[] => r.input(input)().map((e) => e.id);

test('extensions sit on their input in registration order', () => {
  const r = createRegistry();
  r.add(plugin('wisp.a', [ext('a.one', 'shell.composer:bar'), ext('a.two', 'shell.composer:bar')]));
  r.add(plugin('wisp.b', [ext('b.one', 'shell.composer:bar')]));
  expect(ids(r, 'shell.composer:bar')).toEqual(['a.one', 'a.two', 'b.one']);
});

test('an override takes the place of the one it replaces', () => {
  const warns: string[] = [];
  const r = createRegistry((m) => warns.push(m));
  r.add(plugin('wisp.a', [ext('a.one', 'x:y'), ext('a.two', 'x:y')]));
  r.add(plugin('acme', [ext('acme.mine', 'x:y', { overrides: 'a.one' })]));
  expect(ids(r, 'x:y')).toEqual(['acme.mine', 'a.two']);
  r.add(plugin('other', [ext('other.mine', 'x:y', { overrides: 'a.one' })]));
  expect(ids(r, 'x:y')).toEqual(['other.mine', 'a.two']);
  expect(warns.some((w) => w.includes('both override a.one'))).toBe(true);
});

test('tree config: disable, attach, order, later plugins win', () => {
  const r = createRegistry();
  r.add(plugin('wisp.a', [ext('a.one', 'x:y'), ext('a.two', 'x:y'), ext('a.three', 'x:y')]));
  r.add(plugin('spell', [], { tree: { disable: ['a.two'], attach: { 'a.three': { id: 'x', input: 'z' } }, order: { 'a.one': 5 } } }));
  expect(ids(r, 'x:y')).toEqual(['a.one']);
  expect(ids(r, 'x:z')).toEqual(['a.three']);
  r.remove('spell');
  expect(ids(r, 'x:y')).toEqual(['a.one', 'a.two', 'a.three']);
});

test('config is parsed by the extension; a refused value keeps the default', () => {
  const warns: string[] = [];
  const r = createRegistry((m) => warns.push(m));
  const config = { default: { compact: false }, parse: (v: unknown) => {
    if (typeof v !== 'object' || v === null || typeof (v as { compact?: unknown }).compact !== 'boolean') throw new Error('bad');
    return v as { compact: boolean };
  } };
  r.add(plugin('wisp.a', [ext('a.pick', 'x:y', { config: config as NonNullable<ExtensionDef['config']> })]));
  expect(r.input('x:y')()[0]?.config).toEqual({ compact: false });
  r.add(plugin('s1', [], { tree: { config: { 'a.pick': { compact: true } } } }));
  expect(r.input('x:y')()[0]?.config).toEqual({ compact: true });
  r.add(plugin('s2', [], { tree: { config: { 'a.pick': { compact: 'yes' } } } }));
  expect(r.input('x:y')()[0]?.config).toEqual({ compact: false });
  expect(warns.some((w) => w.includes('a.pick'))).toBe(true);
});

test('an input keeps the same array when another input changes', () => {
  const r = createRegistry();
  r.add(plugin('wisp.a', [ext('a.one', 'x:y')]));
  const before = r.input('x:y')();
  r.add(plugin('wisp.b', [ext('b.one', 'x:other')]));
  expect(r.input('x:y')()).toBe(before);
});

test('a plugin registered twice is refused; views, blocks and panes resolve', () => {
  const r = createRegistry();
  const view = { kind: 'a.board', slot: 'main' as const, parse: (p: unknown) => p, component: C as never };
  r.add(plugin('wisp.a', [], { views: [view], blocks: [{ type: 'hero', component: C as never }], settings: [{ id: 'a.p', group: 'general', title: 't', icon: 'x', component: C as never, order: 2 }] }));
  expect(() => r.add(plugin('wisp.a', []))).toThrow();
  r.add(plugin('wisp.b', [], { blocks: [{ type: 'hero', component: (() => 1) as never }], settings: [{ id: 'b.p', group: 'general', title: 't', icon: 'x', component: C as never, order: 1 }] }));
  expect(r.view('a.board')).toBe(view);
  expect(r.viewOwner('a.board')).toBe('wisp.a');
  expect(r.block('hero')?.component).not.toBe(C);
  expect(r.settingsPanes().map((p) => p.id)).toEqual(['b.p', 'a.p']);
});

test('a_later_part_joins_its_plugin_in_place_ADR_0067', () => {
  const r = createRegistry();
  r.add(plugin('wisp.a', [ext('a.one', 'x:y')]));
  r.add(plugin('wisp.b', [ext('b.one', 'x:y')]));
  r.extend('wisp.a', { extensions: [ext('a.two', 'x:y')], palette: [{ id: 'a.pal', items: () => [] }] });
  // in wisp.a's place among the plugins, after its own: a tie in order still sorts as if it had been there from the start
  expect(ids(r, 'x:y')).toEqual(['a.one', 'a.two', 'b.one']);
  expect(r.palette().map((p) => `${p.plugin}:${p.id}`)).toEqual(['wisp.a:a.pal']);
  // an unknown plugin: nothing changes
  r.extend('wisp.gone', { extensions: [ext('gone.one', 'x:y')] });
  expect(ids(r, 'x:y')).toEqual(['a.one', 'a.two', 'b.one']);
  r.remove('wisp.a');
  expect(ids(r, 'x:y')).toEqual(['b.one']);
});

test('account_links_and_home_tips_come_from_plugins_and_go_with_them', () => {
  const r = createRegistry();
  const link = { id: 'g.home', label: () => 'wisp Grid', icon: 'cloud' as const, href: () => 'https://grid.example/' };
  const tip = { id: 'g.reach', icon: 'cloud' as const, title: () => 'Reach', body: () => 'b', cta: () => 'Set up', open: () => undefined };
  r.add({ id: 'g', accountLinks: [link], homeTips: [tip] } as PluginUiDef);
  expect(r.accountLinks().map((l) => [l.plugin, l.id])).toEqual([['g', 'g.home']]);
  expect(r.homeTips().map((d) => [d.plugin, d.id])).toEqual([['g', 'g.reach']]);
  r.remove('g');
  expect(r.accountLinks()).toEqual([]);
  expect(r.homeTips()).toEqual([]);
});
