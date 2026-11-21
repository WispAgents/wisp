// What Settings → Cloud and Settings → Machines (v1 mountGridHere) read of this computer's grid sign-in: the row,
// the address being typed, the sign-in under way, and Sign in.
import { createSignal, onCleanup } from 'solid-js';
import { entityKey, type PluginClient } from '@wisp/client';
import { useSignal } from '@wisp/ui';
import { KIND, signIn, TOPIC, type CloudRow } from '../commands.ts';
import { t } from '../i18n.ts';
import { hereOf } from '../here.ts';
import { gridField } from '../grid-field.ts';
import '../i18n/lazy.ts';

export const host = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

export function useCloud(c: PluginClient) {
  const peer = c.home;
  const scope = c.scope.child('cloud-state');
  onCleanup(() => scope.dispose());
  c.watch(peer, TOPIC, scope);
  const row = useSignal(c.entities.row<CloudRow>(KIND, entityKey(peer, 'this')));
  const r = (): CloudRow | undefined => row() as CloudRow | undefined;
  // what you typed; until then, the grid of the sign-in under way, or yours, else wisp Cloud (setup audit F2)
  const [typed, setGrid] = createSignal<string>();
  const grid = (): string => typed() ?? gridField(r());
  const [starting, setStarting] = createSignal(false);
  const waiting = () => (r()?.signin?.state === 'waiting' ? r()?.signin : undefined);
  const failed = () => (r()?.signin && ['denied', 'expired', 'failed'].includes(r()?.signin?.state ?? '') ? r()?.signin?.error : undefined);
  const start = (): void => {
    setStarting(true);
    c.run(signIn, { peer, grid: grid() })
      .then((s) => {
        if (s.verifyUrl) window.open(s.verifyUrl, '_blank', 'noopener');
      })
      .catch(() => {})
      .finally(() => setStarting(false));
  };
  const here = () => hereOf(r()?.computer, t);
  return { c, peer, r, grid, setGrid, starting, waiting, failed, start, here };
}
