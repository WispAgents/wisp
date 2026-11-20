// ADR-0114: the_picker_offers_a_factory_and_the_launch_sends_it. New task's harness picker ends with a Factory row whose
// pane lists the factories; a pick is `{ agent: 'factory', model: <name> }`, never kept as the launch's harness, and the
// launch sends `factory` with no harness of its own.

import { describe, expect, it } from 'vitest';
import { factoryPane, factoryPick, factoryRow, useFactory, type FactoryRow } from '../src/frame/factory-pick.ts';
import { FACTORY, factoryOf } from '../src/frame/launch-pick.ts';
import { create } from '../src/frame/commands.ts';
import { agentName } from '../src/frame/pickers.ts';
import { factoryReply, runFactory } from '../src/frame/run-factory.ts';

const t = (k: string): string => ({ 'shell.runner.factories': 'Factories', 'shell.runner.factoryBuiltin': 'Built in', 'shell.runner.factory': 'Factory', 'wisp.factory.name': 'Factory' })[k] ?? k;
const rows: FactoryRow[] = [
  { name: 'default', description: 'Spec you approve, …', builtin: true, stations: ['spec', 'build', 'checks', 'review', 'ship'] },
  { name: 'careful', description: '', builtin: false, stations: ['build', 'checks'] },
];

describe('the_picker_offers_a_factory_and_the_launch_sends_it', () => {
  it('lists the factories by name with their stations, the one in use lit', () => {
    const picked: string[] = [];
    const pane = factoryPane(rows, factoryPick('careful'), t, (n) => picked.push(n), () => undefined);
    expect(pane.label).toBe('Factories');
    expect(pane.groups[0]?.items.map((i) => [i.id, i.hint])).toEqual([
      ['default', 'Built in · spec → build → checks → review → ship'],
      ['careful', 'build → checks'],
    ]);
    expect(pane.value).toBe('careful');
    pane.onPick('default');
    expect(picked).toEqual(['default']);
    expect(factoryPane(rows, { agent: 'claude', model: 'opus', effort: '', account: '' }, t, () => undefined, () => undefined).value).toBe('');
    expect(factoryRow('Factory', 'Spec, build, checks, review, ship')).toEqual({ id: FACTORY, name: 'Factory', sub: 'Spec, build, checks, review, ship', icon: 'queue' });
  });

  it('pressing the row uses the factory in use, else default', () => {
    expect(useFactory({ agent: 'claude', model: 'opus', effort: 'high', account: 'a' })).toEqual({ agent: FACTORY, model: 'default', effort: '', account: '' });
    expect(useFactory(factoryPick('careful')).model).toBe('careful');
  });

  it('the launch sends the factory and no harness; the pill says Factory', () => {
    expect(factoryOf(FACTORY, 'careful')).toEqual({ agent: '', factory: 'careful', model: undefined, account: undefined });
    expect(factoryOf(FACTORY, '')).toMatchObject({ agent: '', factory: '' });
    // the create command's wire sends it so, whatever the box picked
    const w = create.wire?.({ peer: 'p', id: 'p:01a1', prompt: 'x', agent: FACTORY, model: 'careful', mode: 'auto', at: 0, account: 'work' }) as Record<string, unknown>;
    expect([w.agent, w.factory, w.model, w.account]).toEqual(['', 'careful', undefined, undefined]);
    expect(factoryOf('claude', 'opus')).toBeUndefined();
    expect(agentName(t as never, [], FACTORY)).toBe('Factory');
  });
});

// the owner's delegate (2026-11-20): a factory run's task says "Factory · <name>" in its header and reply box
describe('a_factory_runs_task_names_the_factory_not_a_harness', () => {
  it('reads the run from the row', () => {
    expect(runFactory({ factory: { name: 'careful' } })).toBe('careful');
    expect(runFactory({ factory: {} })).toBe('default');
    expect(runFactory({ factoryStation: { run: 'r' } } as { factory?: unknown })).toBeUndefined();
    expect(runFactory({})).toBeUndefined();
    expect(runFactory(undefined)).toBeUndefined();
  });
});

// ADR-0114 rule 14: a factory run's reply box has no harness/model chip ("Default"): it says the stations do the work,
// and offers Stop while the run still goes
describe('a_factory_runs_reply_box_shows_no_harness_and_offers_stop_while_it_goes', () => {
  it('reads the run and whether it still goes', () => {
    expect(factoryReply({ factory: { name: 'careful', state: 'running' } })).toEqual({ name: 'careful', live: true });
    expect(factoryReply({ factory: { state: 'waiting' } })).toEqual({ name: 'default', live: true });
    expect(factoryReply({ factory: { name: 'x', state: 'done' } })).toEqual({ name: 'x', live: false });
    expect(factoryReply({ factory: { name: 'x', state: 'stopped' } })?.live).toBe(false);
    expect(factoryReply({ factory: { name: 'x' } })?.live).toBe(true);
    expect(factoryReply({})).toBeUndefined();
  });
});
