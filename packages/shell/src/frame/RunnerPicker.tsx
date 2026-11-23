// The harness picker (v1 runner-picker.js): ONE two-pane popover under the runner pill (the kit's HarnessPicker). Left:
// Auto ("Picks per task", where it's offered) and every harness with its logo and a sub-line; right: the focused one's
// pane: Auto's goals, or its models (search past six), thinking level and accounts; one that can't run yet signs in
// here (wisp.accounts' `shell.runner:signin` slot) or says it isn't set up. A harness press uses it at once and keeps the
// picker open; a model or goal pick commits and closes; thinking and account commit in place. Every pick shows at once
// (the caller's signals); Auto's goal is the router's setting (router.set), rolled back with a toast if refused.
// Runs on wisp.runtime's `harnesses` topic (ADR-0066): every harness at once, `Checking…` until its own detect lands,
// each row updated in place; `runtime.agents` is still asked once the box shows.
import { Show, createEffect, createMemo, createResource, createSignal, onCleanup, type JSX } from 'solid-js';
import { defineCommand, entityKey, type PeerId, type SettingRow } from '@wisp/client';
import { holdHarnesses, pickerDataOf } from './picker-data.ts';
import { HarnessPicker, Text, useSignal, type HarnessPickerPane, type HarnessPickerRow } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Slot } from '../slot.tsx';
import {
  HARNESSES_TOPIC,
  accountFor,
  effortsFor,
  keepOrder,
  harnessesFor,
  offerState,
  offersFrom,
  usedPct,
  type AccountOffer,
  type AgentOffer,
  type PlanWindow,
  type Runner,
  useHarnessRows,
} from './runner.ts';
import { AUTO, AUTO_GOALS, firstFocus, goalOf, modelGroups, pickerRows, searchShown, useHarness, varyRows, withModel, withSetting, withVary, type SubLine, type VaryKey } from './picker.ts';
import { SUBAGENT_NAME, type Variant } from './trial.ts';
import { AUTO_SETTING, FACTORY, autoEnabled } from './launch-pick.ts';
import { FACTORY_HOW_URL, factoryNames, factoryPane, factoryPick, factoryRow, useFactory } from './factory-pick.ts';
import '../i18n/lazy.ts';

/** v1 effortLabel: "X-High", else the level capitalised */
const levelLabel = (e: string): string => (e === 'xhigh' ? 'X-High' : e.charAt(0).toUpperCase() + e.slice(1));


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
  /** New task: a Factory row after the harnesses, whose pane lists the factories (ADR-0114) */
  readonly factories?: boolean;
  /** false: what it's for has no thinking level (a trigger) */ readonly efforts?: boolean;
  /** true: it can pick one of the harness's accounts (New task); elsewhere the run keeps its own */ readonly accounts?: boolean;
  /** where the caret goes once a pick closes it (the prompt) */
  readonly back?: () => HTMLElement | undefined;
  /** Try it two ways (a compare run, v1 runnerPicker `variant`): What's different under the pane of a harness that runs
   * variants (`can`); a segment commits in place, moving the run to that harness if it wasn't on it */
  /** what a harness's Default runs, by name, as the chip says it (useDefaultWords: the folder's answer, else this
   *  computer's; '' unknown). Without it the harnesses topic's word (QA r8 desktop #1: one answer for chip and row) */
  readonly defaultWord?: ((agent: string) => string) | undefined;
  readonly vary?:
    | {
        /** the settings `agent` can vary for the run as it would be there (`r`) */
        readonly can: (agent: string, r: Runner) => readonly string[];
        readonly value: Variant | undefined;
        readonly setting: string;
        readonly onChange: (v: Variant, r: Runner) => void;
      }
    | undefined;
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
  // each harness as its detect settles (ADR-0066), watched while the picker shows and a while after (perf-ui-r2 #3:
  // an open soon after finds it there); the answers are kept per peer the same way (picker-data.ts)
  createEffect(() => onCleanup(holdHarnesses(shell.client, props.peer, HARNESSES_TOPIC)));
  const data = createMemo(() => pickerDataOf(shell.client, props.peer));
  const topicRows = useHarnessRows(shell.client.entities, () => props.peer);
  // what's kept draws with the picker; an answer still to come (or a stale one asked again) fills it in
  const [answered, setAnswered] = createSignal<AgentOffer[] | undefined>(data().agentsNow());
  createEffect(() => {
    const d = data();
    void d.agents().then((v) => {
      if (d === data() && v !== answered()) setAnswered(() => v);
    });
  });
  const agents = createMemo(() => offersFrom(topicRows(), answered()));
  // how much of each plan is gone (v1 usageNote): what's kept draws with the rows; the rest asked together, one update
  const ready = (): string[] => agents().flatMap((a) => (a.available && !a.checking ? [a.id] : []));
  const [usage, setUsage] = createSignal<ReadonlyMap<string, PlanWindow | undefined>>(data().usageNow(ready()));
  const asked = new Set<string>(usage().keys());
  createEffect(() => {
    const ids = ready().filter((id) => !asked.has(id));
    if (ids.length === 0) return;
    for (const id of ids) asked.add(id);
    void data()
      .usage(ids)
      .then((got) => setUsage((m) => new Map([...m, ...got])));
  });
  // a row keeps its place as its harness settles (nothing jumps); set-up ones first on open
  let shown: string[] = [];
  const ordered = createMemo(() => {
    const out = keepOrder(shown, harnessesFor(agents(), ''));
    shown = [...shown, ...out.map((a) => a.id).filter((id) => !shown.includes(id))];
    return out;
  });
  // Auto's row only while its one switch is on (`runtime.auto`, owner #37), whoever asked for it
  const autoSwitch = useSignal(shell.client.entities.row<SettingRow>('setting', entityKey(props.peer as PeerId, AUTO_SETTING)));
  const offerAuto = (): boolean => props.auto === true && autoEnabled(autoSwitch()?.value);
  const rows = createMemo(() =>
    pickerRows(ordered(), {
      auto: offerAuto(),
      inUse: props.value.agent,
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
      case 'none':
        return '';
      default:
        return t('shell.runner.defaultModel');
    }
  };
  const varyLabel = (row: VaryKey, o: string): string =>
    row === 'skills' ? t(o === 'off' ? 'shell.try.off' : 'shell.try.on') : o ? (SUBAGENT_NAME[o] ?? o) : t('shell.try.taskModel');
  const varyTitle = (row: VaryKey, o: string): string =>
    row === 'skills' ? t(o === 'off' ? 'shell.try.offHint' : 'shell.try.onHint') : o ? t('shell.try.subOn', { name: SUBAGENT_NAME[o] ?? o }) : t('shell.try.subTask');
  // the harness whose pane shows: the one in use on open (v1 focusAgent), then wherever you arrow or press
  const [focus, setFocus] = createSignal<string | undefined>();
  const focused = (): string => focus() ?? (props.value.agent === FACTORY ? FACTORY : firstFocus(rows(), props.value.agent));
  // the factories, asked once the picker shows on New task; a wisp without wisp.factory has none (no row)
  const [factories] = createResource(
    () => (props.factories === true ? props.peer : false),
    (peer) => shell.client.commands.run(factoryNames, { peer }).then((v) => (Array.isArray(v) ? v : []), () => []),
  );
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
    () => (offerAuto() && focused() === AUTO ? props.peer : false),
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
  const [fetched] = createResource(wantsAccounts, async (agent) => ({ agent, list: await data().accounts(agent) }));
  const accounts = (): readonly AccountOffer[] => {
    const f = fetched.latest;
    return wantsAccounts() && f?.agent === focused() ? f.list : [];
  };
  /** the focused harness's pick: the one in use, else its defaults */
  const current = (): Runner => (focused() === props.value.agent ? props.value : { agent: focused(), model: '', effort: '', account: '' });
  const pane = createMemo((): HarnessPickerPane => {
    const id = focused();
    if (id === FACTORY)
      return factoryPane(
        factories() ?? [],
        props.value,
        t as never,
        (name) => (change(factoryPick(name)), closeBack()),
        () => (close(), window.open(FACTORY_HOW_URL, '_blank', 'noopener')),
      );
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
    // What's different: on the run's own harness its variant, on another what it would start from
    const vary = props.vary;
    const was = id === props.value.agent ? vary?.value : undefined;
    const varied = vary ? varyRows(vary.can(id, current()), was, vary.setting) : [];
    const accs = accounts();
    const accValue = accountFor(accs, current().account);
    const accNotes = accs.filter((x) => x.state === 'limit' || x.state === 'signedOut').map((x) => t(x.state === 'limit' ? 'shell.runner.accountAtLimit' : 'shell.runner.accountIsOut', { name: x.label }));
    return {
      ...base,
      groups: modelGroups(a, query()).map((g) => ({
        head: g.head,
        // the Default named by the model it runs when the harness says ("Opus (default)", UX 2026-11-18 #9)
        items: g.items.map((m) => ({ id: m.id, label: m.id === '' ? ((r) => (r ? t('shell.runner.defaultIs', { model: r }) : t('shell.runner.default')))(props.defaultWord?.(id) || a?.models.find((x) => x.id === '')?.resolves) : m.label, hint: m.id === '' ? (m.hint ?? t('shell.runner.asSet', { name })) : m.hint && m.hint !== g.head ? m.hint : undefined })),
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
          title: (e: string) => (e ? t('shell.runner.effortTitle', { level: levelLabel(e) }) : t('shell.runner.effortDefault')),
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
      ...(vary &&
        varied.length > 0 && {
          vary: {
            label: t('shell.try.differs'),
            rows: varied.map((r) => ({
              id: r.id,
              label: t(r.id === 'skills' ? 'shell.try.skills' : 'shell.try.subagents'),
              options: r.options.map((o) => ({ id: o, label: varyLabel(r.id, o), title: varyTitle(r.id, o) })),
              value: r.value,
              onPick: (o: string) => vary.onChange(withVary(was, r.id, o), current()),
            })),
          },
        }),
    };
  });
  const viewRows = createMemo((): HarnessPickerRow[] => [
    ...rows().map((r) => ({ id: r.id, name: r.name, auto: r.auto, logo: logoOf(r.id), sub: subText(r.sub), off: r.off })),
    ...((factories()?.length ?? 0) > 0 ? [factoryRow(t('shell.runner.factory'), t('shell.runner.factorySub'))] : []),
  ]);
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
        change(id === FACTORY ? useFactory(props.value) : useHarness(props.value, id, agents()));
      }}
      pane={pane()}
      onClose={close}
      // the harnesses that can run are listed; another is set up in Settings → Agents (UX 2026-11-18 #9)
      setup={{ label: t('shell.runner.setupAnother'), onClick: () => (close(), shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'agents' } } })) }}
    />
  );
}
