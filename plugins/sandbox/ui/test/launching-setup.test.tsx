// UI QA r1: a launch whose `.wisp/sandbox.sh` failed showed all of what it said (up to 4000 chars) at full height, so
// on a phone Open the task sat screens below the fold and the error (at the end) was the last thing reached. The log
// is a dozen lines tall, scrolled to its end, and Open the task stays under it.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Launching from '../src/Launching.tsx';
import type { LaunchRow } from '../src/launches.ts';

const restore: (() => void)[] = [];
afterEach(() => {
  for (const r of restore.splice(0)) r();
  document.body.innerHTML = '';
});

/** jsdom lays nothing out: a pre's scroll metrics, stubbed, with what was set kept per element */
function stubScroll(height: number): WeakMap<Element, number> {
  const set = new WeakMap<Element, number>();
  const proto = HTMLElement.prototype;
  const had = { h: Object.getOwnPropertyDescriptor(proto, 'scrollHeight'), t: Object.getOwnPropertyDescriptor(proto, 'scrollTop') };
  Object.defineProperty(proto, 'scrollHeight', { configurable: true, get: () => height });
  Object.defineProperty(proto, 'scrollTop', {
    configurable: true,
    get(this: Element) {
      return set.get(this) ?? 0;
    },
    set(this: Element, v: number) {
      set.set(this, v);
    },
  });
  restore.push(() => {
    for (const [k, d] of [['scrollHeight', had.h], ['scrollTop', had.t]] as const) {
      if (d) Object.defineProperty(proto, k, d);
      else delete (proto as unknown as Record<string, unknown>)[k];
    }
  });
  return set;
}

test('a_failed_setups_log_is_capped_scrolled_to_its_end_and_open_the_task_is_there', async () => {
  const scrolled = stubScroll(2400);
  const tail = Array.from({ length: 120 }, (_, i) => `step ${i}: installing a package that takes a line of its own`).join('\n').slice(0, 4000);
  const row: LaunchRow = { id: 'l1', title: 'Fix the build', state: 'started', step: '', at: 1, agent: 'claude', peer: 'box', task: 't1', setup: { ok: false, tail } };
  const launch = signal<LaunchRow | undefined>(row);
  const none = signal(undefined);
  const went: unknown[] = [];
  const scope = { child: () => scope, dispose: () => undefined };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: (_kind: string, key: string) => (key === 'laptop:l1' ? launch : none) },
    run: async () => ({}),
    navigate: (i: unknown) => went.push(i),
  };
  const shell = { platform: { openExternal: () => undefined }, toast: { show: () => undefined } };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Launching client={client as never} params={{ id: 'l1' } as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  await Promise.resolve();
  const pre = host.querySelector<HTMLElement>('[data-part="sandbox.launching.setup.tail"] [data-kit="code-pre"]');
  expect(pre, 'the log shows').not.toBeNull();
  expect(pre?.textContent).toBe(tail);
  // capped: its line count, which the kit's sheet turns into a max height with overflow auto
  expect(pre?.getAttribute('data-lines')).toBe('12');
  expect(pre?.style.getPropertyValue('--k-lines')).toBe('12');
  expect(scrolled.get(pre!), 'scrolled to the end, where the error is').toBe(2400);
  const open = host.querySelector<HTMLElement>('[data-part="sandbox.launching.open"]');
  expect(open?.textContent).toBe('Open its task');
  open?.click();
  expect(went).toEqual([{ t: 'openTask', id: 'box:t1' }]);
  stop();
});
