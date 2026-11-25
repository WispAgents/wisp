// Settings → Usage's reads and writes: the 7-day line (runtime.usage), each plan's windows (budget.view), and
// Automations stop at (budget.setStop, shown at once on the settings row).
import { defineCommand, entityKey, type PeerId } from '@wisp/client';

export const TOPIC = 'plugin:wisp.budget:plan';
export const META = 'budget_meta';

export interface UsageDay {
  readonly date: string;
  readonly cost: number;
  readonly turns: number;
  readonly tokens?: number;
  /** its cost holds a list-price estimate (ADR-0119, additive 2026-11-25) */
  readonly cost_estimated?: boolean;
}

/** One harness's share of a day (runtime-api UsageHarnessDay, additive 2026-11-25): the stacked chart's parts. */
export interface UsageHarnessDay {
  readonly date: string;
  readonly agent: string;
  readonly cost: number;
  readonly tokens: number;
  readonly cache_read: number;
  readonly turns: number;
  readonly cost_estimated?: boolean;
}

/** An ISO week of the window (runtime-api UsageWeek, additive 2026-11-25). */
export interface UsageWeek {
  readonly week: string;
  readonly start: string;
  readonly cost: number;
  readonly turns: number;
  readonly tokens: number;
  /** its cost holds a list-price estimate (ADR-0119, additive 2026-11-25) */
  readonly cost_estimated?: boolean;
}

export interface Usage {
  readonly from: number;
  readonly spend: number;
  readonly turns: number;
  readonly worked: number | null;
  readonly questions: number;
  /** v1's Charts and breakdowns (runtime-api UsageSummary, additive 2026-11-12) */
  readonly priced?: number;
  readonly sessions?: number;
  /** of `priced`, the turns estimated at list price, and whether `spend` holds any (ADR-0119, additive 2026-11-25) */
  readonly estimated?: number;
  readonly costEstimated?: boolean;
  /** TRY-32: token totals and per-harness rollup across all harnesses */
  readonly tokens?: number;
  readonly inputTokens?: number;
  readonly outputTokens?: number;
  readonly cacheTokens?: number;
  readonly daily?: readonly UsageDay[];
  /** each local day split by harness, and the ISO weeks (additive 2026-11-25) */
  readonly dailyByHarness?: readonly UsageHarnessDay[];
  readonly weekly?: readonly UsageWeek[];
  readonly harnesses?: readonly UsageRow[];
  readonly projects?: readonly UsageRow[];
  readonly runners?: readonly UsageRow[];
}

export interface UsageRow {
  readonly key: string;
  readonly name: string;
  readonly agent?: string;
  readonly cost: number;
  readonly priced: number;
  readonly turns: number;
  readonly sessions?: number;
  readonly estimated?: number;
  readonly costEstimated?: boolean;
  /** Copilot's premium requests (gated until a real capture, ADR-0119) */
  readonly premiumRequests?: number;
  readonly tokens?: number;
  readonly inputTokens?: number;
  readonly outputTokens?: number;
  readonly cacheTokens?: number;
  readonly worked: number | null;
  readonly questions: number;
  readonly avgMs: number | null;
}

export interface Window {
  readonly id: string;
  readonly label: string;
  /** 0–1 (budget-api WindowView.used) */
  readonly used: number;
  readonly resetsAt?: number | null;
  /** when it runs out at this pace, if before the reset */
  readonly hitAt?: number | null;
  /** the window's length */
  readonly ms?: number | null;
  /** it reset before the time it last gave (a banked reset spent) */
  readonly earlyAt?: number | null;
}

import type { Drain, Notice } from './drain.ts';
import type { Banked, Compare, Overage, Where } from './plan.ts';

export interface AccountPlan {
  readonly account: string;
  readonly at?: number | null;
  readonly windows: readonly Window[];
  /** Drain watch: this account's plan moving faster than usual (budget's Drain) */
  readonly drain?: Drain | null;
  /** window changes not hidden yet */
  readonly notices?: readonly Notice[];
  /** Codex's banked resets, as its last read said */
  readonly banked?: Banked | null;
  /** runs billed past the plan this week */
  readonly overage?: Overage | null;
  /** where its longest window went this period */
  readonly where?: Where | null;
  /** the plan or tier its CLI names (Cursor's `about`; budget-api AccountView, additive 2026-11-25) */
  readonly plan?: string;
  /** what its CLI totals as spent lately (opencode `stats`, `grok usage`; additive 2026-11-25) */
  readonly spent?: CliSpend;
}

/** What a CLI itself reports of recent use (budget-api CliSpend): its own sum, not wisp's turns. */
export interface CliSpend {
  readonly cost?: number;
  readonly days?: number;
  readonly tokens?: number;
  readonly at?: number;
}

/** `tzOffsetMin`: minutes east of UTC (`-getTimezoneOffset()`), so the days are the reader's local days. */
export const usage = defineCommand<{ peer: PeerId; days?: number; tzOffsetMin?: number }, Usage>({
  id: 'runtime.usage',
  owner: (a) => a.peer,
  wire: (a) => ({ ...(a.days && { days: a.days }), ...(a.tzOffsetMin !== undefined && { tz_offset_min: a.tzOffsetMin }) }),
  optimistic: 'none',
  pending: 'the usage line',
});

export const plans = defineCommand<{ peer: PeerId; agent: string; named?: boolean }, AccountPlan[]>({
  id: 'budget.view',
  owner: (a) => a.peer,
  // `named`: Where it went, with the tasks' titles (Settings → Usage only: the picker asks for every harness)
  wire: (a) => ({ agent: a.agent, ...(a.named && { named: true }) }),
  optimistic: 'none',
  pending: 'the plan windows',
});

/** Tokens per turn by account (budget.compare): models two or more accounts ran 5 turns on, this week. */
export const compare = defineCommand<{ peer: PeerId; agent: string }, Compare[]>({
  id: 'budget.compare',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'tokens per turn by account',
});

/** A harness's accounts and their names (wisp.accounts' `accounts.list`); refused or absent: none. */
export const accountNames = defineCommand<{ peer: PeerId; agent: string }, { accounts: { id: string; label: string; default?: boolean }[] }>({
  id: 'accounts.list',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the account names',
});

/** `autoStop`, `autoStopClaude`, `autoStopCodex`, `autoStopAntigravity`: 'off' | '50' | '75' | '90', 'same' for a harness's own. */
export const setStop = defineCommand<{ peer: PeerId; key: string; value: string }>({
  id: 'budget.setStop',
  owner: (a) => a.peer,
  wire: (a) => ({ key: a.key, value: a.value }),
  optimistic: (base, a) => {
    const id = entityKey(a.peer, 'settings');
    const row = base.peek<{ id: string; body: Record<string, string> }>(META, id);
    return [{ e: META, id, op: 'upsert', v: { id: 'settings', body: { ...(row?.body ?? {}), [a.key]: a.value } } }];
  },
});

/** The drain card's Dismiss and a window change's ×: hidden until it happens again. */
export const dismissDrain = defineCommand<{ peer: PeerId; agent: string; account: string; window: string }>({
  id: 'budget.dismissDrain',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent, account: a.account, window: a.window }),
  optimistic: 'none',
  pending: 'Dismiss',
});
export const dismissNotice = defineCommand<{ peer: PeerId; agent: string; account: string; window: string }>({
  id: 'budget.dismissNotice',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent, account: a.account, window: a.window }),
  optimistic: 'none',
  pending: 'the ×',
});

/** The harnesses set up here (wisp.runtime's `runtime.agents`): Home's usage row skips one that isn't. */
export const agents = defineCommand<{ peer: PeerId }, { id: string; available: boolean }[]>({
  id: 'runtime.agents',
  owner: (a) => a.peer,
  wire: () => ({}),
  optimistic: 'none',
  pending: 'the harness list',
});
