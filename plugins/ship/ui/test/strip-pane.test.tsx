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

// audit E5: a review by another harness that failed read "⚠ Not reviewed", titled "Not reviewed", like one never run; the
// why and Click to try again (v1 reviewState's title) were in the model but never reached the strip. A rerun the server
// didn't start ("why") said nothing.
test('a_failed_review_says_why_and_a_rerun_that_didnt_start_says_so_audit_e5', async () => {
  const nav = signal<Nav>({ main: { kind: 'task', id: KEY }, side: { kind: 'none' }, overlays: [], layout: 'split' } as unknown as Nav);
  const review = { status: 'failed', current: true, why: 'Claude Code answered, but not with a review wisp could read. Review again to ask once more.', findings: [], by: { agent: 'claude' } };
  const session = signal<unknown>({ id: 's1', state: 'done', pr: { ...PR, head: 'abc' }, branch: 'wisp/fix-add', review });
  const none = signal<unknown>(undefined);
  const said: string[] = [];
  const ran: string[] = [];
  const client = {
    id: 'wisp.ship',
    home: 'laptop',
    scope: createScope('test', realTimers),
    machine: (def: never, ctx: never, owner?: never) => interpret(def, owner ?? client.scope, ctx),
    entities: { row: (kind: string) => (kind === 'session' ? session : none), list: () => signal([]), peek: () => undefined },
    run: async (cmd: { id: string }) => (ran.push(cmd.id), cmd.id === 'review.maybe' ? { why: 'No other harness is set up to review.' } : null),
    navigate: () => undefined,
    watch: () => undefined,
  };
  const t = (k: string, v?: Record<string, unknown>): string => (v ? `${k}${JSON.stringify(v)}` : k);
  const shell = { nav, client: { nav: { nav } }, t, toast: { show: (m: string) => (said.push(m), 'x'), error: () => undefined }, platform: { openExternal: () => undefined, phone: false }, navigate: () => undefined };
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
  const chip = host.querySelector('[data-part="pr.strip.review"]') as HTMLButtonElement | null;
  expect(chip, 'the review chip').not.toBeNull();
  expect(chip?.getAttribute('title') ?? chip?.getAttribute('aria-label') ?? '').toContain('Claude Code answered, but not with a review');
  expect(chip?.getAttribute('title') ?? '').toContain('ask once more. Click to try again');
  expect(chip?.getAttribute('title') ?? '').not.toContain('..');
  chip?.click();
  await new Promise((r) => setTimeout(r, 0));
  await new Promise((r) => setTimeout(r, 0));
  expect(ran).toContain('review.maybe');
  expect(said).toEqual(['No other harness is set up to review.']);
  stop();
  client.scope.dispose();
});
