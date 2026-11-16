// @vitest-environment happy-dom
// round 324: the PR row on the open task hides while the PR pane is open beside it, so the verdict never shows twice,
// and comes back when the pane closes. It read the shell's nav (an alien signal) directly in its JSX, which Solid
// doesn't follow: opening or closing the pane after the task showed left the row as it was.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { createScope, interpret, realTimers, signal, type Nav } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import PrStrip from '../src/PrStrip.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

const KEY = 'laptop:s1';
const PR = { number: 42, url: 'https://github.com/o/r/pull/42', title: 'Fix add', state: 'OPEN', checks: { fail: 0, pending: 0, pass: 2 } };

test('the_pr_row_hides_when_the_pr_pane_opens_beside_the_task_and_comes_back_when_it_closes', () => {
  const nav = signal<Nav>({ main: { kind: 'task', id: KEY }, side: { kind: 'none' }, overlays: [], layout: 'split' } as unknown as Nav);
  const session = signal<unknown>({ id: 's1', state: 'done', pr: PR, branch: 'wisp/fix-add' });
  const none = signal<unknown>(undefined);
  const client = {
    id: 'wisp.ship',
    home: 'laptop',
    scope: createScope('test', realTimers),
    machine: (def: never, ctx: never, owner?: never) => interpret(def, owner ?? client.scope, ctx),
    entities: { row: (kind: string) => (kind === 'session' ? session : none), list: () => signal([]), peek: () => undefined },
    run: async () => null,
    navigate: () => undefined,
    watch: () => undefined,
  };
  const shell = { nav, client: { nav: { nav } }, t: (k: string) => k, toast: { show: () => undefined, error: () => undefined }, platform: { openExternal: () => undefined, phone: false }, navigate: () => undefined };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <PrStrip client={client as never} config={undefined as never} id={KEY} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  const strip = (): Element | null => host.querySelector('[data-part="pr.strip"]');
  expect(strip(), 'the PR row on the task').not.toBeNull();
  nav({ ...nav(), side: { kind: 'pr', id: KEY } } as unknown as Nav);
  expect(strip(), 'the PR pane opened beside it: the row hides').toBeNull();
  nav({ ...nav(), side: { kind: 'none' } } as unknown as Nav);
  expect(strip(), 'the pane closed: the row is back').not.toBeNull();
  stop();
  client.scope.dispose();
});
