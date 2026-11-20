// The factory band over a run's task (ADR-0114): one row for the whole run, its stations as steps under it. The header
// says the factory, where it is (`Review · round 2 of 3`, Waiting on you, Done) and a menu: Open each station's task,
// Open the PR, Stop the run. A station that waits on you says the question; the question card itself is the task's
// own (it's `runtime.ask` on the run's task), so you answer it where you answer any agent's question.

import { Show, createMemo, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Box, Icon, Inline, Menu, Pill, Stack, Steps, Text, useSignal } from '@wisp/ui';
import { defineCommand, type EntityKey, type PluginClient } from '@wisp/client';
import { t, type Vars } from './i18n.ts';
import { live, openable, runOf, step, where, type Said } from './model.ts';
import { KIND, type SessionRow } from './types.ts';
import './i18n/lazy.ts';

/** `factory.stop`: the run's stations stop; the band says so once the row does (the button shows it's pending). */
const stopRun = defineCommand<{ session: EntityKey }, unknown>({
  id: 'factory.stop',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.session.slice(a.session.indexOf(':') + 1) }),
  optimistic: 'none',
  pending: 'the factory run',
});

/** A said thing in words; a var that names a station is that station's name. */
function say(s: Said): string {
  const vars: Record<string, string | number> = {};
  for (const [k, v] of Object.entries(s.vars ?? {})) vars[k] = typeof v === 'string' && v.startsWith('wisp.factory.') ? t(v as never) : v;
  return t(s.key, vars as Vars);
}

const keyOf = (client: PluginClient, id: string): EntityKey => (id.includes(':') ? id : `${client.home}:${id}`) as EntityKey;

export default function Band(props: ExtensionProps): JSX.Element {
  const key = (): EntityKey | undefined => {
    const s = props.session?.();
    return s ? keyOf(props.client, s.id) : undefined;
  };
  const k = key();
  const row = k ? useSignal(props.client.entities.row<SessionRow>(KIND.session, k)) : () => undefined;
  const run = createMemo(() => runOf(row() as SessionRow | undefined));
  const peer = (): string => (key() ?? '').slice(0, (key() ?? '').indexOf(':'));
  const actions = (): { value: string; label: string; icon: 'external' | 'stop' | 'github' }[] => {
    const r = run();
    if (!r) return [];
    const open = openable(r).map((o) => ({ value: `open:${o.task}`, label: say(o.said), icon: 'external' as const }));
    const pr = r.pr ? [{ value: 'pr', label: t('wisp.factory.openPr'), icon: 'github' as const }] : [];
    const stop = live(r) ? [{ value: 'stop', label: t('wisp.factory.stop'), icon: 'stop' as const }] : [];
    return [...open, ...pr, ...stop];
  };
  const act = (v: string): void => {
    const r = run();
    const at = key();
    if (!r || !at) return;
    if (v === 'stop') void props.client.run(stopRun, { session: at }).catch(() => undefined);
    else if (v === 'pr' && r.pr) window.open(r.pr, '_blank', 'noopener');
    else if (v.startsWith('open:')) props.client.navigate({ t: 'openTask', id: `${peer()}:${v.slice(5)}` as never });
  };
  return (
    <Show when={run()}>
      {(r) => {
        const w = (): ReturnType<typeof where> => where(r());
        const steps = (): { title: string; detail?: string; status: ReturnType<typeof step>['status'] }[] =>
          r().stations.map((s) => {
            const x = step(r(), s);
            const detail = x.detail === undefined ? undefined : typeof x.detail === 'string' ? x.detail : say(x.detail);
            return { title: say(x.title), status: x.status, ...(detail !== undefined && { detail }) };
          });
        return (
          <Box as="section" part="factory.band" pad="3" radius="md" border="line" surface="surface" aria-label={t('wisp.factory.band')}>
            <Stack gap="2">
              <Inline gap="2" align="center">
                <Icon name="queue" size="sm" />
                <Box grow>
                  <Text part="factory.name" weight="medium" truncate>
                    {t('wisp.factory.title', { name: r().name })}
                  </Text>
                </Box>
                <Pill part="factory.where" tone={w().tone} dot={live(r())}>
                  {say(w().said)}
                </Pill>
                <Show when={actions().length > 0}>
                  <Menu part="factory.more" trigger={{ icon: 'dots', 'aria-label': t('wisp.factory.actions'), variant: 'ghost', size: 'xs' }} items={actions()} onSelect={act} placement="bottom-end" />
                </Show>
              </Inline>
              <Steps part="factory.stations" items={steps()} />
              <Show when={r().why && !live(r())}>
                <Text part="factory.why" size="sm" tone="muted">
                  {r().why}
                </Text>
              </Show>
            </Stack>
          </Box>
        );
      }}
    </Show>
  );
}
