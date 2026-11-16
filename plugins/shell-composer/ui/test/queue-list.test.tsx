// The queued follow-ups inside the reply box (v1 queued.js, composer.css .c-queued .cq): one row each on the kit's
// QueuedRow, keyed so a reorder moves the rows; Send now, Combine, the grip, Move up / down and × still act.
import { afterEach, beforeAll, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { preloadControls } from '@wisp/ui';
import ui from '../src/index.ts';
import { QueueList, type QueueActions } from '../src/ui/QueueList.tsx';
import type { Env } from '../src/env.ts';
import type { QueueItem, Row } from '../src/types.ts';

// the row menu's machine is a lazy chunk (lazy-part.ts): loaded before the tests, as the app does once the first screen
// is up, so a right-click's wait is the menu opening, not the chunk's first compile on a busy runner (B23)
beforeAll(() => preloadControls());

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

const env = { t: (k: string, v?: Record<string, unknown>) => (v ? `${k} ${JSON.stringify(v)}` : k) } as unknown as Env;
const items = (...ids: string[]): Row<QueueItem>[] => ids.map((id, i) => ({ id, text: `say ${id}`, at: i }));

function mount(queue: () => readonly Row<QueueItem>[], held = false) {
  const calls: [string, unknown][] = [];
  const actions: QueueActions = {
    now: (id) => calls.push(['now', id]),
    edit: (id, text) => calls.push(['edit', [id, text]]),
    merge: (ids) => calls.push(['merge', ids]),
    order: (ids) => calls.push(['order', ids]),
    cancel: (id) => calls.push(['cancel', id]),
  };
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <QueueList env={env} queue={queue()} held={held} running name="Claude Code" actions={actions} confirm={async () => true} refocus={() => undefined} />, host);
  const rows = (): HTMLElement[] => [...host.querySelectorAll<HTMLElement>('[data-kit="cq-row"]')];
  return { host, calls, rows };
}

// v1 composer.js `keyedRows(list, [...queuedItems(s), ...followUpItems(s)])`: one list inside the box, the queued first
// (apps/web/test/plugins.test.tsx checks it lands before wisp.later's in a real shell)
test('the_queue_sits_inside_the_reply_box_over_its_text_first_v1_c_queued', () => {
  const queue = ui.extensions?.find((e) => e.id === 'shell-composer.queue');
  expect(queue?.attachTo.id).toBe('shell.task:composer-top');
  expect(queue?.order).toBe(10);
  // nothing queued stays on the box's foot
  expect(ui.extensions?.some((e) => e.attachTo.id === 'shell.task:foot')).toBe(false);
});

// v1 .cq: what it waits for (Next · after X finishes / Queued #2), what it says, × always; two or more: a grip each
test('each_queued_row_says_what_it_waits_for_and_what_it_says_with_its_cancel_and_a_grip_when_two_v1_cq', () => {
  const { rows } = mount(() => items('a', 'b'));
  expect(rows()).toHaveLength(2);
  const [a, b] = rows();
  expect(a?.dataset['part']).toBe('composer.queue.row');
  expect(a?.querySelector('[data-kit="cq-tag"]')?.textContent).toBe('shell-composer.queue.next {"name":"Claude Code"}');
  expect(b?.querySelector('[data-kit="cq-tag"]')?.textContent).toBe('shell-composer.queue.nth {"n":2}');
  expect(a?.querySelector('[data-kit="cq-text"]')?.textContent).toBe('say a');
  expect(a?.querySelector('[data-part="composer.queue.grip"]')).not.toBeNull();
  expect(a?.hasAttribute('data-sortable')).toBe(true);
  expect(a?.querySelector('[data-kit="cq-x"]')).not.toBeNull();
  // one queued: no grip, nothing to move
  dispose?.();
  const one = mount(() => items('a'));
  expect(one.rows()[0]?.querySelector('[data-kit="cq-grip"]')).toBeNull();
});

test('send_now_combine_and_cancel_on_a_row_act_on_that_follow_up_v1_cq_acts', async () => {
  const { rows, calls } = mount(() => items('a', 'b'));
  const a = rows()[0];
  (a?.querySelector('[data-part="composer.queue.now"]') as HTMLElement).click();
  (a?.querySelector('[data-part="composer.queue.merge"]') as HTMLElement).click();
  // the last one has no next to combine with
  expect(rows()[1]?.querySelector('[data-part="composer.queue.merge"]')).toBeNull();
  (rows()[1]?.querySelector('[data-kit="cq-x"]') as HTMLElement).click();
  await Promise.resolve();
  await Promise.resolve();
  expect(calls).toEqual([
    ['now', 'a'],
    ['merge', ['a', 'b']],
    ['cancel', 'b'],
  ]);
});

// v1 dragRow / Move up, Move down: the grip moves a row; the new order goes out and the rows move, the same nodes
test('the_grip_reorders_and_a_reorder_moves_the_same_row_nodes_v1_queue_moves', () => {
  const [queue, setQueue] = createSignal(items('a', 'b', 'c'));
  const { rows, calls } = mount(queue);
  const [a, b, c] = rows();
  rows()[2]?.querySelector<HTMLElement>('[data-kit="cq-grip"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  expect(calls).toEqual([['order', ['a', 'c', 'b']]]);
  setQueue(items('c', 'a', 'b'));
  expect(rows()).toEqual([c, a, b]);
  expect(b).toBeDefined();
});

// v1 queuedMenu: right-click is the row's menu, with the same moves as the grip
test('a_right_click_opens_the_rows_menu_with_move_and_combine_all_v1_queued_menu', async () => {
  const { rows, host } = mount(() => items('a', 'b', 'c'));
  rows()[1]?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
  // the menu's machine loads on the first right-click, then the menu opens where it was asked
  const entries = await vi.waitFor(() => {
    const found = [...document.querySelectorAll('[data-part="composer.queue.menu.item"]')];
    if (!found.length) throw new Error('menu not open yet');
    return found;
  });
  const labels = entries.map((m) => m.textContent?.trim());
  expect(labels).toContain('shell-composer.queue.up');
  expect(labels).toContain('shell-composer.queue.down');
  expect(labels).toContain('shell-composer.queue.merge-all {"count":3}');
  expect(labels).toContain('shell-composer.queue.cancel-menu');
  expect(host).toBeDefined();
});

// v1 queuedMenu: a right-click on a row's × or Send now is that row's menu; in the box being typed in, and between the
// rows, the browser's own (no stale menu of the row asked last)
test('a_right_click_is_the_rows_own_menu_and_the_browsers_in_the_edit_box_or_between_rows_v1_queued_menu', async () => {
  const { rows, host } = mount(() => items('a', 'b'));
  const ask = (el: Element | null | undefined): MouseEvent => {
    const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 5, clientY: 5 });
    el?.dispatchEvent(e);
    return e;
  };
  expect(ask(host.querySelector('[data-kit="cq-list"]')).defaultPrevented).toBe(false);
  expect(ask(rows()[1]?.querySelector('[data-kit="cq-x"]')).defaultPrevented).toBe(true);
  const entries = await vi.waitFor(() => {
    const found = [...document.querySelectorAll('[data-part="composer.queue.menu.item"]')];
    if (!found.length) throw new Error('menu not open yet');
    return found.map((m) => m.textContent?.trim());
  });
  // the second row's: it can move up, not down
  expect(entries).toContain('shell-composer.queue.up');
  expect(entries).not.toContain('shell-composer.queue.down');
  // edit the first in place: its box keeps the browser's menu
  rows()[0]?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  const box = await vi.waitFor(() => {
    const ta = rows()[0]?.querySelector('[data-kit="cq-editbox"] textarea');
    if (!ta) throw new Error('not editing yet');
    return ta;
  });
  expect(ask(box).defaultPrevented).toBe(false);
});
