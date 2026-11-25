// A task's workflow board in the main area (ADR-0120), opened from the workflow row in the task's details. Its head is a
// task's head: the workflow's name, Back and Close to the task, and the run's actions (Stop the run, asked first; Copy
// as workflow). Under it a breadcrumb, task › workflow, so the way back is always in sight, then where the run is (a
// bar that fills as steps end, how long, what it cost of its budget) and the kit's RunBoard: phases as lanes, each step
// a card that opens its task, a fan-out a grid of dots. A step's task has the same row in its details, back here: the
// navigation is task → board → step and back. The task's other runs fold into one line each; a click shows one.

import { For, Show, createMemo, createSignal, type JSX } from 'solid-js';
import type { EntityKey, PluginClient } from '@wisp/client';
import { Box, Button, Dialog, Icon, Inline, Meter, Pill, RunBoard, Scroll, Stack, TaskHead, Text, type HeadAction } from '@wisp/ui';
import { t } from './i18n.ts';
import { asFormula, lanes, line, live, phaseNow, pill, share } from './model.ts';
import { current, editRun, openTask, saveWorkflow, savedName, stopRun, useNow, useRuns } from './use.ts';
import type { RunView } from './types.ts';
import type { BoardParams } from './views.ts';
import './i18n/lazy.ts';

export default function Board(props: { readonly client: PluginClient; readonly params: BoardParams }): JSX.Element {
  const k = props.params.task as EntityKey;
  const bare = k.slice(k.indexOf(':') + 1);
  const { row, runs } = useRuns(props.client, k);
  const now = useNow(props.client, () => runs().some(live));
  // the run shown in full: the one asked for or picked, else the one going, else the newest
  const [picked, setPicked] = createSignal<string | undefined>(props.params.run);
  const shown = createMemo((): RunView | undefined => runs().find((r) => r.id === picked()) ?? current(runs()));
  const earlier = createMemo(() => runs().filter((r) => r.id !== shown()?.id).reverse());
  const back = (): void => openTask(props.client, k, bare);
  const taskTitle = (): string => (row() as { title?: string } | undefined)?.title || t('wisp.workflows.theTask');
  const [asking, setAsking] = createSignal(false);
  const [copied, setCopied] = createSignal(false);
  const [saved, setSaved] = createSignal(false);
  const act = (kind: 'skip' | 'retry', ids: readonly string[]): void => {
    const r = shown();
    if (r) void props.client.run(editRun, { session: k, run: r.id, ...(kind === 'skip' ? { skip: ids } : { retry: ids }) }).catch(() => undefined);
  };
  const save = (r: RunView): void => {
    if (!r.spec) return;
    void props.client
      .run(saveWorkflow, { session: k, name: savedName(r.workflow), spec: r.spec })
      .then(() => {
        setSaved(true);
        props.client.scope.timeout(1600, () => setSaved(false));
      })
      .catch(() => undefined);
  };
  const stop = (): void => {
    setAsking(false);
    const r = shown();
    if (r && live(r)) void props.client.run(stopRun, { session: k, run: r.id }).catch(() => undefined);
  };
  const copy = (r: RunView): void => {
    void navigator.clipboard?.writeText(asFormula(r)).then(() => {
      setCopied(true);
      props.client.scope.timeout(1600, () => setCopied(false));
    });
  };
  const bar = (): HeadAction[] => {
    const r = shown();
    if (!r) return [];
    return [
      ...(r.spec ? [{ id: 'save', icon: saved() ? ('check' as const) : ('bookmark' as const), label: t(saved() ? 'wisp.workflows.saved' : 'wisp.workflows.save'), title: t('wisp.workflows.save'), part: 'workflows.save', act: { onClick: () => save(r) } }] : []),
      ...(r.spec ? [{ id: 'copy', icon: copied() ? ('check' as const) : ('copy' as const), label: t(copied() ? 'wisp.workflows.copied' : 'wisp.workflows.copy'), title: t('wisp.workflows.copy'), part: 'workflows.copy', act: { onClick: () => copy(r) } }] : []),
      ...(live(r) ? [{ id: 'stop', icon: 'stop' as const, label: t('wisp.workflows.stop'), title: t('wisp.workflows.stop'), part: 'workflows.stop', act: { onClick: () => setAsking(true) } }] : []),
    ];
  };
  return (
    <Stack gap="0" grow part="workflows.page">
      <TaskHead
        title={shown()?.workflow ?? t('wisp.workflows.boardTitle')}
        color="slate"
        renameLabel=""
        panelLabel=""
        closeLabel={t('wisp.workflows.backTo', { task: taskTitle() })}
        backLabel={t('wisp.workflows.backTo', { task: taskTitle() })}
        onBack={back}
        onClose={back}
        bar={bar()}
      />
      <Scroll part="workflows.scroll" grow>
        <Box pad="5">
          <Stack gap="4">
            <Inline gap="1" align="center" part="workflows.crumbs">
              <Button part="workflows.crumb.task" variant="ghost" size="xs" icon="back" onClick={back}>
                {taskTitle()}
              </Button>
              <Icon name="chevron" size="sm" />
              <Text size="sm" weight="medium" truncate>
                {shown()?.workflow ?? ''}
              </Text>
            </Inline>
            <Show when={shown()} fallback={<Text tone="muted">{t('wisp.workflows.none')}</Text>}>
              {(r) => {
                const p = (): ReturnType<typeof pill> => pill(r());
                return (
                  <Stack gap="4">
                    <Inline gap="3" align="center">
                      <Pill part="workflows.where" tone={p().tone} dot live={p().live}>
                        {live(r()) && phaseNow(r()) ? `${phaseNow(r())} · ${p().text}` : p().text}
                      </Pill>
                      <Box grow>
                        <Text part="workflows.line" size="sm" tone="muted" truncate>
                          {[line(r(), now()), r().description ?? ''].filter(Boolean).join(' — ')}
                        </Text>
                      </Box>
                    </Inline>
                    <Meter part="workflows.progress" value={share(r())} label={t('wisp.workflows.progressLabel')} valueText={p().text} tone={p().tone} />
                    <RunBoard part="workflows.board" label={t('wisp.workflows.board', { name: r().workflow })} actionsLabel={t('wisp.workflows.stepActions')} lanes={lanes(r(), now(), { open: (task) => openTask(props.client, k, task), act })} />
                    <For each={r().notes ?? []}>
                      {(n) => (
                        <Text part="workflows.note" size="sm" tone="muted">
                          {n}
                        </Text>
                      )}
                    </For>
                  </Stack>
                );
              }}
            </Show>
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
          </Stack>
        </Box>
      </Scroll>
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
  );
}
