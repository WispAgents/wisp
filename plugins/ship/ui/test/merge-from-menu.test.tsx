// @vitest-environment happy-dom
// Audit C #7: Ship ▾ › "Merge PR #2…" opened nothing. The menu leaves its ask for the open task's PR strip, and only the
// strip's PR row took it: while a ship's stages showed (just after Ship it opened the PR) or the PR pane was open beside
// the task, that row wasn't there and the ask was dropped. The strip takes it whatever it shows: the same "Squash-merge
// PR #2?" confirm as the row's Merge.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { createScope, interpret, realTimers, signal, type Nav } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import PrStrip from '../src/PrStrip.tsx';
import { mergeRequest } from '../src/merge-request.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const KEY = 'laptop:s1';
const PR = { number: 2, url: 'https://github.com/o/r/pull/2', title: 'Fix add', state: 'OPEN', checks: { fail: 0, pending: 0, pass: 2 } };

function mount(row: Record<string, unknown>, side: unknown) {
  const nav = signal<Nav>({ main: { kind: 'task', id: KEY }, side, overlays: [], layout: 'split' } as unknown as Nav);
  const session = signal<unknown>({ id: 's1', state: 'review', pr: PR, project: 'o/r', branch: 'wisp/fix-add', ...row });
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
  return () => {
    stop();
    client.scope.dispose();
  };
}

const confirm = (): Element | null => document.querySelector('[data-part="ship.merge.confirm"]');

test('merge_from_the_menu_asks_with_the_strips_confirm_while_the_pr_pane_is_open_audit_c_7', () => {
  const done = mount({}, { kind: 'pr', id: KEY });
  expect(confirm()).toBeNull();
  mergeRequest(KEY);
  expect(confirm(), 'the confirm opened').not.toBeNull();
  expect(document.body.textContent).toContain('Squash-merge PR #2?');
  expect(mergeRequest(), 'the ask was taken').toBeUndefined();
  done();
});

test('merge_from_the_menu_asks_while_the_ships_stages_show_audit_c_7', () => {
  const done = mount({ shipping: { step: 'push', update: true } }, { kind: 'none' });
  expect(document.querySelector('[data-part="ship.track"]'), 'the stages show').not.toBeNull();
  expect(document.querySelector('[data-part="pr.strip.row"]'), 'not the PR row').toBeNull();
  mergeRequest(KEY);
  expect(confirm(), 'the confirm opened').not.toBeNull();
  done();
});
