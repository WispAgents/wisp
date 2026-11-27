// The trusted themes' stylesheet on the page: a trusted plugin switched on adds it, switched off (or untrusted, or
// removed) takes it away, the shell's scope ending removes it, a stale answer never lands, a failed read keeps what's
// there. Then through createShell and a real client: the home wisp's `plugins.changed` notice makes it read again.
import { expect, test } from 'vitest';
import { createClient, createScope, peerId, type Frame, type Transport } from '@wisp/client';
import type { ClientMessage, ServerMessage } from '@wisp/protocol';
import { createToaster } from '@wisp/ui';
import { STYLE_ID, mountPluginStyles } from '../src/theme-styles/mount.ts';
import type { PluginStyles } from '../src/theme-styles/part-styles.ts';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { commandErrorToast } from '../src/toasts.ts';
import { nullPlatform } from '../src/platform/index.ts';

const theme: PluginStyles = { plugin: 'user.pink', partStyles: { 'composer.send': { background: 'hotpink' } } };
const sheet = (): string | null => document.getElementById(STYLE_ID)?.textContent ?? null;
/** Waits until `ok()` holds (a clock deadline, not a count of hops); fails the test if it never does. */
async function until(ok: () => boolean, ms = 15_000): Promise<void> {
  const end = Date.now() + ms;
  while (!ok()) {
    if (Date.now() > end) throw new Error('timed out waiting');
    await new Promise((r) => setTimeout(r, 5));
  }
}
const has = (): boolean => sheet() !== null;

test('on adds the style, off removes it, the scope ending removes it', async () => {
  let list: PluginStyles[] = [theme];
  let fire = (): void => {};
  const scope = createScope('t');
  const h = mountPluginStyles({ document, scope, load: async () => list, onChange: (fn) => ((fire = fn), () => {}) });
  await h.refresh();
  expect(sheet()).toContain('[data-part~="composer.send"] { background: hotpink; }');
  expect(document.head.querySelectorAll(`#${STYLE_ID}`).length).toBe(1);
  list = [];
  fire();
  await until(() => !has());
  list = [theme];
  fire();
  await until(has);
  scope.dispose();
  expect(sheet()).toBeNull();
});

test('a stale answer never lands and a failed read keeps what is there', async () => {
  const answers: ((l: PluginStyles[]) => void)[] = [];
  let fail = false;
  const scope = createScope('t');
  const h = mountPluginStyles({
    document,
    scope,
    load: () => (fail ? Promise.reject(new Error('offline')) : new Promise((r) => answers.push(r))),
    onChange: () => () => {},
  });
  const first = h.refresh();
  const second = h.refresh();
  answers[1]?.([theme]);
  await second;
  answers[0]?.([]); // the older request answers last: dropped
  await first;
  expect(sheet()).toContain('hotpink');
  fail = true;
  await h.refresh();
  expect(sheet()).toContain('hotpink');
  scope.dispose();
});

/** A home wisp in memory: welcome, `plugins.styles` answered from `trusted`, and a notice when asked. */
function fakeHome(o: { refuseStyles?: boolean } = {}) {
  let trustedOn: PluginStyles[] = [];
  let push: ((f: Frame) => void) | undefined;
  const transport: Transport = {
    kind: 'mem',
    async *connect(_params, signal) {
      const q: Frame[] = [];
      let wake: (() => void) | undefined;
      push = (f) => (q.push(f), wake?.());
      const peer = { peer: 'local', name: 'local', build: 't', version: '2', proto_min: 2, proto_max: 2, plugins: [] };
      push({ id: 0, messages: [{ t: 'welcome', epoch: 'e', head_seq: 0, oldest_seq: 0, resume: false, proto: 2, peer }] });
      while (!signal.aborted) {
        const f = q.shift();
        if (f) yield f;
        else await new Promise<void>((r) => (wake = r));
      }
    },
    async post(m: ClientMessage) {
      if (m.t !== 'mutate') return;
      const msg: ServerMessage = m.cmd === 'plugins.styles' && !o.refuseStyles ? { t: 'ack', mid: m.mid, seq: 0, result: trustedOn } : { t: 'reject', mid: m.mid, code: 'unknown_command', message: m.cmd };
      queueMicrotask(() => push?.({ id: null, messages: [msg] }));
    },
  };
  return {
    transport,
    set(l: PluginStyles[]) {
      trustedOn = l;
      push?.({ id: null, messages: [{ t: 'notice', kind: 'plugins.changed', data: { id: 'user.pink' } }] });
    },
  };
}

test('through the shell: enabling a trusted theme adds its style, disabling removes it', async () => {
  const home = peerId('local');
  const fake = fakeHome();
  const client = createClient({ clientId: 'c', build: 't', home, width: 1200 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'local', transport: fake.transport }]);
  await until(() => client.peers.status(home)().kind === 'online');
  expect(sheet()).toBeNull(); // nothing trusted and on yet
  fake.set([theme]); // switched on (the server only lists trusted plugins that are on)
  await until(has);
  expect(sheet()).toContain('@layer plugins');
  expect(sheet()).toContain('background: hotpink');
  fake.set([]); // switched off
  await until(() => !has());
  fake.set([theme]);
  await until(has);
  shell.dispose();
  expect(sheet()).toBeNull();
});

// QA r2 phone #1: a home wisp without `plugins.styles` (an older wisp, a remote peer) refused the shell's startup read,
// and every load showed a red "unknown command plugins.styles (is its plugin off?)" toast. That's "no plugin styles":
// nothing on the page, and no toast.
test('a home wisp without plugins.styles means no plugin styles, never an error toast', async () => {
  const home = peerId('local');
  const fake = fakeHome({ refuseStyles: true });
  const base = createToaster();
  const errors: string[] = [];
  const toaster = { ...base, error: (m: string, opts?: Parameters<typeof base.error>[1]) => (errors.push(m), base.error(m, opts)) };
  const client = createClient({ clientId: 'c', build: 't', home, width: 1200, onError: commandErrorToast(toaster) });
  const shell = createShell({ client, toaster, platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'local', transport: fake.transport }]);
  await until(() => client.peers.status(home)().kind === 'online');
  fake.set([theme]); // the notice asks again: still refused, still quiet
  await new Promise((r) => setTimeout(r, 50));
  expect(errors).toEqual([]);
  expect(sheet()).toBeNull();
  shell.dispose();
});
