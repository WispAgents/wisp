// The harness · model picker (v1 runner-picker.js): the harnesses set up here, a search over every harness's models,
// the model list of the one picked, and its thinking level. A harness pick keeps the dialog open; a model pick
// closes it. Runs on wisp.runtime's `harnesses` topic (ADR-0066): every harness at once, `Checking…` until its own
// detect lands, each row updated in place; `runtime.agents` is still asked once the box shows (it detects again when
// the probes' memo has run out, and answers a wisp with no such topic).
import { Show, createEffect, createMemo, createResource, createSignal, onCleanup, type JSX } from 'solid-js';
import { defineCommand, ownerOf, type EntityKey, type PeerId } from '@wisp/client';
import { Dialog, Grid, Input, RadioGroup, Stack, Text, ThinkingSlider, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Slot } from '../slot.tsx';
import {
  HARNESSES_TOPIC,
  HARNESS_ROW,
  accountChoices,
  accountFor,
  effortsFor,
  fullest,
  harnessesFor,
  keepOrder,
  modelsFor,
  offerState,
  offersFrom,
  usedPct,
  type AccountOffer,
  type AgentOffer,
  type HarnessTopicRow,
  type PlanWindow,
  type Runner,
} from './runner.ts';

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

export function RunnerPicker(props: {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly peer: string;
  readonly value: Runner;
  readonly onChange: (r: Runner) => void;
  /** false: what it's for has no thinking level (a trigger) */ readonly efforts?: boolean;
  /** true: it can pick one of the harness's accounts (New task); elsewhere the run keeps its own */ readonly accounts?: boolean;
}): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const harnesses = useSignal(shell.harnesses());
  /** each harness's logo, as its plugin registered it */
  const logoOf = (id: string): string | undefined => harnesses().find((h) => h.id === id)?.logo;
  const [query, setQuery] = createSignal('');
  // each harness as its detect settles (ADR-0066), watched while the picker's box shows
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
  // asked when New task shows, not when the picker opens: it detects again once the probes' answers ran out
  const [answered] = createResource(
    () => props.peer,
    (peer) => shell.client.commands.run(agentsOf, { peer }).catch(() => [] as AgentOffer[]),
  );
  const agents = createMemo(() => offersFrom(topicRows(), answered()));
  /** the ready harnesses' ids, as one string: a row that changes in place doesn't ask the plans again */
  const readyIds = createMemo(() =>
    agents()
      .filter((a) => a.available && !a.checking)
      .map((a) => a.id)
      .join(','),
  );
  // before you pick, how much of each plan is gone (v1 runner-picker.js usageNote): asked when the picker opens, once
  // per harness as each turns ready (a harness that settles doesn't ask the others' plans again); again on the next open
  const [usage, setUsage] = createSignal<ReadonlyMap<string, PlanWindow | undefined>>(new Map());
  const asked = new Set<string>();
  createEffect(() => {
    if (!props.open) {
      asked.clear();
      return;
    }
    const peer = props.peer;
    for (const agent of readyIds() ? readyIds().split(',') : []) {
      if (asked.has(agent)) continue;
      asked.add(agent);
      void shell.client.commands
        .run(plansOf, { peer, agent })
        .catch(() => [])
        .then((w) => setUsage((m) => new Map(m).set(agent, fullest(w))));
    }
  });
  const usageHint = (id: string): string | undefined => {
    const w = usage().get(id);
    return w ? t('shell.runner.used', { label: w.label, pct: usedPct(w) }) : undefined;
  };
  // while the picker is open, a row keeps its place as its harness settles (nothing jumps); a fresh open sorts again
  let shown: string[] = [];
  const list = createMemo(() => {
    const next = harnessesFor(agents(), query());
    if (!props.open) {
      shown = [];
      return next;
    }
    const out = keepOrder(shown, next);
    shown = [...shown, ...out.map((a) => a.id).filter((id) => !shown.includes(id))];
    return out;
  });
  /** a harness that's there but signed out (v1 runner-picker.js signedOut): picked, it offers its sign-in */
  const signedOut = (id: string): boolean => agents().some((a) => a.id === id && a.signedIn === false);
  const models = createMemo(() => modelsFor(agents() ?? [], props.value.agent, query()));
  const efforts = createMemo(() => (props.efforts === false ? [] : effortsFor(agents() ?? [], props.value)));
  // the picked harness's accounts, asked while the picker is open (v1 runner-picker.js accountsAt): two or more show
  const wantsAccounts = (): false | { peer: string; agent: string } =>
    props.open && props.accounts === true && !signedOut(props.value.agent) && agents().some((a) => a.id === props.value.agent && a.available)
      ? { peer: props.peer, agent: props.value.agent }
      : false;
  const [fetched] = createResource(wantsAccounts, async (a) => ({
    agent: a.agent,
    list: (await shell.client.commands.run(accountsOf, a).catch(() => ({ accounts: [] as AccountOffer[] }))).accounts,
  }));
  // only the picked harness's own list: a resource keeps its last value while the next loads or once it's off
  const accounts = (): readonly AccountOffer[] => {
    const f = fetched.latest;
    return wantsAccounts() && f?.agent === props.value.agent ? f.list : [];
  };
  const accountRows = createMemo(() =>
    accountChoices(accounts(), { auto: t('shell.runner.accountAuto'), autoHint: t('shell.runner.accountAutoHint'), limit: t('shell.runner.accountLimit'), out: t('shell.runner.accountOut') }),
  );
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange} title={t('shell.runner.title')} size="md" part="shell.runner">
      <Stack gap="5" part="shell.runner.body">
        <Input type="search" icon="search" size="sm" value={query()} onInput={setQuery} placeholder={t('shell.runner.search')} aria-label={t('shell.runner.search')} />
        <Grid columns={2} gap="6" part="shell.runner.cols">
          <Show
            when={list().length > 0}
            fallback={
              <Text size="sm" tone="muted">
                {answered.loading ? t('shell.runner.loading') : t('shell.runner.none')}
              </Text>
            }
          >
            <RadioGroup
              label={t('shell.runner.harness')}
              options={list().map((a) => {
                const state = offerState(a);
                if (state === 'checking') {
                  // what was known still picks (a harness that was ready); one never seen waits for its answer
                  return { value: a.id, label: a.name, logo: logoOf(a.id), hint: t('shell.runner.checking'), ...(!a.available && a.signedIn !== false && { disabled: true }) };
                }
                const used = state === 'ready' ? usageHint(a.id) : undefined;
                const out = state === 'signedOut' && { hint: t('shell.runner.signedOut') };
                return { value: a.id, label: a.name, logo: logoOf(a.id), ...(state === 'missing' && { disabled: true, hint: t('shell.runner.notSetUp') }), ...out, ...(used && { hint: used }) };
              })}
              value={props.value.agent}
              onChange={(agent) => props.onChange({ agent, model: '', effort: '', account: '' })}
            />
          </Show>
          <Show when={signedOut(props.value.agent)}>
            {/* v1 paintSignIn: a signed-out harness signs in right here, in place of its models (wisp.accounts) */}
            <Slot input="shell.runner:signin" id={props.value.agent} layout="stack" />
          </Show>
          <Show when={models().length > 0 && !signedOut(props.value.agent)}>
            <RadioGroup
              label={t('shell.runner.model')}
              options={models().map((m) => ({ value: m.id, label: m.id === '' ? t('shell.runner.default') : m.label, ...(m.hint && { hint: m.hint }) }))}
              value={props.value.model}
              onChange={(model) => {
                props.onChange({ ...props.value, model });
                props.onOpenChange(false);
              }}
            />
          </Show>
        </Grid>
        <Show when={accountRows().length > 0}>
          {/* v1 runner-picker.js paintAccounts: Auto, then each account; a pick commits in place */}
          <RadioGroup
            label={t('shell.runner.account')}
            part="shell.runner.account"
            options={accountRows()}
            value={accountFor(accounts(), props.value.account)}
            onChange={(account) => {
              const label = accounts().find((a) => a.id === account)?.label;
              props.onChange({ ...props.value, account, ...(account && label && { accountLabel: label }) });
            }}
          />
        </Show>
        <Show when={efforts().length > 0}>
          {/* v1 runner-picker.js: Thinking is a slider from Default to the harness's top level */}
          <ThinkingSlider
            heading={t('shell.runner.thinking')}
            stops={['', ...efforts()]}
            value={props.value.effort}
            onChange={(effort) => props.onChange({ ...props.value, effort })}
            label={(e) => (e ? levelLabel(e) : t('shell.runner.default'))}
            hint={(e) => t(`shell.runner.hint.${e || 'default'}`)}
            aria={{ label: t('shell.runner.thinkingLevel'), valueText: (e) => t('shell.runner.thinkingIs', { level: e ? levelLabel(e) : t('shell.runner.default') }) }}
          />
        </Show>
      </Stack>
    </Dialog>
  );
}
