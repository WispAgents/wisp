// @vitest-environment happy-dom
// Merging a PR the base branch's rules hold (required checks or reviews) toasted gh's own advice ("add the `--admin`
// flag"), a flag nobody types in wisp. The refusal (`merge_blocked`) now opens a confirm: Bypass and merge runs the
// merge again with `admin`, Wait for requirements just closes.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { CommandError, createScope, interpret, realTimers, signal, type Nav } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import PrStrip from '../src/PrStrip.tsx';
import { mergeRequest } from '../src/merge-request.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const KEY = 'laptop:s2';
const PR = { number: 1064, url: 'https://github.com/o/r/pull/1064', title: 'Fix add', state: 'OPEN', checks: { fail: 0, pending: 0, pass: 2 } };
const HELD = "PR #1064 can't merge yet: the base branch's rules (required checks or reviews) aren't met.";

function mount() {
  const nav = signal<Nav>({ main: { kind: 'task', id: KEY }, side: { kind: 'none' }, overlays: [], layout: 'split' } as unknown as Nav);
  const session = signal<unknown>({ id: 's2', state: 'review', pr: PR, project: 'o/r', branch: 'wisp/fix-add' });
  const none = signal<unknown>(undefined);
  const runs: Record<string, unknown>[] = [];
  const errors: string[] = [];
  const client = {
    id: 'wisp.ship',
    home: 'laptop',
    scope: createScope('test', realTimers),
    machine: (def: never, ctx: never, owner?: never) => interpret(def, owner ?? client.scope, ctx),
    entities: { row: (kind: string) => (kind === 'session' ? session : none), list: () => signal([]), peek: () => undefined },
    run: async (_cmd: unknown, args: Record<string, unknown>) => {
      runs.push(args);
      if (!args['admin']) throw new CommandError({ code: 'forbidden', message: HELD, reason: 'merge_blocked' });
      return { pr: { ...PR, state: 'MERGED' }, sync: { ok: true, text: '' } };
    },
    navigate: () => undefined,
    watch: () => undefined,
  };
  const dismissed: string[] = [];
  const toast = { show: (m: string) => m, success: () => undefined, error: (m: string) => errors.push(m), dismiss: (id: string) => dismissed.push(id) };
  const shell = { nav, client: { nav: { nav } }, t: (k: string) => k, toast, platform: { openExternal: () => undefined, phone: false }, navigate: () => undefined };
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
  const done = (): void => {
    stop();
    client.scope.dispose();
  };
  return { runs, errors, dismissed, done };
}

const part = (p: string): HTMLElement | null => document.querySelector(`[data-part="${p}"]`);
const settle = async (): Promise<void> => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
};
async function held(): Promise<void> {
  mergeRequest(KEY);
  part('ship.merge.go')?.click();
  await settle();
}

test('a_merge_the_branch_rules_hold_asks_to_bypass_and_bypass_merges_with_admin', async () => {
  const m = mount();
  await held();
  expect(part('ship.merge.blocked'), 'the bypass confirm opened').not.toBeNull();
  expect(document.body.textContent).toContain(HELD);
  expect(m.errors, 'no toast of gh text').toEqual([]);
  expect(m.dismissed, 'the Merging… toast went').toEqual(['Merging PR #1064…']);
  part('ship.merge.bypass')?.click();
  await settle();
  expect(m.runs.map((r) => r['admin'])).toEqual([false, true]);
  expect(part('ship.merge.blocked'), 'closed once merged').toBeNull();
  m.done();
});

test('wait_for_requirements_closes_without_merging', async () => {
  const m = mount();
  await held();
  part('ship.merge.wait')?.click();
  await settle();
  expect(part('ship.merge.blocked')).toBeNull();
  expect(m.runs).toHaveLength(1);
  m.done();
});
