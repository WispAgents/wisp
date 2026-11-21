// The harness chip's usage ring (UX 2026-11-18 #8), on Home's launcher and the task page's reply box: the picked
// harness's fullest plan window (wisp.budget's `budget.view`) on the computer it runs on, the picked account's only.
// Read again as wisp.budget's plan topic changes (a run recorded its windows, a reset), watched passively while the
// chip shows, never polled. No budget plugin: no ring.
import { createEffect, createMemo, createResource, onCleanup, type Accessor } from 'solid-js';
import type { PeerId } from '@wisp/client';
import { useSignal } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import { planWindows, ringOf } from './runner.ts';

/** The chip's ring, in the chip's words ("Weekly 91% used"); `agent` '' or 'auto': none (Auto picks per task). */
export function usePlanRing(
  shell: Pick<ShellContext, 'client' | 't'>,
  o: { readonly peer: Accessor<string>; readonly agent: Accessor<string>; readonly account: Accessor<string> },
): Accessor<{ pct: number; tone: 'warn' | 'bad'; title: string } | undefined> {
  createEffect(() => {
    const peer = o.peer();
    if (!peer) return;
    const scope = shell.client.scope.child(`plan-ring:${peer}`);
    onCleanup(() => scope.dispose());
    shell.client.peers.watch(peer as PeerId, 'plugin:wisp.budget:plan', scope, { passive: true });
  });
  const rows = useSignal(shell.client.entities.list('budget_acc'));
  let reads = 0;
  const at = createMemo(() => (rows(), ++reads));
  const [plan] = createResource(
    () => (o.peer() && o.agent() && o.agent() !== 'auto' ? `${o.peer()}\n${o.agent()}\n${o.account()}\n${at()}` : undefined),
    (k) => {
      const [peer = '', agent = '', account = ''] = k.split('\n');
      return shell.client.commands.run(planWindows, { peer, agent }).then(
        (l) => ({ key: `${peer}\n${agent}\n${account}`, ring: ringOf(l, account) }),
        () => undefined,
      );
    },
  );
  return () => {
    // the answer for what's picked now only: a switch never shows the last harness's ring while it reads
    const got = plan.latest;
    const u = got && got.key === `${o.peer()}\n${o.agent()}\n${o.account()}` ? got.ring : undefined;
    return u && { pct: u.pct, tone: u.tone, title: shell.t('shell.home.usageRing', { label: u.label, pct: u.pct }) };
  };
}
