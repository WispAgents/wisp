// What a peer's status means, over the list (v1 reconnecting.js, plan §7.2.3, §7.3.2): your own
// wisp reconnecting, offline since a time (with Retry), a box asleep (with Wake), a peer to update (with the command), this app to update (with Reload). Rows from the last cursor stay on
// screen under it either way: nothing goes empty while a computer is away.

import { For, Show, createMemo, type JSX } from 'solid-js';
import type { PeerId } from '@wisp/client';
import { Banner, Button, Text, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { peerNote } from './peer-note.ts';
import '../i18n/lazy.ts';

function PeerNoteBar(props: { readonly peer: PeerId }): JSX.Element {
  const shell = useShell();
  const status = useSignal(shell.client.peers.status(props.peer));
  const note = createMemo(() => peerNote(status(), shell.client.peers.name(props.peer), shell.t, shell.client.scope.timers.now()));
  const act = (a: 'wake' | 'retry' | 'reload'): void => {
    if (a === 'reload') shell.platform.reload();
    else if (a === 'wake') void shell.client.peers.wake(props.peer);
    else shell.client.peers.reconnect(props.peer);
  };
  return (
    <Show when={note()}>
      {(n) => (
        <Banner
          tone={n().tone}
          part="shell.peer.banner"
          action={
            n().action ? (
              <Button size="sm" variant="secondary" onClick={() => act(n().action ?? 'retry')}>
                {shell.t(n().action === 'wake' ? 'shell.peer.wake' : n().action === 'reload' ? 'shell.peer.reload' : 'shell.peer.retry')}
              </Button>
            ) : undefined
          }
        >
          {n().text}
          <Show when={n().command}>
            {(c) => (
              <Text mono size="sm">
                {c()}
              </Text>
            )}
          </Show>
        </Banner>
      )}
    </Show>
  );
}

export function PeerBanner(): JSX.Element {
  const shell = useShell();
  const peers = useSignal(shell.client.peers.list());
  return <For each={peers()}>{(p) => <PeerNoteBar peer={p} />}</For>;
}
