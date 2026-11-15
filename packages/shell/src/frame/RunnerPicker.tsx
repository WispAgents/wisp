// The harness picker (v1 runner-picker.js): ONE two-pane popover under the runner pill (the kit's HarnessPicker). Left:
// Auto ("Picks per task", where it's offered) and every harness with its logo and a sub-line; right: the focused one's
// pane: Auto's goals, or its models (search past six), thinking level and accounts; one that can't run yet signs in
// here (wisp.accounts' `shell.runner:signin` slot) or says it isn't set up. A harness press uses it at once and keeps the
// picker open; a model or goal pick commits and closes; thinking and account commit in place. Every pick shows at once
// (the caller's signals); Auto's goal is the router's setting (router.set), rolled back with a toast if refused.
// Runs on wisp.runtime's `harnesses` topic (ADR-0066): every harness at once, `Checking…` until its own detect lands,
// each row updated in place; `runtime.agents` is still asked once the box shows.
import { Show, createEffect, createMemo, createResource, createSignal, onCleanup, type JSX } from 'solid-js';
import { defineCommand, ownerOf, type EntityKey, type PeerId } from '@wisp/client';
import { HarnessPicker, Text, useSignal, type HarnessPickerPane, type HarnessPickerRow } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Slot } from '../slot.tsx';
import {
  HARNESSES_TOPIC,
  HARNESS_ROW,
  accountFor,
  effortsFor,
  fullest,
  keepOrder,
  harnessesFor,
  offerState,
  offersFrom,
  usedPct,
  type AccountOffer,
  type AgentOffer,
  type HarnessTopicRow,
  type PlanWindow,
  type Runner,
} from './runner.ts';
import { AUTO, AUTO_GOALS, firstFocus, goalOf, modelGroups, pickerRows, searchShown, useHarness, withModel, withSetting, type SubLine } from './picker.ts';
import '../i18n/lazy.ts';

/** v1 effortLabel: "X-High", else the level capitalised */
const levelLabel = (e: string): string => (e === 'xhigh' ? 'X-High' : e.charAt(0).toUpperCase() + e.slice(1));

const agentsOf = defineCommand<{ peer: string }, AgentOffer[]>({ id: 'runtime.agents', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the harness list' });

const plansOf = defineCommand<{ peer: string; agent: string }, { windows: PlanWindow[] }[]>({
  id: 'budget.view',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the plan windows',
});

/** A harness's accounts (wisp.accounts); refused or absent: none, and the picker shows no account choice. */
const accountsOf = defineCommand<{ peer: string; agent: string }, { accounts: AccountOffer[] }>({
  id: 'accounts.list',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the account list',
});

/** Auto's goal is the router's (Settings → Auto shows the same): the pick shows at once, a refusal puts it back. */
const autoGoal = defineCommand<{ peer: string; goal: string }, unknown>({
  id: 'router.set',
  owner: (a) => a.peer,
  wire: (a) => ({ goal: a.goal }),
  optimistic: 'none',
  pending: 'the Auto goal',
});
/** Auto's goal as the router has it, read once when Auto's pane first shows. */
const autoView = defineCommand<{ peer: string }, { goal?: string }>({ id: 'router.view', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'Auto' });

export interface RunnerPickerProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  /** the pill it opens from (the picker sits under it, or above when there's no room) */
  readonly anchor: () => Element | null | undefined;
  readonly peer: string;
  /** `agent: 'auto'` with `model` the goal ('' : the router's) when Auto is the pick */
  readonly value: Runner;
  readonly onChange: (r: Runner) => void;
  /** Auto is offered first (New task); elsewhere a harness is picked */
  readonly auto?: boolean;
  /** false: what it's for has no thinking level (a trigger) */ readonly efforts?: boolean;
  /** true: it can pick one of the harness's accounts (New task); elsewhere the run keeps its own */ readonly accounts?: boolean;
  /** where the caret goes once a pick closes it (the prompt) */
  readonly back?: () => HTMLElement | undefined;
}

export function RunnerPicker(props: RunnerPickerProps): JSX.Element {
  return (
    <Show when={props.open}>
      <PickerOpen {...props} />
    </Show>
  );
}

function PickerOpen(props: RunnerPickerProps): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const harnesses = useSignal(shell.harnesses());
  const logoOf = (id: string): string | undefined => harnesses().find((h) => h.id === id)?.logo;
  // each harness as its detect settles (ADR-0066), watched while the picker shows
  createEffect(() => {
    const scope = shell.client.scope.child('runner-harnesses');
    shell.client.peers.watch(props.peer as PeerId, HARNESSES_TOPIC, scope);
    onCleanup(() => scope.dispose());
  });
  const rowKeys = useSignal(shell.client.entities.keys(HARNESS_ROW));
  const topicRows = createMemo((): HarnessTopicRow[] =>
    rowKeys()
      .filter((k) => ownerOf(k) === props.peer)
      .map((k) => shell.client.entities.row<HarnessTopicRow>(HARNESS_ROW, k as EntityKey)() as HarnessTopicRow | undefined)
      .filter((r): r is HarnessTopicRow => r !== undefined),
  );
  const [answered] = createResource(
    () => props.peer,
    (peer) => shell.client.commands.run(agentsOf, { peer }).catch(() => [] as AgentOffer[]),
  );
  const agents = createMemo(() => offersFrom(topicRows(), answered()));
  // how much of each plan is gone (v1 usageNote): once per harness as each turns ready
  const [usage, setUsage] = createSignal<ReadonlyMap<string, PlanWindow | undefined>>(new Map());
  const asked = new Set<string>();
  createEffect(() => {
    for (const a of agents()) {
      if (!a.available || a.checking || asked.has(a.id)) continue;
      asked.add(a.id);
      void shell.client.commands
        .run(plansOf, { peer: props.peer, agent: a.id })
        .catch(() => [])
        .then((w) => setUsage((m) => new Map(m).set(a.id, fullest(w))));
    }
  });
  // a row keeps its place as its harness settles (nothing jumps); set-up ones first on open
  let shown: string[] = [];
  const ordered = createMemo(() => {
    const out = keepOrder(shown, harnessesFor(agents(), ''));
    shown = [...shown, ...out.map((a) => a.id).filter((id) => !shown.includes(id))];
    return out;
  });
  const rows = createMemo(() =>
    pickerRows(ordered(), {
      auto: props.auto === true,
      autoName: t('shell.home.auto'),
      used: (id) => {
        const w = usage().get(id);
        return w ? { label: w.label, pct: usedPct(w) } : undefined;
      },
    }),
  );
  const subText = (s: SubLine): string => {
    switch (s.k) {
      case 'auto':
        return t('shell.runner.picks');
      case 'checking':
        return t('shell.runner.checking');
      case 'signedOut':
        return t('shell.runner.signedOutShort');
      case 'missing':
        return t('shell.runner.notInstalled');
      case 'used':
        return t('shell.runner.used', { label: s.label, pct: s.pct });
      case 'models':
        return s.n === 1 ? t('shell.runner.oneModel') : t('shell.runner.models', { n: s.n });
      default:
        return t('shell.runner.defaultModel');
    }
  };
  // the harness whose pane shows: the one in use on open (v1 focusAgent), then wherever you arrow or press
  const [focus, setFocus] = createSignal<string | undefined>();
  const focused = (): string => focus() ?? firstFocus(rows(), props.value.agent);
  const offer = (): AgentOffer | undefined => agents().find((a) => a.id === focused());
  const [query, setQuery] = createSignal('');
  const preview = (id: string): void => {
    if (id !== focused()) setQuery('');
    setFocus(id);
  };
  const close = (): void => props.onOpenChange(false);
  const closeBack = (): void => {
    close();
    (props.back?.() ?? (props.anchor() as HTMLElement | null | undefined))?.focus?.();
  };
  const change = (r: Runner | undefined): void => {
    if (r) props.onChange(r);
  };
  // Auto's goal: the router's, read once Auto's pane shows (the pick's own wins)
  const [seenGoal] = createResource(
    () => (props.auto === true && focused() === AUTO ? props.peer : false),
    (peer) => shell.client.commands.run(autoView, { peer }).then((v) => v.goal, () => undefined),
  );
  const pickGoal = (goal: string): void => {
    const was = props.value;
    change(withModel(was, AUTO, goal, agents()));
    closeBack();
    void shell.client.commands.run(autoGoal, { peer: props.peer, goal }).catch(() => {
      // only if it's still that pick: a later one stays
      if (props.value.agent === AUTO && props.value.model === goal) props.onChange(was);
      shell.toast.show(t('shell.runner.goalRefused'));
    });
  };
  const signedOut = (): boolean => offer() !== undefined && offerState(offer() as AgentOffer) === 'signedOut';
  const missing = (): boolean => offer() !== undefined && offerState(offer() as AgentOffer) === 'missing';
  // the focused harness's accounts (v1 accountsAt): two or more show
  // a string, not an object: a harness elsewhere settling (a new agents() array) doesn't ask the accounts again
  const wantsAccounts = (): false | string => (props.accounts === true && focused() !== AUTO && offer()?.available === true && !signedOut() ? focused() : false);
  const [fetched] = createResource(wantsAccounts, async (agent) => ({
    agent,
    list: (await shell.client.commands.run(accountsOf, { peer: props.peer, agent }).catch(() => ({ accounts: [] as AccountOffer[] }))).accounts,
  }));
  const accounts = (): readonly AccountOffer[] => {
    const f = fetched.latest;
    return wantsAccounts() && f?.agent === focused() ? f.list : [];
  };
  /** the focused harness's pick: the one in use, else its defaults */
  const current = (): Runner => (focused() === props.value.agent ? props.value : { agent: focused(), model: '', effort: '', account: '' });
  const pane = createMemo((): HarnessPickerPane => {
    const id = focused();
    if (id === AUTO) {
      return {
        label: t('shell.runner.goal'),
        groups: [{ head: t('shell.runner.goal'), items: AUTO_GOALS.map((g) => ({ id: g, label: t(`shell.auto.goal.${g}`), hint: t(`shell.runner.goalHint.${g}`) })) }],
        value: props.value.agent === AUTO && props.value.model ? goalOf(props.value.model) : goalOf(seenGoal()),
        onPick: pickGoal,
        empty: t('shell.runner.none'),
        note: {
          text: t('shell.runner.autoNote'),
          link: t('shell.auto.how'),
          onLink: () => {
            close();
            shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'auto' } } });
          },
        },
      };
    }
    const a = offer();
    const name = a?.name ?? id;
    const base = { label: t('shell.runner.model'), value: current().model, empty: t('shell.runner.noModels'), onPick: (m: string) => (change(withModel(props.value, id, m, agents())), closeBack()) };
    if (signedOut()) return { ...base, groups: [], body: <Slot input="shell.runner:signin" id={id} layout="stack" /> };
    if (missing())
      return {
        ...base,
        groups: [],
        body: (
          <>
            <Text size="sm" weight="semibold">
              {t('shell.runner.notInstalledHead', { name })}
            </Text>
            <Text size="sm" tone="muted">
              {t('shell.runner.notInstalledTip', { name })}
            </Text>
          </>
        ),
      };
    const levels = props.efforts === false ? [] : effortsFor(agents(), current());
    const accs = accounts();
    const accValue = accountFor(accs, current().account);
    const accNotes = accs.filter((x) => x.state === 'limit' || x.state === 'signedOut').map((x) => t(x.state === 'limit' ? 'shell.runner.accountAtLimit' : 'shell.runner.accountIsOut', { name: x.label }));
    return {
      ...base,
      groups: modelGroups(a, query()).map((g) => ({
        head: g.head,
        items: g.items.map((m) => ({ id: m.id, label: m.id === '' ? t('shell.runner.default') : m.label, hint: m.id === '' ? (m.hint ?? t('shell.runner.asSet', { name })) : m.hint && m.hint !== g.head ? m.hint : undefined })),
      })),
      ...(searchShown(a) && { search: { placeholder: t('shell.runner.searchModels', { name }), value: query(), onInput: setQuery } }),
      ...(levels.length > 0 && {
        thinking: {
          heading: t('shell.runner.thinking'),
          stops: ['', ...levels],
          value: current().effort,
          onChange: (effort: string) => change(withSetting(props.value, id, { effort })),
          label: (e: string) => (e ? levelLabel(e) : t('shell.runner.default')),
          hint: (e: string) => t(`shell.runner.hint.${e || 'default'}`),
          aria: { label: t('shell.runner.thinkingLevel'), valueText: (e: string) => t('shell.runner.thinkingIs', { level: e ? levelLabel(e) : t('shell.runner.default') }) },
        },
      }),
      ...(accs.length >= 2 && {
        accounts: {
          label: t('shell.runner.account'),
          options: [
            { id: '', label: t('shell.runner.accountAuto'), title: t('shell.runner.accountAutoHint') },
            ...accs.map((x) => ({ id: x.id, label: x.label, off: x.state === 'signedOut', title: [x.email, x.plan].filter(Boolean).join(' · ') })),
          ],
          value: accValue,
          onPick: (account: string) => change(withSetting(props.value, id, { account, ...(account && { accountLabel: accs.find((x) => x.id === account)?.label ?? account }) })),
          ...(accNotes.length > 0 && { note: accNotes.join(' ') }),
        },
      }),
    };
  });
  const viewRows = createMemo((): HarnessPickerRow[] =>
    rows().map((r) => ({ id: r.id, name: r.name, auto: r.auto, logo: logoOf(r.id), sub: subText(r.sub), off: r.off })),
  );
  return (
    <HarnessPicker
      anchor={props.anchor}
      label={t('shell.runner.title')}
      head={t('shell.runner.harness')}
      rows={viewRows()}
      inUse={props.value.agent}
      inUseLabel={t('shell.runner.inUse')}
      focused={focused()}
      onPreview={preview}
      onUse={(id) => {
        preview(id);
        change(useHarness(props.value, id, agents()));
      }}
      pane={pane()}
      onClose={close}
    />
  );
}
