// UI QA r1, Settings → Computers → Sandboxes: the trash dropped a sandbox at once, with no question, and what wasn't
// brought back from it went too; and while sandbox.info was on its way, or when it failed, the pane said "Not signed
// in", "None yet" and "None now" as if nothing were set up. Delete asks first (naming the sandbox and whose it is);
// the pane shows a skeleton until it knows, and why it couldn't know, with Try again.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Sandboxes from '../src/Sandboxes.tsx';
import type { Info } from '../src/commands.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const INFO: Info = {
  enabled: true,
  logins: ['claude'],
  env: [],
  sandboxes: [{ machine: 'm1', peer: 'box1', sandbox: 's1', name: 'brave-otter', owner: { task: 't1' }, state: 'running' }],
};

type Run = (id: string, args: Record<string, unknown>) => Promise<unknown>;

function mount(run: Run): { host: HTMLElement; calls: { id: string; args: Record<string, unknown> }[]; stop: () => void } {
  const calls: { id: string; args: Record<string, unknown> }[] = [];
  const none = signal(undefined);
  const scope = { child: () => scope, dispose: () => undefined };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: () => none, list: () => signal([]) },
    run: (cmd: { id: string }, args: Record<string, unknown>) => {
      calls.push({ id: cmd.id, args });
      return run(cmd.id, args);
    },
    navigate: () => undefined,
  };
  const shell = { platform: { openExternal: () => undefined }, toast: { show: () => undefined } };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Sandboxes client={client as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  return { host, calls, stop };
}

const until = async (ok: () => boolean, ms = 2000): Promise<void> => {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
};
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const rows = (): number => document.querySelectorAll('[data-part="sandbox.row"]').length;
const emptyRows = (): number => document.querySelectorAll('[data-part="sandbox.list.none"], [data-part="sandbox.env.none"], [data-part="sandbox.claude"]').length;
const drops = (calls: { id: string }[]): number => calls.filter((c) => c.id === 'sandbox.drop').length;

test('delete_asks_first_naming_the_sandbox_and_cancel_keeps_it', async () => {
  const { calls, stop } = mount(async (id) => (id === 'sandbox.info' ? INFO : {}));
  await until(() => rows() === 1);
  q('[data-part="sandbox.row"] [aria-label="Delete brave-otter"]')?.click();
  await until(() => q('[data-part="sandbox.confirm"]') !== null);
  const dialog = q('[data-part="sandbox.confirm"]');
  expect(dialog, 'a confirm opens').not.toBeNull();
  expect(dialog?.textContent).toContain('brave-otter');
  expect(dialog?.textContent).toContain("A task's (/sandbox)");
  expect(drops(calls), 'nothing dropped yet').toBe(0);
  q('[data-part="sandbox.confirm.cancel"]')?.click();
  await until(() => q('[data-part="sandbox.confirm"]') === null);
  expect(q('[data-part="sandbox.confirm"]')).toBeNull();
  expect(rows(), 'still listed').toBe(1);
  expect(drops(calls)).toBe(0);
  stop();
});

test('delete_drops_only_on_yes', async () => {
  const { calls, stop } = mount(async (id) => (id === 'sandbox.info' ? INFO : {}));
  await until(() => rows() === 1);
  q('[data-part="sandbox.row"] [aria-label="Delete brave-otter"]')?.click();
  await until(() => q('[data-part="sandbox.confirm.ok"]') !== null);
  expect(q('[data-part="sandbox.confirm.ok"]')?.textContent).toBe('Delete');
  q('[data-part="sandbox.confirm.ok"]')?.click();
  await until(() => rows() === 0);
  expect(calls.filter((c) => c.id === 'sandbox.drop').map((c) => c.args['machine'])).toEqual(['m1']);
  expect(rows(), 'its row goes').toBe(0);
  stop();
});

test('while_sandbox_info_is_on_its_way_the_pane_shows_a_skeleton_not_empty_rows', async () => {
  const { stop } = mount((id) => (id === 'sandbox.info' ? new Promise(() => undefined) : Promise.resolve({})));
  await new Promise((r) => setTimeout(r, 0));
  expect(q('[data-part="sandbox.loading"]')).not.toBeNull();
  expect(emptyRows()).toBe(0);
  expect(document.body.textContent).not.toContain('Not signed in');
  stop();
});

test('sandbox_info_failing_says_why_and_try_again_brings_the_list', async () => {
  let fail = true;
  const { calls, stop } = mount(async (id) => {
    if (id !== 'sandbox.info') return {};
    if (fail) throw new Error('grid unreachable');
    return INFO;
  });
  await until(() => q('[data-part="sandbox.info.error"]') !== null);
  expect(q('[data-part="sandbox.info.error"]')?.textContent).toContain('grid unreachable');
  expect(emptyRows(), 'no "none" rows, no signed-out Sign in').toBe(0);
  expect(q('[data-part="sandbox.loading"]')).toBeNull();
  fail = false;
  q('[data-part="sandbox.info.retry"]')?.click();
  await until(() => rows() === 1);
  expect(calls.filter((c) => c.id === 'sandbox.info').length).toBe(2);
  expect(q('[data-part="sandbox.info.error"]')).toBeNull();
  expect(rows()).toBe(1);
  stop();
});
