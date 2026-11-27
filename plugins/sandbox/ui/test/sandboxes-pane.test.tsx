// @vitest-environment happy-dom
// Settings → Computers → Sandboxes, sandbox audit r5: the pane listed only this computer's sandboxes (one made
// elsewhere held a slot nobody saw), never changed while open, said Codex's sign-in in a 4 s toast (the first click
// nothing at all), and dropped every refusal (an env var's, Sign out's) after clearing what was typed.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { createScope, signal, type Timers } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Sandboxes from '../src/Sandboxes.tsx';
import type { Info, LoginAnswer } from '../src/commands.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

/** until `ok`, checked after each macrotask; gives up after `ms` by the clock (the assertion after it then fails) */
async function until(ok: () => boolean, ms = 15_000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

/** The pane's timers, run by hand (a refetch's debounce, Codex's poll). */
function handTimers(): Timers & { run: () => void } {
  const q: { fn: () => void }[] = [];
  return {
    now: () => 0,
    setTimeout: (fn) => {
      const h = { fn };
      q.push(h);
      return h;
    },
    clearTimeout: (h) => {
      const i = q.indexOf(h as { fn: () => void });
      if (i >= 0) q.splice(i, 1);
    },
    run: () => {
      for (const h of q.splice(0)) h.fn();
    },
  };
}

type Run = (id: string, args: Record<string, unknown>) => Promise<unknown>;

function mount(run: Run) {
  const timers = handTimers();
  const lists = {
    machines_peer: signal<unknown[]>([]),
    machines_status: signal<unknown[]>([]),
    'sandbox.launch': signal<unknown[]>([]),
    crew_member: signal<unknown[]>([]),
  };
  const calls: { id: string; args: Record<string, unknown> }[] = [];
  const client = {
    home: 'laptop',
    scope: createScope('test', timers),
    watch: () => undefined,
    navigate: () => undefined,
    entities: { row: () => signal(undefined), list: (kind: keyof typeof lists) => lists[kind] ?? signal([]) },
    run: (cmd: { id: string }, args: Record<string, unknown>) => {
      calls.push({ id: cmd.id, args });
      return run(cmd.id, args);
    },
  };
  const shell = { platform: { openExternal: vi.fn(), copy: vi.fn(async () => true), onResume: () => () => undefined }, toast: { show: vi.fn() } };
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
  const rows = (): string[] => [...host.querySelectorAll('[data-part="sandbox.row"]')].map((r) => r.textContent ?? '');
  const button = (text: string, within: ParentNode = host): HTMLButtonElement | undefined =>
    [...within.querySelectorAll('button')].find((b) => b.textContent?.trim() === text) as HTMLButtonElement | undefined;
  const type = (label: string, v: string): void => {
    const el = host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`)!;
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  };
  const input = (label: string): string => host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`)?.value ?? '';
  return { host, lists, calls, shell, timers, stop, rows, button, type, input };
}

const base: Info = { enabled: true, logins: ['claude'], env: [], sandboxes: [] };
const web = { machine: 'm1', peer: 'm1', sandbox: 'aa', name: 'Sandbox web', owner: { task: 'p:t1' }, state: 'online' };
const machine = (id: string, sandbox: string, name: string) => ({ id: `laptop:${id}`, body: { id, name, data: { sandbox, owner: { task: `p:${id}` } } } });

test('every_sandbox_the_grid_holds_is_listed_with_how_many_of_the_most_and_one_made_elsewhere_deletes_by_its_id', async () => {
  const p = mount(async (id, a) => {
    if (id === 'sandbox.info')
      return { ...base, sandboxes: [web, { machine: '', peer: '', sandbox: 'bb', name: 'api', owner: null, state: '', elsewhere: true }], used: 2, limit: 2 };
    if (id === 'sandbox.drop') return a['sandbox'] === 'bb' ? { dropped: 1 } : Promise.reject(new Error('no'));
    return {};
  });
  await until(() => p.host.querySelector('[data-part="sandbox.quota"]') !== null);
  expect(p.host.querySelector('[data-part="sandbox.quota"]')?.textContent).toContain('2 of 2 sandboxes');
  expect(p.host.querySelector('[data-part="sandbox.quota.meter"]')?.getAttribute('data-tone'), 'full: warn').toBe('warn');
  expect(p.rows()).toHaveLength(2);
  const elsewhere = [...p.host.querySelectorAll('[data-part="sandbox.row"]')][1]!;
  expect(elsewhere.textContent).toContain('Not on this computer');
  expect(p.button('Sign in to Codex there', elsewhere), 'no peer to sign in on').toBeUndefined();
  p.button('Delete', elsewhere)!.click();
  // Delete asks first (UI QA r1): only the confirm's Delete drops it
  await until(() => document.querySelector('[data-part="sandbox.confirm.ok"]') !== null);
  expect(p.calls.some((c) => c.id === 'sandbox.drop'), 'nothing dropped before the confirm').toBe(false);
  document.querySelector<HTMLElement>('[data-part="sandbox.confirm.ok"]')!.click();
  expect(p.rows(), 'gone at once').toHaveLength(1);
  await until(() => p.calls.some((c) => c.id === 'sandbox.drop'));
  expect(p.calls.find((c) => c.id === 'sandbox.drop')?.args).toEqual({ peer: 'laptop', sandbox: 'bb' });
  p.stop();
});

test('a_sandbox_machine_landing_going_or_changing_state_shows_without_a_refetch', async () => {
  let infos = 0;
  const p = mount(async (id) => {
    if (id === 'sandbox.info') {
      infos++;
      return { ...base, sandboxes: [web] };
    }
    return {};
  });
  // the pane shows once sandbox.info has answered (a skeleton until then, UI QA r1)
  await until(() => infos > 0 && p.host.querySelector('[data-part="sandbox.loading"]') === null);
  p.lists.machines_peer([machine('m1', 'aa', 'Sandbox web'), { id: 'laptop:mac', body: { id: 'mac', name: 'Mac', data: { computer: 'c1' } } }]);
  p.lists.machines_status([{ id: 'laptop:m1', body: { id: 'm1', state: 'online' } }]);
  expect(p.rows()).toHaveLength(1);
  expect(p.rows()[0]).toContain('online');
  const before = infos;
  // asleep: the row says so from its link's row, with no new sandbox.info
  p.lists.machines_status([{ id: 'laptop:m1', body: { id: 'm1', state: 'asleep' } }]);
  expect(p.rows()[0]).toContain('asleep');
  expect(infos).toBe(before);
  // a launch's sandbox lands, then the first one goes
  p.lists.machines_peer([machine('m1', 'aa', 'Sandbox web'), machine('m2', 'cc', 'Sandbox api')]);
  expect(p.rows().map((r) => r.split('A task')[0])).toEqual(['Sandbox web', 'Sandbox api']);
  p.lists.machines_peer([machine('m2', 'cc', 'Sandbox api')]);
  expect(p.rows()).toHaveLength(1);
  expect(p.rows()[0]).toContain('Sandbox api');
  // the list changing asks sandbox.info again, once for the burst
  p.timers.run();
  await until(() => infos === before + 1);
  expect(infos).toBe(before + 1);
  p.stop();
});

test('codex_signs_in_on_its_row_waiting_then_its_page_and_code_and_a_refusal_says_why', async () => {
  const answers: (LoginAnswer | Error)[] = [{ state: 'running' }, { state: 'waiting', url: 'https://auth.openai.com/device', code: 'ABCD-1234' }];
  const p = mount(async (id) => {
    if (id === 'sandbox.info') return { ...base, sandboxes: [web] };
    if (id === 'accounts.login.start' || id === 'accounts.login') {
      const a = answers.shift() ?? { state: 'waiting', url: 'https://auth.openai.com/device', code: 'ABCD-1234' };
      return a instanceof Error ? Promise.reject(a) : a;
    }
    return {};
  });
  await until(() => p.button('Sign in to Codex there') !== undefined);
  p.button('Sign in to Codex there')!.click();
  await until(() => p.host.querySelector('[data-part="sandbox.codex.waiting"]') !== null);
  // the first answer has no page yet: waiting on the row, not nothing
  expect(p.host.querySelector('[data-part="sandbox.codex.waiting"]')).not.toBeNull();
  p.timers.run();
  await until(() => p.host.querySelector('[data-part="sandbox.codex.open"]') !== null);
  expect(p.host.querySelector('[data-part="sandbox.codex.waiting"]')).toBeNull();
  const open = p.host.querySelector<HTMLButtonElement>('[data-part="sandbox.codex.open"]');
  expect(open).not.toBeNull();
  expect(p.host.querySelector('[data-part="sandbox.codex.code"]')?.textContent).toContain('ABCD-1234');
  open!.click();
  expect(p.shell.platform.openExternal).toHaveBeenCalledWith('https://auth.openai.com/device');
  (p.host.querySelector('[data-part="sandbox.codex.code"]') as HTMLButtonElement).click();
  expect(p.shell.platform.copy).toHaveBeenCalledWith('ABCD-1234');
  // tried again and refused: the error on the row
  answers.splice(0, answers.length, new Error('codex is not installed there'));
  p.button('Sign in to Codex there')!.click();
  await until(() => p.host.querySelector('[data-part="sandbox.codex.error"]') !== null);
  expect(p.host.querySelector('[data-part="sandbox.codex.error"]')?.textContent).toContain('codex is not installed there');
  expect(p.shell.toast.show).not.toHaveBeenCalled();
  p.stop();
});

test('an_env_var_the_grid_refuses_comes_back_into_its_inputs_with_why_and_a_bad_name_never_goes', async () => {
  const p = mount(async (id) => {
    if (id === 'sandbox.info') return base;
    if (id === 'sandbox.env') return Promise.reject(new Error('This grid keeps 40 variables at most.'));
    return {};
  });
  await until(() => p.host.textContent?.includes('Signed in for every sandbox') === true);
  p.type('NAME', '1ABC');
  p.type('value', 'x');
  expect(p.host.querySelector('[data-part="sandbox.env.problem"]')?.textContent).toContain('starts with a digit');
  expect(p.button('Add')!.disabled).toBe(true);
  p.type('NAME', 'DATABASE_URL');
  p.type('value', 'postgres://x');
  expect(p.host.querySelector('[data-part="sandbox.env.problem"]')).toBeNull();
  p.button('Add')!.click();
  expect(p.host.querySelectorAll('[data-part="sandbox.env.row"]'), 'its name at once').toHaveLength(1);
  await until(() => p.host.querySelector('[data-part="sandbox.env.error"]') !== null);
  expect(p.input('NAME')).toBe('DATABASE_URL');
  expect(p.input('value')).toBe('postgres://x');
  expect(p.host.querySelectorAll('[data-part="sandbox.env.row"]'), 'and gone again').toHaveLength(0);
  expect(p.host.querySelector('[data-part="sandbox.env.error"]')?.textContent).toContain('40 variables at most');
  expect(p.calls.filter((c) => c.id === 'sandbox.env')).toHaveLength(1);
  p.stop();
});

// UI QA r1: the trash dropped a sandbox at once, with no question, and what wasn't brought back from it went too; and
// while sandbox.info was on its way, or when it failed, the pane said "Not signed in", "None yet" and "None now" as if
// nothing were set up. Delete asks first (naming the sandbox and whose it is); the pane shows a skeleton until it
// knows, and why it couldn't know, with Try again.
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const emptyRows = (): number =>
  document.querySelectorAll('[data-part="sandbox.list.none"], [data-part="sandbox.env.none"], [data-part="sandbox.claude"]').length;
const drops = (calls: { id: string }[]): number => calls.filter((c) => c.id === 'sandbox.drop').length;

test('delete_asks_first_naming_the_sandbox_and_cancel_keeps_it', async () => {
  const p = mount(async (id) => (id === 'sandbox.info' ? { ...base, sandboxes: [web] } : {}));
  await until(() => p.rows().length === 1);
  q('[data-part="sandbox.row"] [aria-label="Delete Sandbox web"]')?.click();
  await until(() => q('[data-part="sandbox.confirm"]') !== null);
  const dialog = q('[data-part="sandbox.confirm"]');
  expect(dialog, 'a confirm opens').not.toBeNull();
  expect(dialog?.textContent).toContain('Sandbox web');
  expect(drops(p.calls), 'nothing dropped yet').toBe(0);
  q('[data-part="sandbox.confirm.cancel"]')?.click();
  await until(() => q('[data-part="sandbox.confirm"]') === null);
  expect(q('[data-part="sandbox.confirm"]')).toBeNull();
  expect(p.rows(), 'still listed').toHaveLength(1);
  expect(drops(p.calls)).toBe(0);
  p.stop();
});

test('delete_drops_only_on_yes', async () => {
  const p = mount(async (id) => (id === 'sandbox.info' ? { ...base, sandboxes: [web] } : {}));
  await until(() => p.rows().length === 1);
  q('[data-part="sandbox.row"] [aria-label="Delete Sandbox web"]')?.click();
  await until(() => q('[data-part="sandbox.confirm.ok"]') !== null);
  expect(q('[data-part="sandbox.confirm.ok"]')?.textContent).toBe('Delete');
  q('[data-part="sandbox.confirm.ok"]')?.click();
  await until(() => p.rows().length === 0);
  expect(p.calls.filter((c) => c.id === 'sandbox.drop').map((c) => c.args['machine'])).toEqual(['m1']);
  p.stop();
});

test('while_sandbox_info_is_on_its_way_the_pane_shows_a_skeleton_not_empty_rows', async () => {
  const p = mount((id) => (id === 'sandbox.info' ? new Promise(() => undefined) : Promise.resolve({})));
  await new Promise((r) => setTimeout(r, 0));
  expect(q('[data-part="sandbox.loading"]')).not.toBeNull();
  expect(emptyRows()).toBe(0);
  expect(document.body.textContent).not.toContain('Not signed in');
  p.stop();
});

test('sandbox_info_failing_says_why_and_try_again_brings_the_list', async () => {
  let fail = true;
  const p = mount(async (id) => {
    if (id !== 'sandbox.info') return {};
    if (fail) throw new Error('grid unreachable');
    return { ...base, sandboxes: [web] };
  });
  await until(() => q('[data-part="sandbox.info.error"]') !== null);
  expect(q('[data-part="sandbox.info.error"]')?.textContent).toContain('grid unreachable');
  expect(emptyRows(), 'no "none" rows, no signed-out Sign in').toBe(0);
  expect(q('[data-part="sandbox.loading"]')).toBeNull();
  fail = false;
  q('[data-part="sandbox.info.retry"]')?.click();
  await until(() => p.rows().length === 1);
  expect(p.calls.filter((c) => c.id === 'sandbox.info').length).toBeGreaterThanOrEqual(2);
  expect(q('[data-part="sandbox.info.error"]')).toBeNull();
  p.stop();
});
