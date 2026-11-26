// Settings' search opening Agents on another harness while Agents is open: the frame keeps the pane (samePane), so
// only its `filter` prop changes, and the pane read it once, as it mounted: the detail stayed on the first harness.
// It follows the filter now; an empty one keeps the pick.
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Agents from '../src/panes/Agents.tsx';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
  document.body.innerHTML = '';
});

test('agents_follows_a_new_filter_while_open_and_an_empty_one_keeps_the_pick', () => {
  const scope = { child: () => scope, add: () => undefined, dispose: () => undefined, interval: () => () => undefined, timers: { now: () => 0 } };
  const client = {
    id: 'wisp.accounts',
    home: 'laptop',
    homeNow: () => 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: () => signal(undefined), list: () => signal([]), keys: () => signal([]), peek: () => undefined },
    defineStore: () => ({ value: signal({}), set: () => undefined }),
    run: () => new Promise(() => undefined),
    navigate: () => undefined,
    peers: { list: () => [], name: () => 'laptop', status: () => signal('online'), info: () => signal(undefined), hasFeature: () => false },
    t: (k: string) => k,
  };
  const marks = signal([
    { id: 'claude', name: 'Claude Code', order: 0 },
    { id: 'codex', name: 'Codex', order: 1 },
  ]);
  const shell = { harnesses: () => marks, toast: { show: () => undefined }, client, platform: {}, extensions: () => signal([]), t: (k: string) => k };
  const [filter, setFilter] = createSignal<string | undefined>('claude');
  const host = document.createElement('div');
  document.body.append(host);
  stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Agents client={client as never} {...(filter() !== undefined && { filter: filter() as string })} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  const selected = (): string | null | undefined => host.querySelector('[data-kit="settings-master-item"][aria-selected="true"]')?.getAttribute('data-id');
  expect(selected()).toBe('claude');
  setFilter('codex'); // the same pane, a new filter: no remount
  expect(selected()).toBe('codex');
  setFilter(undefined);
  expect(selected(), 'no filter: the pick stays').toBe('codex');
  setFilter('');
  expect(selected(), 'an empty filter: the pick stays').toBe('codex');
});
