// A sandbox that couldn't copy the project showed git's whole output (hundreds of embedded-repo hints) as the
// callout's title, filling the screen. The reason is three lines at most, with Show all / Show less when it's long.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Launching from '../src/Launching.tsx';
import type { LaunchRow } from '../src/launches.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

function mount(error: string): HTMLElement {
  const row: LaunchRow = { id: 'l1', title: 'Fix the build', state: 'failed', step: '', at: 1, agent: 'claude', peer: 'box', error };
  const launch = signal<LaunchRow | undefined>(row);
  const none = signal(undefined);
  const scope = { child: () => scope, dispose: () => undefined };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: (_kind: string, key: string) => (key === 'laptop:l1' ? launch : none) },
    run: async () => ({}),
    navigate: () => undefined,
  };
  const shell = { platform: { openExternal: () => undefined }, toast: { show: () => undefined } };
  const host = document.createElement('div');
  document.body.append(host);
  render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Launching client={client as never} params={{ id: 'l1' } as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  return host;
}

test('a_long_failure_reason_is_three_lines_with_show_all', async () => {
  const error = `couldn't copy the project: ${'target-agents/ (over 16 MB) '.repeat(20)}`;
  const host = mount(error);
  await Promise.resolve();
  const text = host.querySelector<HTMLElement>('[data-part="sandbox.launching.error.reason"]');
  expect(text?.textContent).toBe(error);
  expect(text?.getAttribute('data-lines')).toBe('3');
  const more = host.querySelector<HTMLElement>('[data-part="sandbox.launching.error.more"]');
  expect(more?.textContent).toBe('Show all');
  more?.click();
  expect(text?.getAttribute('data-lines')).toBeNull();
  expect(more?.textContent).toBe('Show less');
});

test('a_short_failure_reason_has_no_show_all', async () => {
  const host = mount('no machine answered');
  await Promise.resolve();
  expect(host.querySelector('[data-part="sandbox.launching.error.reason"]')?.textContent).toBe('no machine answered');
  expect(host.querySelector('[data-part="sandbox.launching.error.more"]')).toBeNull();
});
