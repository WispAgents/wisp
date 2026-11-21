// The factory band over a run's task (ADR-0114): one row for the whole run, its stations as steps under it. The header
// says the factory, where it is while it goes (`Review · round 2 of 3`, Waiting on you; once it ended, the task's own
// status says how) and a menu: Open each station's task,
// Open the PR, Stop the run. A station that waits on you says the question; the question card itself is the task's
// own (it's `runtime.ask` on the run's task), so you answer it where you answer any agent's question.

import { Show, createMemo, createSignal, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Box, Button, Dialog, Icon, Inline, Menu, Pill, Stack, Steps, Text, useSignal } from '@wisp/ui';
import { defineCommand, type EntityKey, type PluginClient } from '@wisp/client';
import { t, type Vars } from './i18n.ts';
import { endWhy, live, openable, pill, runOf, step, type Said } from './model.ts';
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
  // Stop the run is asked first, as a task's Stop is (audit G13: every station stopped at once, with no way back)
  const [asking, setAsking] = createSignal(false);
  const stop = (): void => {
    setAsking(false);
    const at = key();
    if (at && run() && live(run() as NonNullable<ReturnType<typeof run>>)) void props.client.run(stopRun, { session: at }).catch(() => undefined);
  };
  const act = (v: string): void => {
    const r = run();
    const at = key();
    if (!r || !at) return;
    if (v === 'stop') setAsking(true);
    else if (v === 'pr' && r.pr) window.open(r.pr, '_blank', 'noopener');
    else if (v.startsWith('open:')) props.client.navigate({ t: 'openTask', id: `${peer()}:${v.slice(5)}` as never });
  };
  return (
    <Show when={run()}>
      {(r) => {
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
                <Show when={pill(r())}>
                  {(w) => (
                    <Pill part="factory.where" tone={w().tone} dot>
                      {say(w().said)}
                    </Pill>
                  )}
                </Show>
                <Show when={actions().length > 0}>
                  <Menu part="factory.more" trigger={{ icon: 'dots', 'aria-label': t('wisp.factory.actions'), variant: 'ghost', size: 'xs' }} items={actions()} onSelect={act} placement="bottom-end" />
                </Show>
              </Inline>
              <Steps part="factory.stations" items={steps()} />
              <Show when={asking()}>
                <Dialog
                  open
                  role="alertdialog"
                  size="sm"
                  title={t('wisp.factory.stopTitle')}
                  description={t('wisp.factory.stopBody')}
                  onOpenChange={(open) => !open && setAsking(false)}
                  part="factory.stopAsk"
                  footer={
                    <Inline gap="4" justify="end">
                      <Button variant="ghost" onClick={() => setAsking(false)}>
                        {t('wisp.factory.stopCancel')}
                      </Button>
                      <Button variant="danger" part="factory.stopAsk.ok" onClick={stop}>
                        {t('wisp.factory.stop')}
                      </Button>
                    </Inline>
                  }
                />
              </Show>
              <Show when={endWhy(r())}>
                {(why) => (
                  <Text part="factory.why" size="sm" tone="muted">
                    {why()}
                  </Text>
                )}
              </Show>
            </Stack>
          </Box>
        );
      }}
    </Show>
  );
}
