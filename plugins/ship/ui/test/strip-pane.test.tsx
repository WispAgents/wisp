// @vitest-environment happy-dom
// round 324: the PR row on the open task hides while the PR pane is open beside it, so the verdict never shows twice,
// and comes back when the pane closes. It read the shell's nav (an alien signal) directly in its JSX, which Solid
// doesn't follow: opening or closing the pane after the task showed left the row as it was.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { createScope, interpret, realTimers, signal, type Nav } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import { t as shipT } from '../src/i18n.ts';
import PrStrip from '../src/PrStrip.tsx';
import '../src/i18n/lazy.ts';

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

function mountStrip(sessionVal: unknown): { host: HTMLDivElement; stop: () => void; dispose: () => void; opened: string[] } {
  const nav = signal<Nav>({ main: { kind: 'task', id: KEY }, side: { kind: 'none' }, overlays: [], layout: 'split' } as unknown as Nav);
  const session = signal<unknown>(sessionVal);
  const none = signal<unknown>(undefined);
  const opened: string[] = [];
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
  const shell = {
    nav,
    client: { nav: { nav } },
    t: shipT,
    toast: { show: () => undefined, error: () => undefined },
    platform: { openExternal: (u: string) => opened.push(u), phone: false },
    navigate: () => undefined,
  };
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
  return { host, stop, dispose: () => client.scope.dispose(), opened };
}

test('merged_multi_pr_card_hides_empty_toolbar_and_shows_summary_with_all_chips', () => {
  const p1 = { number: 839, url: 'https://github.com/o/wisp/pull/839', state: 'MERGED' };
  const p2 = { number: 840, url: 'https://github.com/o/wisp/pull/840', state: 'MERGED' };
  const { host, stop, dispose, opened } = mountStrip({ id: 's1', state: 'done', project: 'wisp', pr: p1, prs: [{ repo: 'wisp', pr: p2 }] });
  expect(host.querySelector('[data-part="pr.strip.row.title"]')?.textContent).toBe('2 PRs');
  expect(host.querySelector('[data-part="pr.strip.row.sub"]')?.textContent).toBe('All merged');
  expect(host.querySelector('[data-part="pr.strip.bar"]'), 'no empty toolbar void when 0 actions').toBeNull();
  const chips = [...host.querySelectorAll('[data-part="pr.strip.chip"]')];
  expect(chips.map((c) => c.textContent?.trim())).toEqual(['wisp #839', 'wisp #840']);
  expect(host.querySelector('[data-part="pr.strip.more"]')).toBeNull();
  (chips[1] as HTMLButtonElement).click();
  expect(opened).toEqual(['https://github.com/o/wisp/pull/840']);
  stop();
  dispose();
});

test('many_attached_prs_collapse_past_five_and_expand_with_worst_hidden_tone', () => {
  const prs = Array.from({ length: 12 }, (_, i) => ({
    repo: `repo-${i + 1}`,
    pr: {
      number: 100 + i,
      url: `https://github.com/o/r${i}/pull/${100 + i}`,
      state: i < 10 ? 'MERGED' : 'OPEN',
      ...(i === 11 ? { checks: { fail: 2, pending: 0, pass: 1 } } : {}),
    },
  }));
  const { host, stop, dispose } = mountStrip({ id: 's1', state: 'done', project: 'repo-1', pr: prs[0]!.pr, prs: prs.slice(1) });
  expect(host.querySelector('[data-part="pr.strip.row.title"]')?.textContent).toBe('12 PRs');
  expect(host.querySelector('[data-part="pr.strip.row.sub"]')?.textContent).toBe('1 failing · 1 open · 10 merged');
  expect(host.querySelectorAll('[data-part="pr.strip.chip"]').length).toBe(4);
  const more = host.querySelector('[data-part="pr.strip.more"]') as HTMLButtonElement | null;
  expect(more?.textContent?.trim()).toBe('+8 more');
  expect(more?.getAttribute('data-tone'), 'hidden failing PR tints +N more red').toBe('bad');
  more?.click();
  expect(host.querySelectorAll('[data-part="pr.strip.chip"]').length).toBe(12);
  expect(host.querySelector('[data-part="pr.strip.more"]')?.textContent?.trim()).toBe('Show less');
  (host.querySelector('[data-part="pr.strip.more"]') as HTMLButtonElement).click();
  expect(host.querySelectorAll('[data-part="pr.strip.chip"]').length).toBe(4);
  stop();
  dispose();
});

test('single_pr_keeps_direct_title_no_chips_and_github_action', () => {
  const { host, stop, dispose } = mountStrip({ id: 's1', state: 'done', project: 'wisp', pr: { number: 840, url: 'https://github.com/o/wisp/pull/840', state: 'MERGED' } });
  expect(host.querySelector('[data-part="pr.strip.row.title"]')?.textContent).toBe('PR #840');
  expect(host.querySelector('[data-part="pr.strip.row.sub"]')?.textContent).toBe('Merged');
  expect(host.querySelector('[data-part="pr.strip.also"]')).toBeNull();
  expect(host.querySelector('[data-part="pr.strip.bar"]')).not.toBeNull();
  stop();
  dispose();
});
