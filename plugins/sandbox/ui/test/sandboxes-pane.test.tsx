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
async function until(ok: () => boolean, ms = 4000): Promise<void> {
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
  await until(() => infos > 0);
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
