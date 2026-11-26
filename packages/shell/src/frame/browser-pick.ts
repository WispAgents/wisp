// The browser a harness's agent drives (ADR-0128): `runtime.browser` (Settings → Agents, every harness) and
// `runtime.browsers` (one harness's own: its detail in Settings → Agents, or the picker's Browser row). What a harness
// can take comes from wisp.runtime (`AgentStatus.browser`): per-run MCP servers (Cursor and Antigravity take none),
// its own Chrome (Claude in Chrome), and whether this wisp is a sandbox's (no Chrome of yours there, only its own
// browser). The words are the caller's (`t`); this decides which choice is offered, greyed out, or in effect.

export const BROWSER_SETTING = 'runtime.browser';
export const BROWSERS_SETTING = 'runtime.browsers';

export const BROWSER_VALUES = ['default', 'off', 'chrome', 'headless'] as const;
export type BrowserValue = (typeof BROWSER_VALUES)[number];

/** wisp.runtime's `BrowserSupport` (generated: plugins/runtime/api/bindings/runtime/BrowserSupport.ts), as the UI reads it. */
export interface BrowserCan {
  readonly mcp: boolean;
  readonly ownChrome: boolean;
  readonly sandbox: boolean;
}

/** A wisp from before the setting says nothing: a harness is taken as one that takes MCP, outside a sandbox. */
export const BROWSER_CAN_UNKNOWN: BrowserCan = { mcp: true, ownChrome: false, sandbox: false };

const valueOf = (v: unknown): BrowserValue | undefined => (BROWSER_VALUES as readonly unknown[]).includes(v) ? (v as BrowserValue) : undefined;

/** The global setting's value (unset: default). */
export function browserGlobal(global: unknown): BrowserValue {
  return valueOf(global) ?? 'default';
}

/** `agent`'s own value, '' when it follows the global one. */
export function browserOwn(own: unknown, agent: string): BrowserValue | '' {
  const v = own && typeof own === 'object' ? valueOf((own as Record<string, unknown>)[agent]) : undefined;
  return v === undefined || v === 'default' ? '' : v;
}

/** What `agent` follows: its own value, else the global one. */
export function browserSetting(global: unknown, own: unknown, agent: string): BrowserValue {
  return browserOwn(own, agent) || browserGlobal(global);
}

/** `own` with `agent`'s value set ('' or 'default': it follows the global one again). */
export function withBrowser(own: unknown, agent: string, v: BrowserValue | ''): Record<string, string> {
  const next: Record<string, string> = {};
  if (own && typeof own === 'object') for (const [k, x] of Object.entries(own as Record<string, unknown>)) if (typeof x === 'string') next[k] = x;
  if (v === '' || v === 'default') delete next[agent];
  else next[agent] = v;
  return next;
}

/** Why a choice can't be had on a harness: no per-run MCP (Cursor, Antigravity); Claude in Chrome in a sandbox; or any
 *  Chrome in a sandbox. The caller's i18n key is `shell.browser.why.<reason>`. */
export type BrowserWhy = 'noMcp' | 'sandboxClaude' | 'sandbox';

export interface BrowserChoice {
  readonly id: BrowserValue;
  /** greyed out, with why */
  readonly why?: BrowserWhy;
}

export interface BrowserRow {
  readonly choices: readonly BrowserChoice[];
  /** the choice in effect: never one greyed out (a sandbox's Chrome runs on its own browser: headless here) */
  readonly value: BrowserValue;
  /** a sandbox runs every choice but off on its own browser; a harness without MCP has none */
  readonly note?: BrowserWhy | 'sandboxBrowser';
}

/** The picker's Browser row for a harness that can do `can`, set to `setting`. Mirrors `browser::resolve` in
 *  wisp-runtime-api: what's greyed out here is what a run would never get. */
export function browserRow(can: BrowserCan, setting: BrowserValue): BrowserRow {
  if (!can.mcp && !can.ownChrome) {
    return { choices: BROWSER_VALUES.map((id) => (id === 'off' ? { id } : { id, why: 'noMcp' as const })), value: 'off', note: 'noMcp' };
  }
  if (can.sandbox) {
    const chrome: BrowserWhy = can.ownChrome ? 'sandboxClaude' : 'sandbox';
    const choices = BROWSER_VALUES.map((id): BrowserChoice => (id === 'chrome' ? { id, why: chrome } : { id }));
    // anything but off is the sandbox's browser: Chrome falls back to it, and shows as the browser it runs on
    const value: BrowserValue = setting === 'chrome' ? 'headless' : setting;
    return { choices, value, ...(value !== 'off' && { note: 'sandboxBrowser' as const }) };
  }
  return { choices: BROWSER_VALUES.map((id) => ({ id })), value: setting };
}
