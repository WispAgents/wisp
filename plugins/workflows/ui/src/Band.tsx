// The workflow band over a task's turns (ADR-0119): the run its agent (or you) started, live. The header says the
// workflow, where it is (`Build · 4/9`, then how it ended), how long it has run and what it cost of its budget, and a
// menu: Copy as workflow (a formula to keep and run again), Stop the run (asked first). A bar fills as steps end. The
// board under it is the kit's RunBoard: the phases as lanes, each step a card that opens its task, a fan-out a grid of
// dots. Earlier runs of the task fold into one line each; a click opens one. On a step's own task, one line says which
// run it belongs to and opens that run's task.

import { For, Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Box, Button, Dialog, Icon, Inline, Menu, Meter, Pill, RunBoard, Stack, Text, useSignal } from '@wisp/ui';
import { defineCommand, type EntityKey, type PluginClient } from '@wisp/client';
import { t } from './i18n.ts';
import { asFormula, lanes, line, live, pill, runsOf, share, stepOf } from './model.ts';
import { KIND, type RunView, type SessionRow } from './types.ts';
import './i18n/lazy.ts';

/** `workflows.stop`: the run's working steps stop; the board says so once the row does (the button shows it's pending). */
const stopRun = defineCommand<{ session: EntityKey; run: string }, unknown>({
  id: 'workflows.stop',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.run }),
  optimistic: 'none',
  pending: 'the workflow run',
});

const keyOf = (client: PluginClient, id: string): EntityKey => (id.includes(':') ? id : `${client.home}:${id}`) as EntityKey;

/** A clock for the live durations: ticks each second while a run goes, on the client's scope (stops when none does). */
function useNow(c: PluginClient, going: () => boolean): () => number {
  const [now, setNow] = createSignal(c.scope.timers.now());
  createEffect(() => {
    if (!going()) return;
    setNow(c.scope.timers.now());
    onCleanup(c.scope.interval(1000, () => setNow(c.scope.timers.now())));
  });
  return now;
}

export default function Band(props: ExtensionProps): JSX.Element {
  const k = ((): EntityKey | undefined => {
    const s = props.session?.();
    return s ? keyOf(props.client, s.id) : undefined;
  })();
  const row = k ? useSignal(props.client.entities.row<SessionRow>(KIND.session, k)) : () => undefined;
  const runs = createMemo(() => runsOf(row() as SessionRow | undefined));
  const peer = (): string => (k ?? '').slice(0, (k ?? '').indexOf(':'));
  const now = useNow(props.client, () => runs().some(live));
  // the run shown in full: the one you opened, else the newest
  const [picked, setPicked] = createSignal<string | undefined>();
  const shown = createMemo((): RunView | undefined => runs().find((r) => r.id === picked()) ?? runs()[runs().length - 1]);
  const earlier = createMemo(() => runs().filter((r) => r.id !== shown()?.id).reverse());
  const open = (task: string): void => props.client.navigate({ t: 'openTask', id: `${peer()}:${task}` as never });
  const [asking, setAsking] = createSignal(false);
  const [copied, setCopied] = createSignal(false);
  const stop = (): void => {
    setAsking(false);
    const r = shown();
    if (k && r && live(r)) void props.client.run(stopRun, { session: k, run: r.id }).catch(() => undefined);
  };
  const act = (v: string): void => {
    const r = shown();
    if (!r) return;
    if (v === 'stop') setAsking(true);
    else if (v === 'copy') {
      void navigator.clipboard?.writeText(asFormula(r)).then(() => {
        setCopied(true);
        props.client.scope.timeout(1600, () => setCopied(false));
      });
    }
  };
  const actions = (r: RunView): { value: string; label: string; icon: 'copy' | 'stop' }[] => [
    ...(r.spec ? [{ value: 'copy', label: t('wisp.workflows.copy'), icon: 'copy' as const }] : []),
    ...(live(r) ? [{ value: 'stop', label: t('wisp.workflows.stop'), icon: 'stop' as const }] : []),
  ];
  const part = createMemo(() => stepOf(row() as SessionRow | undefined));
  const parent = (): string | undefined => {
    const p = (row() as SessionRow | undefined)?.parentId;
    return typeof p === 'string' && p ? p.slice(p.indexOf(':') + 1) : undefined;
  };
  return (
    <>
      <Show when={runs().length === 0 && part() && parent()}>
        {(owner) => (
          <Inline part="workflows.partOf" gap="2" align="center">
            <Icon name="org" size="sm" />
            <Box grow>
              <Text size="sm" tone="muted" truncate>
                {t('wisp.workflows.partOf', { node: part()?.node ?? '' })}
              </Text>
            </Box>
            <Button part="workflows.partOf.open" variant="ghost" size="xs" onClick={() => open(owner())}>
              {t('wisp.workflows.openRun')}
            </Button>
          </Inline>
        )}
      </Show>
      <Show when={shown()}>
        {(r) => {
          const p = (): ReturnType<typeof pill> => pill(r());
          return (
            <Box as="section" part="workflows.band" pad="3" radius="md" border="line" surface="surface" aria-label={t('wisp.workflows.band', { name: r().workflow })}>
              <Stack gap="3">
                <Inline gap="2" align="center">
                  <Icon name="org" size="sm" />
                  <Box grow>
                    <Text part="workflows.name" weight="medium" truncate>
                      {r().workflow}
                    </Text>
                  </Box>
                  <Show when={copied()}>
                    <Pill part="workflows.copied" tone="good" size="sm">
                      {t('wisp.workflows.copied')}
                    </Pill>
                  </Show>
                  <Pill part="workflows.where" tone={p().tone} dot live={p().live}>
                    {p().text}
                  </Pill>
                  <Show when={actions(r()).length > 0}>
                    <Menu part="workflows.more" trigger={{ icon: 'dots', 'aria-label': t('wisp.workflows.actions'), variant: 'ghost', size: 'xs' }} items={actions(r())} onSelect={act} placement="bottom-end" />
                  </Show>
                </Inline>
                <Text part="workflows.line" size="sm" tone="muted" truncate>
                  {[line(r(), now()), r().description ?? ''].filter(Boolean).join(' — ')}
                </Text>
                <Meter part="workflows.progress" value={share(r())} label={t('wisp.workflows.progressLabel')} valueText={p().text} tone={p().tone === 'neutral' ? 'neutral' : p().tone} />
                <RunBoard part="workflows.board" label={t('wisp.workflows.board', { name: r().workflow })} lanes={lanes(r(), now(), open)} />
                <For each={r().notes ?? []}>
                  {(n) => (
                    <Text part="workflows.note" size="sm" tone="muted">
                      {n}
                    </Text>
                  )}
                </For>
                <Show when={earlier().length > 0}>
                  <Stack gap="1">
                    <Text part="workflows.earlier" size="xs" tone="muted">
                      {t('wisp.workflows.earlier')}
                    </Text>
                    <For each={earlier()}>
                      {(e) => (
                        <Inline gap="2">
                          <Button part="workflows.earlierRun" variant="ghost" size="xs" icon="org" onClick={() => setPicked(e.id)}>
                            {t('wisp.workflows.earlierLine', { name: e.workflow, state: pill(e).text, line: line(e, now()) })}
                          </Button>
                        </Inline>
                      )}
                    </For>
                  </Stack>
                </Show>
                <Show when={asking()}>
                  <Dialog
                    open
                    role="alertdialog"
                    size="sm"
                    title={t('wisp.workflows.stopTitle')}
                    description={t('wisp.workflows.stopBody')}
                    onOpenChange={(o) => !o && setAsking(false)}
                    part="workflows.stopAsk"
                    footer={
                      <Inline gap="4" justify="end">
                        <Button variant="ghost" onClick={() => setAsking(false)}>
                          {t('wisp.workflows.stopCancel')}
                        </Button>
                        <Button variant="danger" part="workflows.stopAsk.ok" onClick={stop}>
                          {t('wisp.workflows.stop')}
                        </Button>
                      </Inline>
                    }
                  />
                </Show>
              </Stack>
            </Box>
          );
        }}
      </Show>
    </>
  );
}
