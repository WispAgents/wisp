// Inline live cards for agents delegated on a turn (`turn.ext['wisp.runtime'].delegations`).
// Shown right under the turn in Chat mode (and Report mode) so delegated subagents are visible and one click away.
import { createEffect, createMemo, createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import { entityKey, ownerOf, type EntityKey, type PluginClient } from '@wisp/client';
import { Avatar, Card, ListRow, Pill, Stack, TeamPlace, TeamTime, useSignal } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { delegationTime, delegationsOf, type DelegationSessionLike } from './delegations.ts';
import type { TurnDoc } from './model.ts';

const TICK_MS = 1000;

export function TurnDelegations(props: {
  readonly client: PluginClient;
  readonly turnKey: EntityKey;
  readonly doc: TurnDoc;
  readonly nameOf: (agent: string) => string;
}): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const harnesses = useSignal(shell.harnesses());
  const sessions = useSignal(props.client.entities.list<DelegationSessionLike>('session'));
  const rows = createMemo(() => delegationsOf(props.doc, sessions(), props.nameOf, t));
  const [now, setNow] = createSignal(props.client.scope.timers.now());
  const running = createMemo(() => rows().some((r) => r.running));
  createEffect(() => {
    if (!running()) return;
    setNow(props.client.scope.timers.now());
    const stop = props.client.scope.interval(TICK_MS, () => setNow(props.client.scope.timers.now()));
    onCleanup(stop);
  });
  const logoOf = (agent: string): string | undefined => harnesses().find((h) => h.id === agent)?.logo;
  const open = (sessionId: string): void => {
    shell.navigate({ t: 'openTask', id: entityKey(ownerOf(props.turnKey), sessionId) as never });
  };
  return (
    <Show when={rows().length > 0}>
      <Card part="turn.delegations" pad="2" as="section" aria-label={t('shell-task.delegations.title')}>
        <Stack gap="1">
          <For each={rows()}>
            {(r) => {
              const ms = (): number | undefined => delegationTime(r, now());
              const timed = createMemo(() => ms() !== undefined);
              const logo = (): string | undefined => logoOf(r.agent);
              return (
                <ListRow
                  part="turn.delegation"
                  title={r.name}
                  lead={<Avatar part="turn.delegation.harness" name={r.name} {...((src) => (src ? { src } : {}))(logo())} size="sm" shape="square" />}
                  sub={
                    <>
                      <Show when={r.place}>
                        {(p) => <TeamPlace part="turn.delegation.place" name={p()} icon="server" away title={p()} />}
                      </Show>
                      {r.line}
                    </>
                  }
                  subTone={r.live ? 'accent-ink' : r.tone === 'warn' ? 'warn' : 'muted'}
                  pills={
                    <Show when={!r.live || !timed()}>
                      <Pill tone={r.tone} dot live={r.live} size="sm">
                        {r.status}
                      </Pill>
                    </Show>
                  }
                  {...(timed() && { time: <TeamTime part="turn.delegation.time" ms={ms() ?? 0} /> })}
                  {...(r.openable && { onOpen: () => open(r.id) })}
                />
              );
            }}
          </For>
        </Stack>
      </Card>
    </Show>
  );
}
