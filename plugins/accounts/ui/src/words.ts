// The Agents pane's rules, DOM-free so tests read them: what a harness row says, what Re-check turned
// up, the changed-harness line, and which step of a sign-in shows (v1 settings.js paintAgents,
// recheckSummary; agent-login.js control/loginPanel; harness-watch.js changeText).

import { t } from './i18n.ts';
import type { MenuEntry } from '@wisp/ui/pure';
import type { AccountRow, DetectRow, HarnessChange, HarnessRow, KeyRow, LoginRow, Pin, ToolRow } from './types.ts';
import './i18n/lazy.ts';

type Tone = 'neutral' | 'good' | 'warn' | 'bad';

const modelCount = (h: Pick<HarnessRow, 'models'> | undefined): number => (h?.models ?? []).filter((m) => m.id).length;

/** Installed but signed out: hidden from the picker, and its row offers Sign in. */
export const signedOut = (h: Pick<HarnessRow, 'available' | 'signedIn'>): boolean => !h.available && h.signedIn === false;
/** Found, or found and signed out: its row has a version and a sign-in. */
export const present = (h: Pick<HarnessRow, 'available' | 'signedIn'>): boolean => h.available || signedOut(h);

/** "Checking…", "Ready", "Signed out", "Not found" and its tone (v1 sx-state). */
/** `cli`: its CLI's own login as last asked, where the row's Sign in follows it: a found harness its CLI says is signed
 *  out reads Signed out, as its button says (QA r1 #23: "Sign in" beside "● Ready"). */
export function harnessState(h: Pick<HarnessRow, 'available' | 'signedIn' | 'checking'> & { readonly disabled?: boolean | undefined }, cli?: boolean): { tone: Tone; label: string } {
  // its plugin is off (Settings → Plugins): nothing else about it is live
  if (h.disabled) return { tone: 'neutral', label: t('accounts.state.disabled') };
  if (h.checking) return { tone: 'neutral', label: t('accounts.state.checking') };
  if (h.available && cli === false) return { tone: 'warn', label: t('accounts.state.signedOut') };
  if (h.available) return { tone: 'good', label: t('accounts.state.ready') };
  if (signedOut(h)) return { tone: 'warn', label: t('accounts.state.signedOut') };
  return { tone: 'bad', label: t('accounts.state.notFound') };
}

/** The GitHub row's state (git's only while missing): Not found, gh signed in or out, or nothing when gh didn't say. */
export function toolState(x: Pick<ToolRow, 'id' | 'name' | 'installed' | 'signedIn'>): { tone: Tone; label: string } | undefined {
  if (!x.installed) return { tone: 'bad', label: t('accounts.state.notFound') };
  if (x.signedIn === true) return { tone: 'good', label: t('accounts.login.signedIn') };
  if (x.signedIn === false) return { tone: 'warn', label: t('accounts.state.signedOut') };
  return undefined;
}

/** The harness's small line: "2.1.30 · 3 models · 2 accounts", or how to get it while it isn't found. */
export function harnessLine(h: HarnessRow, accounts: number): string {
  // never seen yet: no install tip before its detect says it isn't there
  if (h.checking && !present(h)) return t('accounts.line.checking');
  if (!present(h)) return h.installing ? t('accounts.installing') : (h.setup?.tip ?? t('accounts.setup.tip', { name: h.name }));
  const n = modelCount(h);
  const parts = [
    h.version ?? '',
    signedOut(h) ? t('accounts.line.signIn') : n ? t('accounts.line.models', { count: n }) : '',
    h.hasKey === false ? t('accounts.line.freeOnly') : '',
    accounts > 1 ? t('accounts.line.accounts', { count: accounts }) : '',
  ];
  return parts.filter(Boolean).join(' · ');
}

/** Not found here (v1 settings.js paintAgents): this OS's tip and the one-line command to copy, unless wisp installs it
 * itself (Install) or it's installing; undefined while it's found. */
export function howToGet(h: HarnessRow): { tip: string; command?: string } | undefined {
  if (present(h) || h.installing || h.checking) return undefined;
  const tip = h.setup?.tip ?? t('accounts.setup.tip', { name: h.name });
  const command = h.installable ? undefined : h.setup?.command;
  return command ? { tip, command } : { tip };
}

/** "a@corp.com · Max · Work" (v1 accountsHtml). */
export const accountLine = (a: Pick<AccountRow, 'email' | 'plan' | 'kind'>): string => [a.email, a.plan, t(a.kind === 'work' ? 'accounts.kind.work' : 'accounts.kind.personal')].filter(Boolean).join(' · ');

/** What a Re-check turned up: harnesses found or gone, model lists that grew or shrank, new versions. */
export function recheckSummary(before: ReadonlyMap<string, Pick<HarnessRow, 'available' | 'models' | 'version'>>, after: readonly HarnessRow[]): string {
  const notes: string[] = [];
  for (const a of after) {
    const was = before.get(a.id);
    if (a.available && !was?.available) notes.push(t('accounts.recheck.found', { name: a.name }));
    else if (!a.available && was?.available) notes.push(t('accounts.recheck.gone', { name: a.name }));
    else if (a.available && modelCount(a) !== modelCount(was)) {
      const d = modelCount(a) - modelCount(was);
      notes.push(t(d > 0 ? 'accounts.recheck.more' : 'accounts.recheck.fewer', { name: a.name, count: Math.abs(d) }));
    } else if (a.available && a.version !== was?.version) notes.push(t('accounts.recheck.updated', { name: a.name }));
  }
  return notes.length ? notes.join(' · ') : t('accounts.recheck.none');
}

/** One harness's Update in an Update all: the version it went to, or why it didn't (the server's words, which name the
 *  command to run by hand). */
export interface UpdateOutcome {
  readonly name: string;
  readonly version?: string;
  readonly error?: string;
}

/** What Update all says once every update settled (audit I IS11: it said nothing when they didn't take): how many took,
 *  then each harness, what it went to or why not, in the order pressed. */
export function updateAllSummary(results: readonly UpdateOutcome[]): { readonly message: string; readonly description: string; readonly failed: boolean } {
  const ok = results.filter((r) => r.error === undefined).length;
  const lines = results.map((r) => (r.error === undefined ? t('accounts.updateAll.took', { name: r.name, version: r.version ?? '' }).trim() : t('accounts.updateAll.didnt', { why: r.error })));
  return { message: t('accounts.updateAll.summary', { ok, count: results.length }), description: lines.join('\n'), failed: ok < results.length };
}

/** The pane's harness rows (ADR-0066): wisp.runtime's detection rows in their order, each with what this plugin's
 * own row adds (sign-in, keys, accounts, Install); a detection row is the fresher of the two. With no detection rows
 * (a wisp before the topic), this plugin's rows as they are. */
export function mergeHarnesses(mine: readonly HarnessRow[], detected: readonly DetectRow[]): HarnessRow[] {
  if (detected.length === 0) return foundFirst([...mine].sort((a, b) => a.ord - b.ord));
  const byId = new Map(mine.map((h) => [h.id, h]));
  const out = [...detected]
    .sort((a, b) => a.ord - b.ord)
    .map((d): HarnessRow => {
      const base: HarnessRow = byId.get(d.id) ?? { id: d.id, name: d.agent.name, ord: d.ord, available: false, models: [], signIn: 'none' };
      const { version: _v, error: _e, setup: _s, signedIn: _o, checking: _c, efforts: _f, ...rest } = base;
      return {
        ...rest,
        name: d.agent.name,
        ord: d.ord,
        available: d.agent.available,
        models: d.agent.models.map((m) => ({ id: m.id, label: m.label, ...(m.efforts && { efforts: [...m.efforts] }) })),
        ...(d.agent.efforts && { efforts: [...d.agent.efforts] }),
        ...(d.agent.version !== undefined && { version: d.agent.version }),
        ...(d.agent.error !== undefined && { error: d.agent.error }),
        ...(d.agent.setup !== undefined && { setup: d.agent.setup }),
        ...(d.agent.signedIn === false && { signedIn: false as const }),
        ...(d.state === 'checking' && { checking: true }),
      };
    });
  const seen = new Set(out.map((h) => h.id));
  return foundFirst([...out, ...mine.filter((h) => !seen.has(h.id)).sort((a, b) => a.ord - b.ord)]);
}

/** The harnesses found first, then the missing ones (v1: Copilot, not found, came last), each in wisp's order. */
// one still being checked keeps its place (nothing jumps as they settle); only one known missing goes last
const missingNow = (h: HarnessRow): boolean => !h.available && h.signedIn !== false && h.checking !== true;
const foundFirst = (list: readonly HarnessRow[]): HarnessRow[] => [...list.filter((h) => !missingNow(h)), ...list.filter(missingNow)];

/** Before the server's first answer: a row per harness the plugins registered, each Checking… (a restart or a first
 *  start showed the lede alone until every harness was detected). */
export const pendingHarnesses = (known: readonly { readonly id: string; readonly name: string; readonly order?: number | undefined }[]): HarnessRow[] =>
  [...known]
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map((k, i) => ({ id: k.id, name: k.name, ord: k.order ?? i, available: false, models: [], signIn: 'none', checking: true }));

/** The rows with the known harnesses that haven't answered yet added, Checking…, while the pane's own rows aren't in
 *  (`waiting`): the answers land one by one, and a row never vanishes to come back. */
export function withPending(rows: readonly HarnessRow[], known: Parameters<typeof pendingHarnesses>[0], waiting: boolean): HarnessRow[] {
  if (!waiting) return [...rows];
  const have = new Set(rows.map((r) => r.id));
  return [...rows, ...pendingHarnesses(known).filter((p) => !have.has(p.id))].sort((a, b) => a.ord - b.ord);
}

/** An account row's account (its id is its entity's, `claude/w1`; an optimistic one may be bare). */
export const accOf = (a: Pick<AccountRow, 'agent' | 'id'>): string => (a.id.startsWith(`${a.agent}/`) ? a.id.slice(a.agent.length + 1) : a.id);

/** v1 accounts.js moreMenu: Rename, Make it a work / personal account, Move up, Move down, Sign out, Remove. What can't
 *  apply stays, disabled, so every row's menu has the same shape (the CLI's own login can be signed out, not removed). */
type Tr = (k: string, v?: Readonly<Record<string, string>>) => string;
const tt: Tr = (k, v) => t(k as Parameters<typeof t>[0], v);

export function accountMenu(a: AccountRow, i: number, n: number, tr: Tr = tt): MenuEntry[] {
  return [
    { value: 'rename', label: tr('accounts.account.rename'), icon: 'pen' },
    { value: 'kind', label: tr(a.kind === 'work' ? 'accounts.account.personal' : 'accounts.account.work'), icon: 'user' },
    { value: 'up', label: tr('accounts.account.up'), disabled: i === 0 },
    { value: 'down', label: tr('accounts.account.down'), disabled: i === n - 1 },
    { value: 'out', label: tr('accounts.login.signOut'), icon: 'lock', disabled: a.state === 'signedOut' },
    { kind: 'separator' },
    { value: 'remove', label: tr('accounts.account.remove'), icon: 'trash', danger: true, disabled: a.default, ...(a.default && { hint: tr('accounts.account.keepsOwn') }) },
  ];
}

/** An account's state when it isn't ready (v1: "At its limit until 9 pm", "Signed out"); ready says nothing. */
export function accountState(a: AccountRow, tr: Tr = tt): string | undefined {
  if (a.state === 'limit') return a.until ? tr('accounts.state.limitUntil', { until: new Date(a.until).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }) }) : a.why || tr('accounts.state.limit');
  if (a.state === 'signedOut') return tr('accounts.state.signedOut');
  return undefined;
}

/** A program by its version, else its path (the server sends `version: ""` when the harness didn't say one). */
const label = (p: Pin): string => p.version || p.path;

const dayOf = (c: HarnessChange, locale?: string): string => new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(c.from.at);

/** What changed: "2.0.1 → 2.0.2", "2.0.1, but its program’s bytes differ", or with no version "its program’s bytes differ"
 *  or from which path to which (never "…: , but…", TRY-17). */
export function changeWhat(c: HarnessChange): string {
  const from = label(c.from);
  const moved = c.kind !== 'update' && c.from.real !== c.to.real && c.from.path !== c.to.path;
  // a version is said with where it is now; a path already says where
  const to = moved && c.to.version ? t('accounts.change.at', { what: label(c.to), path: c.to.path }) : label(c.to);
  return from !== to ? `${from} → ${to}` : c.from.version ? t('accounts.change.bytes', { what: from }) : t('accounts.change.bytesOnly');
}

/** "Claude Code changed since 2 Nov: 2.0.1 → 2.0.2" (v1 harness-watch changeText): the whole sentence. */
export function changeText(name: string, c: HarnessChange, locale?: string): string {
  return t('accounts.change', { name, day: dayOf(c, locale), what: changeWhat(c) });
}

/** The notice's title: who and since when, which fits a row's one line; what changed opens its description (QA phone r4
 *  #5: the whole sentence, cut to fit, read "Codex changed since 16 Nov:…" on Android). */
export function changeTitle(name: string, c: HarnessChange, locale?: string): string {
  return t('accounts.change.since', { name, day: dayOf(c, locale) });
}

/** A phrase as a sentence's start: "its program’s bytes differ" → "Its program’s bytes differ". */
export const sentence = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);

/** The tooltip: both programs' paths and hashes. */
export function changeTip(c: HarnessChange): string {
  const one = (p: Pin): string => `${p.path}${p.real !== p.path ? ` (${p.real})` : ''}, sha256 ${p.sha256}`;
  return t('accounts.change.tip', { was: one(c.from), now: one(c.to) });
}

/** Still open: not hashing, and not trusted since this change was seen. */
export const stillChanged = (h: Pick<HarnessRow, 'changed' | 'pinnedAt'>): boolean => !!h.changed && !h.changed.checking && !((h.pinnedAt ?? 0) >= h.changed.at);

export const busy = (l: Pick<LoginRow, 'state'> | undefined): boolean => l?.state === 'running' || l?.state === 'waiting';

/** The control in a harness's row (one account) or an account's row (v1 agent-login control). */
export type LoginControl = 'cancel' | 'signIn' | 'signedIn' | 'none';
export function loginControl(l: LoginRow | undefined, opts: { account: boolean; out: boolean }): LoginControl {
  if (busy(l)) return 'cancel';
  // a sign-in that failed or was cancelled: Sign in again (a failed one's panel says why)
  if (l?.state === 'failed' || l?.state === 'cancelled') return 'signIn';
  // An account row says "Signed out" itself and keeps Sign out in its ⋯ menu
  if (opts.account) return l?.signedIn === false || (l?.signedIn !== true && opts.out) ? 'signIn' : 'none';
  if (l?.signedIn === true) return 'signedIn';
  if (l?.signedIn === false || opts.out) return 'signIn';
  return 'none';
}

/** What a sign-in's panel shows (v1 loginPanel): nothing, the failure, or the steps so far. */
export type LoginStep =
  | { kind: 'none' }
  | { kind: 'failed'; error: string }
  | { kind: 'steps'; url: string; code: string; paste: boolean; checking: boolean };
/** whose account a CLI's sign-in page asks for (v1 agent-login.js NAMES); none: step 1 names no one */
const SIGN_IN_WITH: Readonly<Record<string, string>> = { claude: 'Claude', codex: 'ChatGPT', grok: 'xAI', antigravity: 'Google', gh: 'GitHub' };
export const signInWith = (agent: string): string | undefined => SIGN_IN_WITH[agent];

export function loginStep(agent: string, l: LoginRow | undefined): LoginStep {
  if (!l || (!busy(l) && l.state !== 'failed')) return { kind: 'none' };
  if (l.state === 'failed') return { kind: 'failed', error: l.error || t('accounts.login.failed') };
  // Claude shows a code on its page to paste back here; Codex and GitHub show one to type there
  const paste = (agent === 'claude' && !l.checking) || l.needsInput === true;
  return { kind: 'steps', url: l.url ?? '', code: l.code ?? '', paste: paste && !l.code, checking: l.checking === true && l.needsInput !== true };
}

/** "…a3f9" for a saved key; the key itself never reaches the page. */
export const masked = (k: Pick<KeyRow, 'set' | 'hint'>): string => (k.set ? (k.hint ? `…${k.hint}` : '…') : '');

/** "opencode.ai/auth" from its URL, for the line saying where keys come from. */
export const site = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

/** Another computer's harnesses that sign in with their CLI (v1 cloud.js signable): found there, in wisp's order. */
export function signable(rows: readonly (HarnessRow | undefined)[]): HarnessRow[] {
  return rows.filter((h): h is HarnessRow => !!h && h.available && h.signIn === 'cli').sort((a, b) => a.ord - b.ord);
}
