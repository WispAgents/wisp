// This Mac (This PC, This computer): what agents cost the computer, and acting on it
// (design/machine-page.md). A warning on top when the disk or memory runs short, then three tabs: Overview (a status
// line, memory/CPU/disk bars with their wait marks, what needs you, what runs now, the last 24 hours), Space (the disk as one bar, build
// output folder by folder, space to free, the harnesses' folders) and Limits (automations, and where their limits are).
// Painted from the cached row first (first paint comes from IndexedDB), then kept live by the topic.

import type { PeerId, PluginClient } from '@wisp/client';
import { Button, Callout, Dialog, ExtensionSlot, Inline, Stack, Tabs, Text } from '@wisp/ui';
import { createMemo, createSignal, onMount, Show, type JSX } from 'solid-js';
import { Automations } from './Automations.tsx';
import { DayChart, Loads, StatusLine } from './Overview.tsx';
import { useMachineData } from './data.ts';
import { openSettings, useHost } from './host.ts';
import { machineCommands } from './commands.ts';
import { BuildOutput, DiskSpace, FreeUp, HarnessData } from './Space.tsx';
import { LeftBehind, Using } from './Using.tsx';
import { alertOf, buildGroup, pctText, size, updatedLine } from './view.ts';
import './i18n/lazy.ts';

interface Ask {
  readonly title: string;
  readonly body: string;
  readonly ok: string;
  readonly run: () => void;
}

export function MachinePane(props: { readonly client: PluginClient; readonly peer?: PeerId }): JSX.Element {
  const t = props.client.t;
  const host = useHost(props.client);
  const data = useMachineData(props.client, props.peer ?? props.client.home, host.now);
  const [ask, setAsk] = createSignal<Ask | undefined>();
  const [tab, setTab] = createSignal('overview');
  const alert = createMemo(() => alertOf(data.snap()));
  const failed = (): boolean => data.status().kind === 'unreachable';
  const shared = { client: props.client, host, data, confirm: setAsk };
  const review = (filter: 'all' | 'left'): void => openSettings(host.navigate, 'worktrees', filter);
  // a reading as the page opens, never the last one a minute old (and the peer samples faster while it's read)
  const fresh = (): void => void props.client.run(machineCommands(data.peer).read, {}).catch(() => {});
  onMount(fresh);
  // the warning's one action: all the build output goes (nothing of anyone's work), and the Space tab shows it going
  const freeBuild = (): void => {
    const g = buildGroup(data.snap());
    setTab('space');
    if (!g) return;
    void props.client
      .run(machineCommands(data.peer).cleanBuild, { build: true, ids: g.ids, ...(g.leftovers.length && { leftovers: g.leftovers }) })
      .then(() => {
        host.toast.success(t('machine.build.going', { size: size(g.bytes) }));
        fresh();
      })
      .catch(() => {});
  };
  return (
    <Stack gap="8" part="machine.pane">
      <Inline gap="4" justify="between" part="machine.top">
        <Text size="sm" tone="muted" numeric part="machine.at">
          {updatedLine(data.snap()?.at, data.now(), failed(), t)}
        </Text>
        <ExtensionSlot input="machine.head" part="machine.head" extensions={[]} />
      </Inline>
      <Show when={alert()}>
        {(a) => {
          const x = a();
          return x.kind === 'disk' ? (
            <Callout
              tone={x.free < 2 * 1024 ** 3 ? 'bad' : 'warn'}
              title={t('machine.alert.disk', { free: size(x.free) })}
              part="machine.alert"
              action={
                <Button variant="primary" size="sm" onClick={freeBuild}>
                  {x.build > 0 ? t('machine.alert.freeBuild', { size: size(x.build) }) : t('machine.alert.space')}
                </Button>
              }
            >
              {x.build > 0 ? t('machine.alert.diskBuild', { size: size(x.build) }) : t('machine.alert.diskNone')}
            </Callout>
          ) : (
            <Callout tone="warn" title={t('machine.alert.memory', { pct: pctText(x.free) })} part="machine.alert">
              {t('machine.alert.memoryBody')}
            </Callout>
          );
        }}
      </Show>
      <Tabs
        value={tab()}
        onChange={setTab}
        aria-label={t('machine.tabs')}
        part="machine.tabs"
        items={[
          {
            value: 'overview',
            label: t('machine.tab.overview'),
            content: (
              <Stack gap="8">
                <Stack gap="4">
                  <StatusLine client={props.client} data={data} />
                  <Loads client={props.client} data={data} />
                </Stack>
                <LeftBehind {...shared} />
                <Using {...shared} />
                <DayChart client={props.client} data={data} />
              </Stack>
            ),
          },
          {
            value: 'space',
            label: t('machine.tab.space'),
            content: (
              <Stack gap="8">
                <DiskSpace client={props.client} data={data} />
                <BuildOutput {...shared} review={review} />
                <FreeUp {...shared} review={review} />
                <HarnessData {...shared} review={review} />
              </Stack>
            ),
          },
          {
            value: 'limits',
            label: t('machine.tab.limits'),
            content: <Automations client={props.client} host={host} data={data} limits={() => openSettings(host.navigate, 'usage', 'machine')} />,
          },
        ]}
      />
      <Show when={ask()}>
        {(a) => (
          <Dialog
            open
            role="alertdialog"
            size="sm"
            title={a().title}
            description={a().body}
            onOpenChange={(open) => !open && setAsk(undefined)}
            footer={
              <Inline gap="4" justify="end">
                <Button variant="ghost" onClick={() => setAsk(undefined)}>
                  {t('machine.ask.cancel')}
                </Button>
                <Button
                  variant="primary"
                  tone="bad"
                  onClick={() => {
                    a().run();
                    setAsk(undefined);
                  }}
                >
                  {a().ok}
                </Button>
              </Inline>
            }
          />
        )}
      </Show>
    </Stack>
  );
}

export default MachinePane;
