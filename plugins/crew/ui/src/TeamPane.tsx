// The lead's team in the panel beside the task (v1 team.js: the live line per delegate, then its report), F-crew-11:
// every crew job this task handed out, delegated or tagged, each row a member's face, @name, where it runs (this
// computer or its machine, v1 .tl-place), what it's doing or came back with, and its time: a running one ticks
// (v1 .elapsed), one that's back shows its state and how long it took (v1 .sub-dur, .pill). A click opens the job, a
// task of its own. A sideTabs pane: its tab shows only while the task has such jobs (later.ts).
import { createMemo, createSignal, createEffect, For, Show, onCleanup, type Accessor, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Button, Face, ListRow, Pill, Stack, TeamPlace, TeamTime, Text, useSignal } from '@wisp/ui';
import { crewData } from './data.ts';
import { t } from './i18n.ts';
import { shownOf, teamRows, timeOf, type Say, type TeamRow } from './team.ts';
import './i18n/lazy.ts';

/** v1 app.js: the running times tick once a second */
export const TICK_MS = 1000;

/** The rows, laid out. `now` is read only by each row's time, so a tick repaints those text nodes and nothing else. */
export function TeamList(props: { readonly rows: readonly TeamRow[]; readonly now: Accessor<number>; readonly open: (id: string) => void }): JSX.Element {
  // a long team folds to its first few (v1 c33f33e6): Show N more unfolds it, and it stays unfolded
  const [unfolded, setUnfolded] = createSignal(false);
  const shown = createMemo(() => (unfolded() ? undefined : shownOf(props.rows)));
  const visible = (): readonly TeamRow[] => {
    const keep = shown();
    return keep ? props.rows.filter((_, i) => keep.has(i)) : props.rows;
  };
  return (
    <Stack gap="1" part="crew.team">
      <Show when={props.rows.length} fallback={<Text tone="muted">{t('wisp.crew.team.none')}</Text>}>
        <For each={visible()}>
          {(r) => {
            const ms = (): number | undefined => timeOf(r, props.now());
            // whether it has a time at all: a boolean, so a tick doesn't rebuild the row's cells, only the time's text
            const timed = createMemo(() => ms() !== undefined);
            return (
              <ListRow
                part="crew.team.row"
                title={r.handle}
                lead={<Face avatar={r.avatar} name={r.name} color={r.color} size="sm" mood={r.live ? 'live' : 'idle'} />}
                sub={
                  <>
                    <TeamPlace part="crew.team.place" name={r.place.name} icon={r.place.icon} away={r.place.away} title={r.place.tip} />
                    {r.line}
                  </>
                }
                subTone={r.live ? 'accent-ink' : r.tone === 'warn' ? 'warn' : 'muted'}
                // v1 .tl-row: a working delegate shows its time, not a pill; one still being made, asking, back, or with no
                // start to tick from (a far wisp's older row) says its state
                pills={
                  <Show when={!r.live || r.pending || !timed()}>
                    <Pill tone={r.tone} dot live={r.live} size="sm">
                      {r.status}
                    </Pill>
                  </Show>
                }
                // no time, no time column (the sub keeps its width on a 320 px phone)
                {...(timed() && { time: <TeamTime part="crew.team.time" ms={ms() ?? 0} /> })}
                {...(!r.pending && { onOpen: () => props.open(r.id) })}
              />
            );
          }}
        </For>
        <Show when={shown()}>
          {(keep) => (
            <Button variant="link" size="xs" icon="chevron" part="crew.team.more" onClick={() => setUnfolded(true)}>
              {t('wisp.crew.team.more', { n: props.rows.length - keep().size })}
            </Button>
          )}
        </Show>
      </Show>
    </Stack>
  );
}

export default function TeamPane(props: { readonly client: PluginClient; readonly task: string; readonly value: string }): JSX.Element {
  const c = props.client;
  const say = t as unknown as Say;
  const data = crewData(c);
  const jobs = useSignal(data.jobs);
  const members = useSignal(data.members);
  const rows = createMemo(() => teamRows(jobs(), props.task, (id) => members().find((m) => m.id === id), say, c.home));
  // the clock ticks only while a job runs, on the pane's own scope (closing the tab stops it)
  const [now, setNow] = createSignal(c.scope.timers.now());
  const running = createMemo(() => rows().some((r) => r.running && !r.pending));
  createEffect(() => {
    if (!running()) return;
    setNow(c.scope.timers.now());
    const stop = c.scope.interval(TICK_MS, () => setNow(c.scope.timers.now()));
    onCleanup(stop);
  });
  const open = (id: string): void => {
    const key = data.sessionKey(id);
    if (key) c.navigate({ t: 'openTask', id: key as never });
  };
  return <TeamList rows={rows()} now={now} open={open} />;
}
