// Ask near the limit in the prompt boxes (v1 features/hold.js, limit-hold.js; ADR-0091): before New task or a follow-up
// goes, the box asks budget.ask whether it would take its account's window past your line (Settings → Usage → Ask near
// the limit); if so the box shows the row with this sentence instead of sending, and the draft stays. The server asks
// the same (the `budget_hold` refusal of runtime.create / runtime.followUp) when the box couldn't tell.
import { defineCommand, type PeerId, type PluginClient } from '@wisp/client';
import type { SendHold, SendHoldAsk } from '@wisp/shell/contract';
import { accountNames } from './commands.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

/** budget.ask's answer (wisp_budget_api::AskHold). */
export interface AskHold {
  readonly agent: string;
  /** `default` when the harness has one */
  readonly account: string;
  readonly window: string;
  readonly label: string;
  /** 0..1 */
  readonly used: number;
  /** your line, 0..1 */
  readonly line: number;
  /** where this send would likely take it, low and high */
  readonly to: readonly [number, number];
  /** ms since the epoch */
  readonly resetsAt?: number;
  readonly why: string;
}

/** The refusal reason the server-side guard gives (wisp_budget_api::VETO_CODE). */
export const HOLD_REASON = 'budget_hold';

export const ask = defineCommand<{ peer: PeerId; agent: string; account?: string; session?: string }, AskHold | null>({
  id: 'budget.ask',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent, ...(a.account && { account: a.account }), ...(a.session && { session: a.session }) }),
  optimistic: 'none',
  pending: 'the usage check',
});

/** v1 runners.js agentName for the harnesses with plans. */
const NAMES: Record<string, string> = { claude: 'Claude Code', codex: 'Codex' };

const pct = (x: number): string => `${Math.floor(x * 100)}%`;
/** "Weekly" → "weekly"; "Weekly Opus" → "weekly Opus" (v1 `lower`). */
const lower = (label: string): string => (label || t('budget.hold.usage')).replace(/^\w/, (c) => c.toLowerCase());

/** Where this send would likely take the window (v1 limit-hold.js `holdTo`): "96–99%", "about 95%", past the limit. */
export function holdTo(to: readonly [number, number]): string {
  const [lo, hi] = to.map((x) => Math.floor(x * 100)) as [number, number];
  if (lo >= 100) return t('budget.hold.pastLimit');
  if (hi - lo > 2) return `${lo}–${Math.min(hi, 100)}%`;
  return t('budget.hold.about', { pct: `${Math.min(hi, 99)}%` });
}

/** The row's sentence (v1 hold.js `whose` + `landing`): "Claude Code’s weekly window is at 91%. This task may take it
 *  to 96%." · "Codex (Work)’s 5-hour window is at 92%, past your 90% line." */
export function holdSay(h: Pick<AskHold, 'agent' | 'label' | 'used' | 'line' | 'to'>, followUp: boolean, accountLabel?: string): string {
  const name = NAMES[h.agent] ?? h.agent;
  const who = accountLabel ? t('budget.hold.who', { name, account: accountLabel }) : name;
  const head = t('budget.hold.head', { who, window: lower(h.label), pct: pct(h.used) });
  if (h.to[1] - h.used < 0.005) return t('budget.hold.past', { head, line: pct(h.line) });
  const to = holdTo(h.to);
  const where = to === t('budget.hold.pastLimit') ? to : t('budget.hold.to', { to });
  return t(followUp ? 'budget.hold.follow' : 'budget.hold.task', { head, where });
}

/** The check the prompt boxes ask (`PluginUiDef.sendHolds`). */
export async function checkHold(client: PluginClient, a: SendHoldAsk): Promise<SendHold | undefined> {
  if (!a.agent || a.agent === 'auto') return undefined; // wisp picks the harness: the turn's own guard asks then
  const peer = a.peer as PeerId;
  const h = await client.run(ask, { peer, agent: a.agent, ...(a.account && { account: a.account }), ...(a.session && { session: a.session }) });
  if (!h) return undefined;
  // the account's name only when the harness has several (v1 `whose`)
  let label: string | undefined;
  if (h.account && h.account !== 'default') {
    const names = await client.run(accountNames, { peer, agent: h.agent }).catch(() => ({ accounts: [] }));
    label = names.accounts.find((x) => x.id === h.account)?.label ?? h.account;
  }
  return { reason: HOLD_REASON, say: holdSay(h, a.session !== undefined, label), ...(h.resetsAt !== undefined && { until: h.resetsAt }) };
}

