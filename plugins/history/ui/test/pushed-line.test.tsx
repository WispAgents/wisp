// @vitest-environment happy-dom
// final audit F2/F3: the line under the task's last answer says where a push went after a reload (from the task's
// slot), a ⋯ → Push to repo toasts it with its PR, and a save after a push offers Push again.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { signal, type EntityKey } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import type { ExtensionProps } from '@wisp/shell/contract';
import { KitProvider } from '@wisp/ui';
import OfferFoot from '../src/OfferFoot.tsx';
import later from '../src/later.ts';
import { runPush, runSave, type Pushed, type Tracked } from '../src/save.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

async function until(ok: () => boolean, ms = 5_000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

/** OfferFoot on task `id`'s one turn, its row holding `history` as its `wisp.history` slot. */
function mount(id: string, history: unknown, run: (cmd: unknown, a: unknown) => Promise<unknown> = async () => undefined) {
  const row = { id: `local:${id}`, agent: 'claude', body: { ext: { 'wisp.history': history } } };
  const client = {
    home: 'local',
    entities: { list: () => signal([{ id: `local:t-${id}`, body: { id: `t-${id}`, idx: 0, sessionId: id } }]) },
    run,
    navigate: () => undefined,
  };
  const toast = { success: vi.fn(), show: vi.fn(), error: vi.fn() };
  const openExternal = vi.fn();
  const shell = { toast, platform: { openExternal } };
  const props = { client: client as never, config: {}, session: () => row, id: `local:t-${id}` } satisfies ExtensionProps;
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <OfferFoot {...props} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  const part = (name: string): HTMLElement | null => host.querySelector(`[data-part="${name}"]`);
  return { host, part, toast, openExternal, props, stop };
}

test('after_a_reload_the_answer_says_where_the_push_went_with_its_pr', async () => {
  const m = mount('reload', { tracked: { files: ['a', 'b'] }, pushed: { branch: 'wisp/history-x', commit: 'c', pr: 'https://github.com/a/b/pull/1' } });
  await until(() => m.part('history.offer.pushed') !== null);
  expect(m.part('history.offer.pushed')?.textContent).toBe('Pushed to wisp/history-x, pull request open');
  const open = m.part('history.offer.openPr');
  expect(open?.textContent).toContain('Open PR');
  open?.click();
  expect(m.openExternal).toHaveBeenCalledWith('https://github.com/a/b/pull/1');
  expect(m.part('history.offer.push'), 'pushed: no Push').toBeNull();
  m.stop();
});

test('push_to_repo_from_the_menu_toasts_the_branch_and_its_pr', async () => {
  const pushed: Pushed = { branch: 'wisp/history-m', commit: 'c', files: ['a'], pr: 'https://github.com/a/b/pull/3' };
  const m = mount('menu', { tracked: { files: ['a'] } }, async () => pushed);
  const rowPush = later.extensions?.find((x) => x.id === 'wisp.history.rowPush');
  rowPush?.bar?.select?.(m.props as never);
  await until(() => m.toast.success.mock.calls.length > 0);
  expect(m.toast.success).toHaveBeenCalledTimes(1);
  const [msg, opts] = m.toast.success.mock.calls[0] as [string, { action: { label: string; onClick: () => void } }];
  expect(msg).toBe('Pushed to wisp/history-m, pull request open');
  expect(opts.action.label).toBe('Open PR');
  opts.action.onClick();
  expect(m.openExternal).toHaveBeenCalledWith('https://github.com/a/b/pull/3');
  expect(m.part('history.offer.pushed')?.textContent).toBe('Pushed to wisp/history-m, pull request open');
  m.stop();
});

test('a_save_after_a_push_shows_push_again_and_no_open_pr', async () => {
  const key = 'local:again' as EntityKey;
  const saved: Tracked = { repo: '/r', store: 'files', files: ['a', 'b'], commits: [], scrubbed: 0 };
  // the slot remembers the earlier push, as the server leaves it
  const m = mount('again', { tracked: { files: ['a', 'b'] }, pushed: { branch: 'wisp/history-a', commit: 'c', pr: 'https://x/pull/9' } });
  await runSave(async () => saved, key);
  await runPush(async () => ({ branch: 'wisp/history-a', commit: 'c', files: [], pr: 'https://x/pull/9' }), key);
  await until(() => m.part('history.offer.openPr') !== null);
  await runSave(async () => saved, key);
  await until(() => m.part('history.offer.push') !== null);
  expect(m.part('history.offer.push'), 'the newer save offers Push').not.toBeNull();
  expect(m.part('history.offer.openPr'), 'not the old PR').toBeNull();
  m.stop();
});
