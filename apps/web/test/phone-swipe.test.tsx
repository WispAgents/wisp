// @vitest-environment happy-dom
// Audit A #8: on a phone, a task row swiped past the line runs its action. The touch is QA's: pointer events as a
// finger sends them, down, a move past the line, up. Right is Mark done: the row's state follows (done) and the toast
// offers Undo, which puts it back. Left is Save for later: the row moves to For later. Nothing waits on a transition.
import { afterEach, expect, test, vi } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  vi.restoreAllMocks();
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

async function start(kind: 'android' | 'ios') {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const toaster = createToaster();
  const shell = createShell({ client, toaster, platform: nullPlatform({ kind, phone: true, mobile: true }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def?.id === 'wisp.shell-list') shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, root };
}

type Row = { id: string; title: string; state?: string; archived?: boolean; forLater?: unknown; summary?: { forLater?: unknown } };

/** a finger dragging the row from `from` by `by` px, then letting go */
function swipe(row: HTMLElement, from: number, by: number): void {
  vi.spyOn(row, 'getBoundingClientRect').mockReturnValue({ width: 390, height: 72, top: 100, left: 0, right: 390, bottom: 172, x: 0, y: 100, toJSON: () => ({}) } as DOMRect);
  const pe = (type: string, x: number, t: number): boolean =>
    row.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: 130, pointerType: 'touch', isPrimary: true, pointerId: 1, timeStamp: t } as PointerEventInit));
  pe('pointerdown', from, 0);
  for (let i = 1; i <= 10; i++) pe('pointermove', from + (by * i) / 10, i * 40);
  pe('pointerup', from + by, 420);
}

const swipeOf = (root: HTMLElement, title: string): HTMLElement | undefined =>
  [...root.querySelectorAll<HTMLElement>('[data-pane="list"] [data-kit="swipe"]')].find((s) => s.textContent?.includes(title));

for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_a_row_swiped_right_is_marked_done_and_undo_puts_it_back`, async () => {
    const { client, root } = await start(kind);
    const rows = (): readonly Row[] => client.entities.list<Row>('session')();
    await until(() => rows().length > 0 && root.querySelector('[data-pane="list"] [data-kit="swipe"]') !== null);
    // a finished task that isn't done yet: right is Mark done
    const target = rows().find((r) => r.state === 'review' && !r.archived && swipeOf(root, r.title) !== undefined);
    expect(target, 'a row to mark done').toBeDefined();
    const t = target as Row;
    swipe(swipeOf(root, t.title) as HTMLElement, 100, 260);
    const state = (): string | undefined => client.entities.row<Row>('session', t.id as EntityKey)()?.state;
    await until(() => state() === 'done');
    expect(state(), 'the action ran: the row is done').toBe('done');
    // its Undo, as the toolbar's Mark done offers
    const undo = (): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('button')].find((b) => b.textContent?.trim() === 'Undo');
    await until(() => undo() !== undefined);
    undo()?.click();
    await until(() => state() === 'review');
    expect(state(), 'Undo put it back').toBe('review');
  });
}

test('on_android_a_row_swiped_left_is_saved_for_later', async () => {
  const { client, root } = await start('android');
  const rows = (): readonly Row[] => client.entities.list<Row>('session')();
  await until(() => rows().length > 0 && root.querySelector('[data-pane="list"] [data-kit="swipe"]') !== null);
  const later = (r: Row | undefined): boolean => !!(r?.forLater ?? r?.summary?.forLater);
  const target = rows().find((r) => r.state === 'review' && !r.archived && !later(r) && swipeOf(root, r.title) !== undefined) as Row;
  expect(target).toBeDefined();
  swipe(swipeOf(root, target.title) as HTMLElement, 300, -260);
  const row = (): Row | undefined => client.entities.row<Row>('session', target.id as EntityKey)();
  await until(() => later(row()));
  expect(later(row()), 'saved for later').toBe(true);
});
