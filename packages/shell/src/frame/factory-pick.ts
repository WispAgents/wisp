// New task's Factory row (ADR-0114): the harness picker's left list ends with **Factory** ("Spec, build, checks, review,
// ship"), and its pane lists the factories a task can run through by name, the built-in `default` first. Picking one is
// the pick `{ agent: 'factory', model: <name> }`; the launch sends `factory` instead of a harness (launch-pick.ts
// `factoryOf`). Loaded with the picker, never on the first screen.

import { defineCommand } from '@wisp/client';
import type { HarnessPickerPane, HarnessPickerRow } from '@wisp/ui';
import { FACTORY } from './launch-pick.ts';
import type { Runner } from './runner.ts';
import '../i18n/lazy.ts';

/** `factory.names`'s rows (plugins/factory/api/bindings/factory/FactoryRow.ts). */
export interface FactoryRow {
  readonly name: string;
  readonly description: string;
  readonly builtin: boolean;
  readonly stations: readonly string[];
}

/** Every factory the peer can run a task through; none when it has no wisp.factory. */
export const factoryNames = defineCommand<{ peer: string }, FactoryRow[]>({ id: 'factory.names', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the factories' });

/** The left list's Factory row. */
export const factoryRow = (name: string, sub: string): HarnessPickerRow => ({ id: FACTORY, name, sub, icon: 'queue' });

/** The pick a factory makes. */
export const factoryPick = (name: string): Runner => ({ agent: FACTORY, model: name, effort: '', account: '' });

/** The Factory row pressed: the factory in use, else `default`. */
export const useFactory = (was: Runner): Runner => factoryPick(was.agent === FACTORY && was.model ? was.model : 'default');

type Words = (key: string, vars?: Record<string, string | number>) => string;

/** The pane: the factories by name, each with its stations (or what it says it does), the one in use lit. */
export function factoryPane(rows: readonly FactoryRow[], was: Runner, t: Words, onPick: (name: string) => void, onHow: () => void): HarnessPickerPane {
  return {
    label: t('shell.runner.factories'),
    groups: [
      {
        head: t('shell.runner.factories'),
        items: rows.map((r) => ({ id: r.name, label: r.name, hint: r.builtin ? `${t('shell.runner.factoryBuiltin')} · ${r.stations.join(' → ')}` : r.description || r.stations.join(' → ') })),
      },
    ],
    value: was.agent === FACTORY ? was.model || 'default' : '',
    onPick,
    empty: t('shell.runner.noFactories'),
    note: { text: t('shell.runner.factoryNote'), link: t('shell.runner.factoryHow'), onLink: onHow },
  };
}
