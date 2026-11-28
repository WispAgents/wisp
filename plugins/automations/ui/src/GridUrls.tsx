// A webhook trigger's grid URLs (ADR-0147), in its form once it's saved: `https://<computer>--<handle>.<grid>/_hook/wgh_…`,
// made for this computer by wisp.grid-client (the grid keeps only the token's hash). Make shows the new URL once, here
// and nowhere else (no store row, no draft, no storage), until Done or the form closes; the list shows each live URL by
// its last four characters, and Revoke takes it out at once (a refusal brings it back). Not on the grid, or signed out:
// the command's own words say what to do.
import { createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import { defineCommand, type PeerId, type PluginClient } from '@wisp/client';
import { useHere, useShell } from '@wisp/shell';
import { Button, Code, englishT, Inline, Stack, Text, timeLabel, Wrap } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export interface GridUrl {
  readonly id: string;
  readonly trigger: string;
  readonly computer: string;
  readonly hint: string;
  readonly createdAt: number;
  readonly lastUsedAt: number | null;
}

const list = defineCommand<{ peer: PeerId; trigger: string }, GridUrl[]>({
  id: 'grid-client.hooks',
  owner: (a) => a.peer,
  wire: (a) => ({ trigger: a.trigger }),
  optimistic: 'none',
  pending: 'the URL list',
});
const create = defineCommand<{ peer: PeerId; trigger: string }, GridUrl & { readonly url: string }>({
  id: 'grid-client.hooksCreate',
  owner: (a) => a.peer,
  wire: (a) => ({ trigger: a.trigger }),
  optimistic: 'none',
  pending: 'the Make button (Making…)',
});
const revoke = defineCommand<{ peer: PeerId; id: string }>({ id: 'grid-client.hooksRevoke', owner: (a) => a.peer, wire: (a) => ({ id: a.id }), optimistic: 'none', pending: 'the row, gone at once' });

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

export function GridUrls(props: { readonly client: PluginClient; readonly trigger: string }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const here = useHere();
  const [rows, setRows] = createSignal<readonly GridUrl[] | undefined>();
  const [failed, setFailed] = createSignal('');
  const [making, setMaking] = createSignal(false);
  const [fresh, setFresh] = createSignal<string | undefined>();
  // the form closed, however: the URL goes with it
  onCleanup(() => setFresh(undefined));
  const when = (at: number): string => timeLabel(at, c.scope.timers.now(), englishT);

  const load = (): void => {
    c.run(list, { peer: c.home, trigger: props.trigger }).then(
      (v) => {
        setFailed('');
        setRows(v);
      },
      (e: unknown) => setFailed(message(e)),
    );
  };
  load();

  const make = (): void => {
    setMaking(true);
    c.run(create, { peer: c.home, trigger: props.trigger }).then(
      (made) => {
        setMaking(false);
        setFresh(made.url);
        const { url: _, ...row } = made;
        setRows((l) => [row, ...(l ?? [])]);
      },
      (e: unknown) => {
        setMaking(false);
        setFailed(message(e));
      },
    );
  };

  const drop = (u: GridUrl): void => {
    const before = rows();
    setRows((l) => (l ?? []).filter((x) => x.id !== u.id));
    c.run(revoke, { peer: c.home, id: u.id }).then(
      () => shell.toast.show(t('automations.form.gridUrl.revoked')),
      () => {
        setRows(before);
        load();
      },
    );
  };

  const copy = (url: string): void => void shell.platform.copy(url).then((ok) => ok && shell.toast.show(t('automations.form.gridUrl.copied')));

  return (
    <Stack gap="2" part="automations.hookUrl">
      <Show when={fresh()}>
        {(url) => (
          <Stack gap="2" part="automations.form.gridUrl.new">
            <Code block>{url()}</Code>
            <Text tone="muted">{t(here.hub() ? 'automations.form.gridUrl.onceHub' : 'automations.form.gridUrl.once')}</Text>
            <Inline gap="2">
              <Button size="sm" variant="primary" icon="copy" part="automations.form.gridUrl.copy" onClick={() => copy(url())}>
                {t('automations.form.gridUrl.copy')}
              </Button>
              <Button size="sm" variant="ghost" part="automations.form.gridUrl.done" onClick={() => setFresh(undefined)}>
                {t('automations.form.gridUrl.done')}
              </Button>
            </Inline>
          </Stack>
        )}
      </Show>
      <For each={rows() ?? []}>
        {(u) => (
          <Wrap gap="2" align="center" part="automations.form.gridUrl.row">
            <Code>{`wgh_…${u.hint}`}</Code>
            <Text tone="muted">
              {u.lastUsedAt ? t('automations.form.gridUrl.used', { made: when(u.createdAt), used: when(u.lastUsedAt) }) : t('automations.form.gridUrl.made', { made: when(u.createdAt) })}
            </Text>
            <Button size="xs" variant="ghost" icon="trash" part="automations.form.gridUrl.revoke" onClick={() => drop(u)}>
              {t('automations.form.gridUrl.revoke')}
            </Button>
          </Wrap>
        )}
      </For>
      <Show when={failed()}>{(why) => <Text tone="muted">{why()}</Text>}</Show>
      <Inline gap="2">
        <Button size="sm" variant="ghost" icon="plus" part="automations.form.gridUrl.make" loading={making()} disabled={making()} onClick={make}>
          {t(making() ? 'automations.form.gridUrl.making' : 'automations.form.gridUrl.make')}
        </Button>
      </Inline>
    </Stack>
  );
}
