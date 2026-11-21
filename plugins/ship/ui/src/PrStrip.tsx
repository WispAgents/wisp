// The PR row on the open task (v1 ship.js renderPrStrip, design/side-panel.md "Opening it"): the PR
// in its tone, its one line, and a bar of what to do next, Merge (or Fix conflicts) pinned at the
// end. Clicking the row opens the side panel on PR. While a ship is in flight it shows the stages
// instead; while the PR pane is open beside the task it hides, so the verdict never shows twice.

import { Match, Show, Switch, createEffect, type Accessor, type JSX } from 'solid-js';
import { CommandError, readonly, type EntityKey, type PluginClient } from '@wisp/client';
import { mergeRequest } from './merge-request.ts';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Card, Icon, IconButton, ListRow, Stack, Toolbar, ToneChip, useSignal, Wrap, type ToolbarItem } from '@wisp/ui';
import { fixConflicts, refresh, review } from './commands.ts';
import { t } from './i18n.ts';
import { useMerge, type MergeAsk } from './MergeConfirm.tsx';
import { fixing, openPrs, prLine, prsOf, prTone, worstTone, type PrTone } from './model/pr.ts';
import { REVIEW_ICON, reviewActs, reviewState, type ReviewContext } from './model/review.ts';
import { stepsOfLive, track } from './model/ship.ts';
import { ShipRun } from './ShipRun.tsx';
import { TONE, TONE_ICON } from './tones.ts';
import { useBusy } from './busy.ts';
import { openShip, useReviewCtx, useSession, useShip } from './use.ts';
import type { ShipSession } from './types.ts';
import './i18n/lazy.ts';

export default function PrStrip(props: ExtensionProps): JSX.Element {
  const key = (): EntityKey | undefined => (props.id ?? props.session?.()?.id) as EntityKey | undefined;
  return (
    <Show when={key()} keyed>
      {(k) => <Strip client={props.client} k={k} />}
    </Show>
  );
}

/** Commands whose refusal the shell already toasts: the caller only stops waiting. */
export const quiet = (p: Promise<unknown>): void =>
  void p.catch((err: unknown) => {
    if (!(err instanceof CommandError)) throw err;
  });

function Strip(props: { readonly client: PluginClient; readonly k: EntityKey }): JSX.Element {
  const s = useSession(props.client, () => props.k);
  const ship = useShip(props.client, props.k);
  const run = useSignal(ship.flows.run);
  // through useSignal (round 324): the nav is an alien signal; read directly, the row didn't hide when the PR pane
  // opened beside the task after it showed, nor come back when the pane closed
  const nav = useSignal(ship.shell.nav);
  const paneOpen = (): boolean => nav().side.kind === 'pr' && (nav().side as { id?: string }).id === props.k;
  // Started here, or elsewhere (another tab, the API): the server's stage moves the track along.
  const live = (): ReturnType<typeof track> | undefined => {
    const r = run();
    const sh = s()?.shipping;
    if (r) return track(t, r.steps, r.error === undefined ? sh : undefined, r.error);
    return sh ? track(t, stepsOfLive(sh), sh, undefined) : undefined;
  };
  // Merge PR #n… from a task's menu (Ship ▾, the row's menu): asked here, whatever the strip shows, with the confirm the
  // PR row's Merge opens. Only the PR row took it, and it isn't there while a ship's stages show or the PR pane is open:
  // the ask was dropped (audit C #7).
  const ctx = useReviewCtx(props.client, s);
  const merge = useMerge(ship.flows, s, ctx, ship.shell);
  const asked = useSignal(readonly(mergeRequest));
  createEffect(() => {
    if (asked() !== props.k) return;
    mergeRequest(undefined);
    merge.ask();
  });
  return (
    <>
      <Show when={!paneOpen()}>
        <Switch>
          <Match when={live()}>{(tr) => <ShipRun track={tr()} onRetry={() => openShip(props.client, ship.shell, props.k, s())} onDismiss={() => ship.flows.dismiss()} />}</Match>
          <Match when={prsOf(s()).length > 0 && s()}>{(row) => <PrRow client={props.client} k={props.k} s={row()} merge={merge} ctx={ctx} />}</Match>
        </Switch>
      </Show>
      <merge.Confirm />
    </>
  );
}

// size-ok: the row's bar is one list of items, each with its own condition from v1
function PrRow(props: { readonly client: PluginClient; readonly k: EntityKey; readonly s: ShipSession; readonly merge: MergeAsk; readonly ctx: Accessor<ReviewContext> }): JSX.Element {
  const ship = useShip(props.client, props.k);
  const merge = props.merge;
  const ctx = props.ctx;
  const checking = useBusy();
  const prs = () => prsOf(props.s);
  const lead = () => prs()[0];
  const several = (): boolean => prs().length > 1;
  const fix = () => (props.s.pr ? props.s.conflictFix : undefined); // wisp fixes conflicts on the task's own PR only
  const tone = (): PrTone => worstTone(prs().map((x, i) => prTone(x.pr, i ? undefined : fix())));
  const open = () => openPrs(props.s);
  const rv = () => (props.s.pr?.state === 'OPEN' ? reviewState(t, props.s, ctx()) : undefined);
  const title = (): string => {
    const x = lead();
    return !x ? '' : several() ? `${x.repo ?? ''} #${x.pr.number}` : t('wisp.ship.tab.one', { n: x.pr.number });
  };
  const items = (): ToolbarItem[] => {
    const out: ToolbarItem[] = [];
    const x = lead();
    if (!x) return out;
    if (open().length) {
      const check = (): void => void checking.run(props.client.run(refresh, { key: props.k }));
      // icon-only in both forms: the ring replaces the icon while it checks, the width holds
      out.push({
        id: 'refresh',
        icon: 'replay',
        label: t('wisp.ship.strip.checkTitle'),
        priority: 1,
        onSelect: check,
        render: () => <IconButton icon="replay" size="sm" label={t('wisp.ship.strip.checkTitle')} loading={checking.busy()} onClick={check} part="pr.strip.check" />,
      });
    }
    if (!several()) out.push({ id: 'github', icon: 'external', label: t('wisp.ship.strip.github'), priority: 2, onSelect: () => ship.shell.platform.openExternal(x.pr.url) });
    if (x.pr.state === 'OPEN' && x.pr.conflict && props.s.pr && !fixing(fix()) && props.s.state !== 'running') {
      out.push({ id: 'fix', icon: 'merge', label: t('wisp.ship.action.fix'), priority: 'pin', onSelect: () => quiet(props.client.run(fixConflicts, { key: props.k })) });
    }
    const v = rv();
    if (v) {
      // a rerun the server didn't start says why (audit E5: the chip said "Not reviewed" before and after)
      const go = (): void =>
        reviewActs(v.kind)
          ? void props.client.run(review, { key: props.k }).then(
              (r) => {
                const why = (r as { why?: unknown } | null)?.why;
                if (typeof why === 'string' && why) ship.shell.toast.show(why);
              },
              () => {}, // the shell's onError toasts it
            )
          : ship.openPane();
      const bad = v.kind === 'tests' || v.kind === 'issues';
      out.push({
        id: 'review',
        icon: REVIEW_ICON[v.kind],
        label: v.label,
        priority: 3,
        ...(bad ? { tone: 'bad' as const } : {}),
        onSelect: go,
        // its title says why it isn't reviewed and that a click tries again (v1 reviewState's title, audit E5)
        render: (form) =>
          form === 'icon' ? (
            <IconButton icon={REVIEW_ICON[v.kind]} size="sm" variant="ghost" label={`${v.label}: ${v.title}`} onClick={go} part="pr.strip.review" />
          ) : (
            <Button size="sm" variant="ghost" icon={REVIEW_ICON[v.kind]} {...(bad ? { tone: 'bad' as const } : {})} title={v.title} onClick={go} part="pr.strip.review">
              {v.label}
            </Button>
          ),
      });
    }
    if (open().length && !open().some((o) => o.pr.conflict)) {
      const label = open().length > 1 ? t('wisp.ship.strip.mergeAll') : t('wisp.ship.strip.merge');
      out.push({ id: 'merge', icon: 'merge', label, priority: 'pin', render: (form) => <MergeButton label={label} form={form} busy={merge.merging()} onClick={merge.ask} /> });
    }
    return out;
  };
  return (
    <Card part="pr.strip" tone={TONE[tone()]} pad="3">
      <Stack gap="1">
        <ListRow
          part="pr.strip.row"
          title={title()}
          sub={prLine(t, lead()?.pr ?? { number: 0, url: '', state: 'OPEN' }, fix())}
          lead={<Icon name={TONE_ICON[tone()]} size="md" tone={TONE[tone()] === 'accent' ? 'accent-ink' : TONE[tone()]} />}
          actions={<Icon name="chevron" size="sm" tone="faint" />}
          onOpen={() => ship.openPane()}
        />
        {/* the other repos' PRs: a Wrap line under the row (v1 .pr-strip wraps), never the row's nowrap pills
            column, which in a narrow pane pushed the title out and spilled the chips past the card */}
        <Show when={several()}>
          <Wrap gap="2" part="pr.strip.also">
            {prs()
              .slice(1)
              .map((o) => (
                <ToneChip
                  label={`${o.repo ?? ''} #${o.pr.number}`}
                  tone={TONE[prTone(o.pr)]}
                  title={`${o.repo ?? ''}: ${t('wisp.ship.pill.title', { n: o.pr.number, line: prLine(t, o.pr) })}`}
                  onClick={() => ship.shell.platform.openExternal(o.pr.url)}
                />
              ))}
          </Wrap>
        </Show>
        <Toolbar aria-label={t('wisp.ship.strip.actions')} items={items()} compact part="pr.strip.bar" />
      </Stack>
    </Card>
  );
}

function MergeButton(props: { readonly label: string; readonly form: 'full' | 'icon'; readonly busy: boolean; readonly onClick: () => void }): JSX.Element {
  return (
    <Show when={props.form === 'full'} fallback={<IconButton icon="check" size="sm" label={props.label} loading={props.busy} onClick={() => props.onClick()} part="pr.strip.merge" />}>
      <Button variant="ghost" size="sm" icon="check" loading={props.busy} onClick={() => props.onClick()} part="pr.strip.merge">
        {props.label}
      </Button>
    </Show>
  );
}
