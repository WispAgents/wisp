// The PR row on the open task (v1 ship.js renderPrStrip, design/side-panel.md "Opening it"): the PR
// in its tone, its one line, and a bar of what to do next, Merge (or Fix conflicts) pinned at the
// end. Clicking the row opens the side panel on PR. While a ship is in flight it shows the stages
// instead; while the PR pane is open beside the task it hides, so the verdict never shows twice.

import { For, Match, Show, Switch, createEffect, createMemo, createSignal, type Accessor, type JSX } from 'solid-js';
import { CommandError, entityKey, readonly, type EntityKey, type PluginClient } from '@wisp/client';
import { mergeRequest } from './merge-request.ts';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Card, Icon, IconButton, ListRow, Stack, Switch as OnOff, Text, Toolbar, ToneChip, useSignal, Wrap, type ToolbarItem } from '@wisp/ui';
import { fixConflicts, refresh, review, setSteward } from './commands.ts';
import { t } from './i18n.ts';
import { useMerge, type MergeAsk } from './MergeConfirm.tsx';
import { fixing, openPrs, prChipTitle, prsOf, prTone, worstTone, type PrTone } from './model/pr.ts';
import { prStripSub, prStripTitle } from './model/strip.ts';
import { REVIEW_ICON, reviewActs, reviewState, type ReviewContext } from './model/review.ts';
import { chipRisk, RISK_ICON, RISK_TONE, riskLine, riskRow, verdictOf } from './model/risk.ts';
import { stepsOfLive, track } from './model/ship.ts';
import { stewardLine, stewardStatus, stewardSwitch } from './model/steward.ts';
import { ShipRun } from './ShipRun.tsx';
import { TONE, TONE_ICON } from './tones.ts';
import { useBusy } from './busy.ts';
import { openShip, useReviewCtx, useSession, useShip } from './use.ts';
import type { ConflictFix, RepoPr, ShipSession } from './types.ts';
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

/** Show up to 5 PR chips inline; beyond that collapse to 4 + a "+N more" toggle chip (never "+1 more"). */
const PR_CHIP_CAP = 5;
const PR_CHIP_PREVIEW = 4;

// size-ok: the row's bar is one list of items, each with its own condition from v1
function PrRow(props: { readonly client: PluginClient; readonly k: EntityKey; readonly s: ShipSession; readonly merge: MergeAsk; readonly ctx: Accessor<ReviewContext> }): JSX.Element {
  const ship = useShip(props.client, props.k);
  const merge = props.merge;
  const ctx = props.ctx;
  const checking = useBusy();
  const [expanded, setExpanded] = createSignal(false);
  // Settings → Ship's Steward mode: what the PR row's switch starts with on a PR that never had one
  const stewardSetting = useSignal(props.client.entities.row<{ value?: unknown }>('setting', entityKey(props.client.home, 'ship.steward')));
  const keep = createMemo(() => stewardSwitch(props.s, stewardSetting()?.value));
  const flipSteward = (): void => {
    const k = keep();
    if (k) quiet(props.client.run(setSteward, { key: props.k, mode: k.next }));
  };
  const prs = createMemo(() => prsOf(props.s));
  const several = (): boolean => prs().length > 1;
  // When every PR belongs to the task's own repo (already named above the card), chips show "#853" cleanly
  // instead of repeating the repo name on every pill; cross-repo tasks keep "repo" + non-truncating "#853".
  const sameRepo = createMemo((): boolean => !!props.s.project && prs().every((x) => !x.repo || x.repo === props.s.project));
  // wisp fixes conflicts on the task's own PR only
  const fixFor = (x: RepoPr): ConflictFix | undefined => (props.s.pr && x.pr === props.s.pr ? props.s.conflictFix : undefined);
  const tone = createMemo((): PrTone => worstTone(prs().map((x) => prTone(x.pr, fixFor(x)))));
  const open = createMemo(() => openPrs(props.s));
  const rv = () => (props.s.pr?.state === 'OPEN' ? reviewState(t, props.s, ctx()) : undefined);
  const risk = createMemo(() => riskRow(t, props.s, open()));
  // A single settled (merged/closed) PR has no active toolbar actions: put its GitHub link inline in the row
  // instead of opening a mostly empty 32px toolbar beneath it.
  const inlineGithub = createMemo((): string | undefined => {
    const first = prs()[0];
    if (!first || several() || open().length > 0 || rv()) return undefined;
    return first.pr.url;
  });
  // each PR's chip says its own merge risk ("#853 · high risk"), its title the judge's line
  const chipSuffix = (o: RepoPr): string | undefined => {
    const n = !sameRepo() && o.repo ? `#${o.pr.number}` : undefined;
    const r = o.pr.state === 'OPEN' ? chipRisk(t, props.s, o.pr) : undefined;
    return r ? [n, r].filter(Boolean).join(' · ') : n;
  };
  const chipTitle = (o: RepoPr): string => {
    const base = prChipTitle(t, o, fixFor(o));
    return props.s.riskOn && o.pr.state === 'OPEN' ? `${base}\n${riskLine(t, o, verdictOf(props.s, o.pr), false)}` : base;
  };
  const title = (): string => prStripTitle(t, prs());
  const sub = (): string => prStripSub(t, prs(), props.s.pr, props.s.conflictFix);
  const overflow = (): boolean => prs().length > PR_CHIP_CAP;
  const shown = createMemo((): readonly RepoPr[] => (overflow() && !expanded() ? prs().slice(0, PR_CHIP_PREVIEW) : prs()));
  const hidden = createMemo((): readonly RepoPr[] => (overflow() && !expanded() ? prs().slice(PR_CHIP_PREVIEW) : []));
  const hiddenTone = createMemo((): PrTone => worstTone(hidden().map((x) => prTone(x.pr, fixFor(x)))));
  const hiddenNeedsDot = (): boolean => hiddenTone() === 'fail' || hiddenTone() === 'pending';
  const hiddenLive = (): boolean => hidden().some((x) => !!x.pr.checks?.pending && x.pr.state === 'OPEN');
  const items = createMemo((): ToolbarItem[] => {
    const out: ToolbarItem[] = [];
    const list = prs();
    const first = list[0];
    if (!first) return out;
    if (open().length) {
      const check = (): void => void checking.run(props.client.run(refresh, { key: props.k }));
      // icon-only in both forms: the ring replaces the icon while it checks, the width holds
      out.push({
        id: 'refresh',
        icon: 'replay',
        label: t('wisp.ship.strip.check'),
        priority: 1,
        onSelect: check,
        render: () => <IconButton icon="replay" size="sm" label={t('wisp.ship.strip.checkTitle')} loading={checking.busy()} onClick={check} part="pr.strip.check" />,
      });
    }
    if (!several() && !inlineGithub()) out.push({ id: 'github', icon: 'external', label: t('wisp.ship.strip.github'), priority: 2, onSelect: () => ship.shell.platform.openExternal(first.pr.url) });
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
            <IconButton icon={REVIEW_ICON[v.kind]} size="sm" variant="ghost" {...(bad ? { tone: 'bad' as const } : {})} label={`${v.label}: ${v.title}`} onClick={go} part="pr.strip.review" />
          ) : (
            <Button size="sm" variant="ghost" icon={REVIEW_ICON[v.kind]} {...(bad ? { tone: 'bad' as const } : {})} title={v.title} onClick={go} part="pr.strip.review">
              {v.label}
            </Button>
          ),
      });
    }
    // Pinned primary action (Fix conflicts or Merge) always sits last so Toolbar pushes it cleanly to the end.
    if (props.s.pr?.state === 'OPEN' && props.s.pr.conflict && !fixing(props.s.conflictFix) && props.s.state !== 'running') {
      out.push({ id: 'fix', icon: 'merge', label: t('wisp.ship.action.fix'), priority: 'pin', onSelect: () => quiet(props.client.run(fixConflicts, { key: props.k })) });
    }
    if (open().length && !open().some((o) => o.pr.conflict)) {
      const label = open().length > 1 ? t('wisp.ship.strip.mergeAll') : t('wisp.ship.strip.merge');
      out.push({ id: 'merge', icon: 'merge', label, priority: 'pin', render: (form) => <MergeButton label={label} form={form} busy={merge.merging()} onClick={merge.ask} /> });
    }
    return out;
  });
  return (
    <Card part="pr.strip" tone={TONE[tone()]} pad="3">
      <Stack gap="1">
        <ListRow
          part="pr.strip.row"
          title={title()}
          sub={sub()}
          subTitle={sub()}
          lead={<Icon name={TONE_ICON[tone()]} size="md" tone={TONE[tone()] === 'accent' ? 'accent-ink' : TONE[tone()]} />}
          actions={
            <>
              <Show when={inlineGithub()}>
                {(url) => (
                  <IconButton
                    icon="external"
                    size="xs"
                    variant="ghost"
                    label={t('wisp.ship.strip.github')}
                    part="pr.strip.github"
                    onClick={(e) => {
                      e.stopPropagation();
                      ship.shell.platform.openExternal(url());
                    }}
                  />
                )}
              </Show>
              <Icon name="chevron" size="sm" tone="faint" />
            </>
          }
          onOpen={() => ship.openPane()}
        />
        {/* the Steward (ADR-0132) on its own row: what it is, what it's doing (or what it would do), and its switch;
            on a shipped PR and on one the agent attached (`wisp_pr`). Without an open PR, only a stop's reason stays. */}
        <Show when={keep()} fallback={<StewardNote s={props.s} />}>
          {(k) => (
            <ListRow
              part="pr.strip.keep"
              lead={<Icon name="judge" size="sm" tone={k().on ? 'accent' : 'faint'} />}
              title={t('wisp.ship.steward.title')}
              sub={stewardStatus(t, props.s) ?? t('wisp.ship.steward.off')}
              subTitle={t(k().on ? 'wisp.ship.steward.switchOff' : 'wisp.ship.steward.switchOn')}
              actions={<OnOff checked={k().on} onChange={flipSteward} aria-label={t('wisp.ship.steward.title')} title={t(k().on ? 'wisp.ship.steward.switchOff' : 'wisp.ship.steward.switchOn')} part="pr.strip.steward.switch" />}
            />
          )}
        </Show>
        {/* merge risk (ADR-0143) on its own row, built as the Steward's: the worst open PR's verdict in a line and its
            level on the right (every PR's line on hover); each PR's own word is on its chip, the reasons in the PR pane */}
        <Show when={risk()}>
          {(r) => (
            <ListRow
              part="pr.strip.risk"
              lead={<Icon name={RISK_ICON[r().kind]} size="sm" tone={RISK_TONE[r().kind] === 'muted' ? 'faint' : RISK_TONE[r().kind]} />}
              title={t('wisp.ship.risk.title')}
              sub={r().sub}
              subTitle={`${r().title}\n${t('wisp.ship.risk.open')}`}
              actions={
                <Text size="sm" weight="medium" tone={RISK_TONE[r().kind]} nowrap part="pr.strip.risk.level">
                  {r().level}
                </Text>
              }
              onOpen={() => ship.openPane()}
            />
          )}
        </Show>
        {/* every attached PR as a wrapping chip row under the summary (never ListRow's nowrap pills column, which
            squeezed the title and spilled chips past the card); bounded with "+N more" past 5 PRs */}
        <Show when={several()}>
          <Wrap gap="2" part="pr.strip.also">
            <For each={shown()}>
              {(o) => (
                <ToneChip
                  part="pr.strip.chip"
                  label={!sameRepo() && o.repo ? o.repo : `#${o.pr.number}`}
                  suffix={chipSuffix(o)}
                  tone={TONE[prTone(o.pr, fixFor(o))]}
                  live={(!!o.pr.checks?.pending || !!o.pr.waitingChecks) && o.pr.state === 'OPEN'}
                  title={chipTitle(o)}
                  onClick={() => ship.shell.platform.openExternal(o.pr.url)}
                />
              )}
            </For>
            <Show when={overflow() && !expanded()}>
              <ToneChip
                part="pr.strip.more"
                label={t('wisp.ship.strip.more', { count: hidden().length })}
                tone={TONE[hiddenTone()]}
                dot={hiddenNeedsDot()}
                live={hiddenLive()}
                title={t('wisp.ship.strip.moreTitle', { count: prs().length })}
                onClick={() => setExpanded(true)}
              />
            </Show>
          </Wrap>
          <Show when={overflow() && expanded()}>
            <Wrap gap="2" part="pr.strip.toggle">
              <Button variant="ghost" size="xs" part="pr.strip.more" onClick={() => setExpanded(false)}>
                {t('wisp.ship.strip.less')}
              </Button>
            </Wrap>
          </Show>
        </Show>
        <Show when={items().length > 0}>
          <Toolbar aria-label={t('wisp.ship.strip.actions')} items={items()} compact part="pr.strip.bar" />
        </Show>
      </Stack>
    </Card>
  );
}

/** A stopped Steward's reason after its PR merged or closed (no switch: nothing left to keep green). */
function StewardNote(props: { readonly s: ShipSession }): JSX.Element {
  return (
    <Show when={stewardLine(t, props.s)}>
      {(line) => (
        <Text as="p" size="sm" tone={props.s.steward?.step === 'stopped' ? 'warn' : 'muted'} part="pr.strip.steward">
          {line()}
        </Text>
      )}
    </Show>
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
