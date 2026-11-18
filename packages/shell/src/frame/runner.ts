// The harness · model picker's data (v1 runner-picker.js), DOM-free: what each harness offers (runtime.agents), the
// models matching what's typed (by label or id, the harness's name too), and the thinking levels the pick allows.

import { defineCommand, ownerOf, type Client, type EntityKey, type EntityReader, type PeerId } from '@wisp/client';
import { useDerived } from '@wisp/ui/pure';
import { createEffect, createSignal, onCleanup, type Accessor } from 'solid-js';

export interface AgentModel {
  readonly id: string;
  readonly label: string;
  readonly hint?: string;
  readonly efforts?: readonly string[];
  /** the default's (`id` ''): the model it runs, as the harness's own config names it ("Opus"), when it can tell */
  readonly resolves?: string;
}
export interface AgentOffer {
  readonly id: string;
  readonly name: string;
  readonly available: boolean;
  /** false: installed but signed out (runtime.agents `signedIn`); the picker offers its sign-in */
  readonly signedIn?: boolean | null;
  readonly models: readonly AgentModel[];
  readonly efforts: readonly string[];
  /** its detect hasn't answered yet (the `harnesses` topic, ADR-0066): the rest is what was known before */
  readonly checking?: boolean;
}

/** What `agent`'s Default runs, by name, when its harness can tell (HarnessModel.resolves, read from its own config by
 *  its plugin, never a run): "Opus"; else `fallback` ("Default"). UX 2026-11-18 #9: the chip names the real model. */
export function defaultWord(rows: readonly HarnessTopicRow[], agent: string, fallback: string): string {
  return rows.find((r) => r.id === agent)?.agent.models.find((m) => m.id === '')?.resolves || fallback;
}

/** What Default runs, by name, for `peer`'s harnesses in `cwd` (owner's delegate 2026-11-18: what the harness will
 *  run): the folder's answer (runtime.defaultModel, asked once per computer, harness and folder, and again for a new
 *  folder), else this computer's (the harnesses topic, watched passively: it never wakes a sleeping box), else
 *  `fallback`. So the pill names the model without the picker opening first. */
export function useDefaultWords(
  client: Pick<Client, 'scope' | 'peers' | 'entities' | 'commands'>,
  peer: () => string,
  cwd: () => string | undefined = () => undefined,
): (agent: string, fallback: string) => string {
  createEffect(() => {
    const scope = client.scope.child('default-models');
    client.peers.watch(peer() as PeerId, HARNESSES_TOPIC, scope, { passive: true });
    onCleanup(() => scope.dispose());
  });
  const rows = useHarnessRows(client.entities, peer);
  // answers by computer, harness and folder: undefined asked and waiting, null can't tell
  const [answers, setAnswers] = createSignal<ReadonlyMap<string, string | null>>(new Map());
  const asked = new Set<string>();
  const there = (agent: string): string | undefined => {
    const dir = cwd();
    if (!dir || !agent || agent === 'auto') return undefined;
    const on = peer();
    const key = `${on}\u0000${agent}\u0000${dir}`;
    if (!asked.has(key)) {
      asked.add(key);
      const put = (v: string | null): void => void setAnswers((m) => new Map([...m, [key, v]]));
      // the asking is its own chunk (default-model-ask.ts): the answer comes after a request anyway. A failure (the
      // chunk, the peer asleep) isn't an answer: it's asked again on the next look, the topic's word meanwhile
      void import('./default-model-ask.ts')
        .then((m) => m.askDefault(client, { peer: on, agent, cwd: dir }))
        .then((v) => put(v ?? null), () => void asked.delete(key));
    }
    return answers().get(key) ?? undefined;
  };
  return (agent, fallback) => there(agent) ?? defaultWord(rows(), agent, fallback);
}

/** wisp.runtime's `harnesses` topic (ADR-0066): every harness at once as checking, each as its detect lands. */
export const HARNESSES_TOPIC = 'plugin:wisp.runtime:harnesses';
export const HARNESS_ROW = 'runtime.harness';

/** A row on it (plugins/runtime/api/bindings/runtime/HarnessRow.ts). */
export interface HarnessTopicRow {
  readonly id: string;
  readonly ord: number;
  readonly state: 'checking' | 'installed' | 'missing';
  readonly agent: Omit<AgentOffer, 'checking'>;
}

/** `peer()`'s rows on the harnesses topic, following each row as its detect settles (Checking… → found or missing). Through
 * useDerived (round 324): a createMemo over the keys read each row once, so a row that settled after its key showed
 * stayed Checking… until the picker reopened. */
export function useHarnessRows(entities: Pick<EntityReader, 'keys' | 'row'>, peer: () => string): Accessor<HarnessTopicRow[]> {
  return useDerived(() =>
    entities
      .keys(HARNESS_ROW)()
      .filter((k) => ownerOf(k) === peer())
      .map((k) => entities.row<HarnessTopicRow>(HARNESS_ROW, k as EntityKey)() as HarnessTopicRow | undefined)
      .filter((r): r is HarnessTopicRow => r !== undefined),
  );
}

/** What the picker offers: the topic's rows in their order, each marked while it's checking; none yet (a wisp that
 * has no such topic): what `runtime.agents` answered. */
export function offersFrom(rows: readonly HarnessTopicRow[], answered: readonly AgentOffer[] | undefined): readonly AgentOffer[] {
  if (rows.length === 0) return answered ?? [];
  return [...rows].sort((a, b) => a.ord - b.ord).map((r) => ({ ...r.agent, id: r.id, ...(r.state === 'checking' && { checking: true }) }));
}

/** A harness row's state in the picker: still checking, ready, signed out (picked, it signs in) or not set up here. */
export type OfferState = 'checking' | 'ready' | 'signedOut' | 'missing';
export function offerState(a: Pick<AgentOffer, 'available' | 'signedIn' | 'checking'>): OfferState {
  if (a.checking) return 'checking';
  if (a.signedIn === false) return 'signedOut';
  return a.available ? 'ready' : 'missing';
}

/** The order rows show in while the picker is open: the ones already shown keep their place (a harness that settles
 * never jumps), new ones go after them; `next` is the order a fresh look would use. */
export function keepOrder(shown: readonly string[], next: readonly AgentOffer[]): AgentOffer[] {
  const at = new Map(shown.map((id, i) => [id, i]));
  return next
    .map((a, i) => ({ a, k: at.get(a.id) ?? shown.length + i }))
    .sort((x, y) => x.k - y.k)
    .map((x) => x.a);
}
export interface Runner {
  readonly agent: string;
  /** '' : the harness's default */
  readonly model: string;
  /** '' : the harness's default */
  readonly effort: string;
  /** one of the harness's accounts (wisp.accounts' `accounts.list` id); unset or '': Auto, your saved one */
  readonly account?: string;
  /** that account's name, for the bar's words (not sent) */
  readonly accountLabel?: string;
  /** the model's name, for the bar's words (not sent) */
  readonly modelLabel?: string;
}

/** The model's name a button shows (v1 paintRunnerBtn: the label of one of the harness's models, else `Default`): its
 * label when the pick knows it, '' (the harness's default) for no model or an id the harness doesn't list. */
export const modelWord = (r: { readonly model: string; readonly modelLabel?: string | undefined }): string => (r.model && r.modelLabel ? r.modelLabel : '');

/** One of a harness's accounts as `accounts.list` gives it (wisp.accounts' AccountView). */
export interface AccountOffer {
  readonly id: string;
  readonly label: string;
  readonly email?: string;
  readonly plan?: string;
  readonly default?: boolean;
  readonly state?: 'ready' | 'limit' | 'signedOut';
  readonly until?: number | null;
}

/** The account choice under the models (v1 runner-picker.js paintAccounts): Auto, then each account in your order,
 * only when the harness has two or more. A signed-out one can't be picked; one at its limit can (wisp never
 * switches for you), and says so. */
export function accountChoices(
  accounts: readonly AccountOffer[],
  words: { readonly auto: string; readonly autoHint: string; readonly limit: string; readonly out: string },
): { value: string; label: string; hint?: string; disabled?: boolean }[] {
  if (accounts.length < 2) return [];
  return [
    { value: '', label: words.auto, hint: words.autoHint },
    ...accounts.map((a) => {
      const line = [a.email, a.plan].filter(Boolean).join(' · ');
      if (a.state === 'signedOut') return { value: a.id, label: a.label, hint: words.out, disabled: true };
      if (a.state === 'limit') return { value: a.id, label: a.label, hint: words.limit };
      return { value: a.id, label: a.label, ...(line && { hint: line }) };
    }),
  ];
}

/** The account a task starts on: the pick while it's still one of the harness's and signed in, else Auto (''). */
export function accountFor(accounts: readonly AccountOffer[], want: string | undefined): string {
  if (!want || accounts.length < 2) return '';
  const a = accounts.find((x) => x.id === want);
  return a && a.state !== 'signedOut' ? a.id : '';
}

/** The models to show for `agent`, the Default first, narrowed by `query` (the harness's own name matches all). */
export function modelsFor(agents: readonly AgentOffer[], agent: string, query: string): AgentModel[] {
  const a = agents.find((x) => x.id === agent);
  if (!a) return [];
  const all = [{ id: '', label: 'Default' }, ...a.models.filter((m) => m.id !== '')];
  const q = query.trim().toLowerCase();
  if (!q || a.name.toLowerCase().includes(q)) return all;
  return all.filter((m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q));
}

/** Harnesses whose name or a model matches `query` (all of them when it's empty); set up ones first. */
export function harnessesFor(agents: readonly AgentOffer[], query: string): AgentOffer[] {
  const q = query.trim().toLowerCase();
  const hit = (a: AgentOffer): boolean => !q || a.name.toLowerCase().includes(q) || a.models.some((m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q));
  // a signed-out one can be picked to sign in, so it sits with the set-up ones
  // and one still checking (ADR-0066): most are there, and it shouldn't drop below the rest until it's known
  const usable = (a: AgentOffer): number => Number(a.available || a.signedIn === false || a.checking === true);
  return agents.filter(hit).sort((a, b) => usable(b) - usable(a));
}

/** The thinking levels of a pick: the model's own, else the harness's. */
export function effortsFor(agents: readonly AgentOffer[], r: Runner): readonly string[] {
  const a = agents.find((x) => x.id === r.agent);
  if (!a) return [];
  const m = a.models.find((x) => x.id === r.model);
  return m?.efforts?.length ? m.efforts : a.efforts;
}

/** The bar's words: "Claude Code · Opus", "Codex" (its default model), with the thinking level when set. */
export function runnerWords(agents: readonly AgentOffer[], r: Runner, name: (id: string) => string): string {
  const a = agents.find((x) => x.id === r.agent);
  const model = a?.models.find((m) => m.id === r.model)?.label ?? (r.model || '');
  return [name(r.agent), model, r.effort, r.account ? (r.accountLabel ?? r.account) : ''].filter(Boolean).join(' · ');
}

/** The run a task's next follow-up takes when the reply box picks none: its own harness, model and thinking (runtime
 *  keeps them, run.rs apply_opts). QA desktop r3 #1: the screen said "Default" while the task ran on Opus. */
export function taskRunner(s: { readonly agent: string; readonly summary: Readonly<Record<string, unknown>> }): Runner {
  const str = (v: unknown): string => (typeof v === 'string' ? v : '');
  return { agent: s.agent, model: str(s.summary['model']), effort: str(s.summary['effort']) };
}

/** A thinking level in words: "High", "X-High"; '' for the harness's own. */
export const effortWord = (e: string): string => (e === 'xhigh' ? 'X-High' : e ? e.charAt(0).toUpperCase() + e.slice(1) : '');

/** "Claude Code · opus · High": a run in words, `defaultModel` where it picked none. */
export const runWords = (name: string, r: Pick<Runner, 'model' | 'effort'>, defaultModel: string): string => [name, r.model || defaultModel, effortWord(r.effort)].filter(Boolean).join(' · ');

/** A reply's own harness · model (the reply box's pick) as runtime.followUp's `run`; none: the task's own. The pick names
 *  its model and thinking, '' for its harness's default (runtime run.rs apply_opts takes them as named). */
export function runPick(r: Runner | undefined): { agent: string; model: string; effort: string } | undefined {
  return r ? { agent: r.agent, model: r.model, effort: r.effort } : undefined;
}

/** A plan's usage window as `budget.view` gives it (0–1 used). */
export interface PlanWindow {
  readonly label: string;
  readonly used: number;
  readonly resetsAt?: number | null;
}

/** How much of a harness's plan is gone (v1 runner-picker.js fullest): its fullest window over its accounts. */
export function fullest(accounts: readonly { readonly windows: readonly PlanWindow[] }[]): PlanWindow | undefined {
  let out: PlanWindow | undefined;
  for (const a of accounts) for (const w of a.windows) if (Number.isFinite(w.used) && (!out || w.used > out.used)) out = w;
  return out;
}

/** v1's quiet line under a harness: "5h 42% used" (whole percents, rounded down). */
export const usedPct = (w: PlanWindow): number => Math.floor(w.used * 100);

/** A harness's plan windows per account (wisp.budget's `budget.view`); quiet: no budget plugin, no ring, no toast. */
export const planWindows = defineCommand<{ peer: string; agent: string }, { account?: string; windows: PlanWindow[] }[]>({
  id: 'budget.view',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the harness chip\'s usage ring',
  quiet: true,
});

/** The harness chip's usage ring (UX 2026-11-18 #8): only once its fullest window passes 80%; past 95% it's nearly out. */
export function usageRing(w: PlanWindow | undefined): { pct: number; tone: 'warn' | 'bad'; label: string } | undefined {
  if (!w || !Number.isFinite(w.used) || w.used < 0.8) return undefined;
  const pct = Math.min(100, usedPct(w));
  return { pct, tone: pct >= 95 ? 'bad' : 'warn', label: w.label };
}

/** The ring for a harness's accounts as `budget.view` gives them: `account` the picked one ('': all, the fullest). */
export function ringOf(
  accounts: readonly { readonly account?: string; readonly windows: readonly PlanWindow[] }[] | undefined,
  account: string,
): { pct: number; tone: 'warn' | 'bad'; label: string } | undefined {
  if (!accounts) return undefined;
  return usageRing(fullest(account ? accounts.filter((a) => a.account === account) : accounts));
}
