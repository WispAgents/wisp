// What a task's row says, decided from its summary alone (v1 feed.js renderHead, headPills, runLine,
// quickDone, quickLater; core/format.js statusPill). Pure and DOM-free: the row component only lays
// it out, so every rule here has a test. Text is an i18n key and its vars, never English.

import { modelWords } from '@wisp/shell/pure';
import type { IconName } from '@wisp/ui';
import type { Tone } from '@wisp/tokens';
import { isHeld } from '@wisp/shell/pure';
import type { Row, Agent, SessionRow } from '../types.ts';
import { resText } from './machine.ts';
import { isForLater, isScheduled, nextAt } from './sections.ts';
import '../i18n/lazy.ts';

export interface Txt {
  readonly key: string;
  readonly vars?: Readonly<Record<string, string | number>>;
}
const tx = (key: string, vars?: Readonly<Record<string, string | number>>): Txt => (vars ? { key, vars } : { key });
/** Text the user or the agent wrote: shown as it is. */
export const RAW = 'shell-list.raw';
const raw = (text: string): Txt => ({ key: RAW, vars: { text } });

export interface PillView {
  readonly id: string;
  readonly label: Txt;
  readonly tone: Tone;
  readonly icon?: IconName;
  readonly dot?: boolean;
  readonly live?: boolean;
  readonly title?: Txt;
  readonly variant?: 'soft' | 'outline';
}

export interface SubView {
  readonly text: Txt;
  readonly icon?: IconName;
  readonly title?: Txt;
  /** the live line of a busy task: running or asking */
  readonly live?: 'running' | 'waiting';
  readonly held?: boolean;
}

export interface RunView {
  readonly agent: string;
  readonly agentName: string;
  readonly model?: string;
  readonly where: string;
  readonly remote: boolean;
  /** it runs in a sandbox of its own (ADR-0047): the machine's slot says Sandbox, with the box */
  readonly sandbox?: boolean;
  readonly title: Txt;
  /** Try it two ways: the run's one change in words, after the model (v1 feed.js runVariant) */
  readonly variant?: readonly Txt[];
}

export type TimeView = { readonly kind: 'ago'; readonly at: number; readonly title?: Txt } | { readonly kind: 'ahead'; readonly at: number; readonly title?: Txt };

export type Mark = { readonly kind: 'agent'; readonly status?: Tone; readonly live: boolean } | { readonly kind: 'origin'; readonly icon: IconName; readonly title: Txt } | { readonly kind: 'bell' };

export interface RowView {
  readonly mark: Mark;
  readonly sub?: SubView;
  readonly run?: RunView;
  readonly pills: readonly PillView[];
  /** v1 goal.js goalPill(): its round and the last check's progress, after the state */
  readonly goal?: { readonly n: number; readonly progress: number; readonly paused: boolean; readonly title: Txt };
  /** v1 overlaps.js overlapPill(): how many other tasks touch the same files */
  readonly overlap?: { readonly n: number; readonly clash: boolean; readonly title: Txt };
  readonly ram: string;
  readonly time: TimeView;
  readonly unread: boolean;
  /** a finished task's title reads quieter (ink-2) */
  readonly quiet: boolean;
  readonly quickLater: 'save' | 'unsave' | 'gap';
  /** Mark done (review), Reopen (done; docs/ux/2026-11-18-desktop.md #2), its empty slot, or none (the phone's swipes) */
  readonly quickDone: 'show' | 'reopen' | 'gap' | 'none';
}

export interface RowContext {
  /** the list is beside the open task (split view) */
  readonly split: boolean;
  /** this row is the open task */
  readonly open: boolean;
  /** swipes mark done on this device (the phone app): rows keep neither the ✓ nor its space */
  readonly swipes: boolean;
  /** what this computer is called (This Mac, Studio) */
  readonly here: string;
  /** what a sandbox is called in the machine's slot ("Sandbox") */
  readonly sandbox?: string;
  readonly agents: ReadonlyMap<string, Row<Agent>>;
  /** a harness's name as its plugin registered it (`shell.harnesses()`: "Codex"), when it has one: every line names it
   *  so, never by its raw id (audit D #2). Additive (2026-11-19). */
  readonly named?: (id: string) => string | undefined;
  /** a command for this task is on screen but its computer is away (ADR-0065 §4): the status column says
   *  *Waiting to send* until it goes. @wisp/client's waitingToSend. Additive. */
  readonly waiting?: boolean;
  /** what a task on its harness's Default runs, by name (ShellContext.defaultModel): the tip names it as the chip does.
   *  Additive (2026-11-18). */
  readonly defaultModel?: (s: SessionRow) => string | undefined;
}

export const busy = (s: SessionRow): boolean => s.state === 'running' || s.state === 'waiting';

/** The agent's verdict word (core/format.js statusKey): Blocked is split into wisp's own halts. */
export function statusKey(s: SessionRow): string {
  if (s.status === 'blocked' && s.halt) return s.halt;
  return s.status && s.status !== 'running' ? s.status : 'done';
}
const STATUS_TONE: Readonly<Record<string, Tone>> = { done: 'good', partial: 'warn', blocked: 'warn', failed: 'bad', limit: 'warn', refused: 'warn', signedOut: 'warn', info: 'info', held: 'warn', stopped: 'neutral' };

export const agentName = (id: string, agents: ReadonlyMap<string, Row<Agent>>, named?: (id: string) => string | undefined): string =>
  named?.(id) ?? agents.get(id)?.name ?? (id === 'auto' ? 'Auto' : id === 'wisp' ? 'wisp' : id);

function heldSub(s: SessionRow, c: RowContext): Txt {
  const h = s.held;
  if (!h) return raw('');
  if (h.project) return tx(h.project.setup ? 'shell-list.held.setup' : 'shell-list.held.project');
  if (h.harness) return tx('shell-list.held.harness', { agent: agentName(s.agent, c.agents, c.named) });
  if (h.waiting) return tx('shell-list.held.reset', { label: (h.label ?? '').toLowerCase() });
  if (h.label && h.used !== undefined) return tx('shell-list.held.window', { label: h.label, pct: Math.floor(h.used * 100) });
  return raw(h.why ?? h.label ?? '');
}

function laterLine(s: SessionRow): SubView | undefined {
  const l = s.later?.[0];
  if (!l) return undefined;
  if (l.held?.load) return { icon: 'pause', held: true, text: l.goal ? tx('shell-list.later.goal-computer', { n: l.goal }) : tx('shell-list.later.computer', { text: l.text }) };
  if (l.held) return { icon: 'pause', held: true, text: l.goal ? tx('shell-list.later.goal-plan', { n: l.goal }) : tx('shell-list.later.plan', { text: l.text }) };
  if (l.goal) return { icon: 'target', text: tx('shell-list.later.goal', { n: l.goal }) };
  return { icon: l.remind ? 'bell' : 'clock', text: raw(l.text) };
}

/** The line under the title: live text, held, scheduled, then the report's first line. */
export function subOf(s: SessionRow, c: RowContext): SubView | undefined {
  if (busy(s)) return s.live ? { text: raw(s.live), live: s.state === 'running' ? 'running' : 'waiting' } : undefined;
  if (c.open && !c.split) return undefined;
  // a done task never says it waits (QA r1 #10), but one waiting for its plan's reset does (Scheduled)
  if (s.held && (isHeld(s.state, s) || s.held.waiting)) return { icon: 'pause', held: true, text: heldSub(s, c) };
  if (isScheduled(s)) return laterLine(s);
  if (s.preview) return { text: raw(s.preview) };
  if (s.foreign) return { text: tx('shell-list.sub.brought-in', { tool: s.foreign.charAt(0).toUpperCase() + s.foreign.slice(1) }) };
  return undefined;
}

/** Harness, model and where it runs: the machine is always named, this computer too. */
export function runOf(s: SessionRow, c: RowContext): RunView | undefined {
  // a factory run runs no agent of its own: its line is the factory, by name (ADR-0114)
  if (s.factory) {
    const name = s.factory.name || 'default';
    return { agent: 'factory', agentName: name, where: c.here, remote: false, title: tx('shell-list.run.factory', { name }) };
  }
  if (!s.agent || s.agent === 'wisp' || s.kind === 'reminder') return undefined;
  const a = c.agents.get(s.agent);
  // the offer's label, else a Claude id in words (QA r15 #3: never the raw id)
  const model = (s.model && a?.models?.find((m) => m.id === s.model)?.label) || (s.model ? modelWords(s.model) : undefined);
  const sandbox = s.machine?.kind === 'sandbox';
  const where = sandbox ? (c.sandbox ?? 'Sandbox') : (s.machine?.name ?? c.here);
  const name = agentName(s.agent, c.agents, c.named);
  // on its Default: the tip names the model that runs, as the chip does (one name everywhere); the line stays v1's
  const runs = model ? model : c.defaultModel?.(s);
  return {
    agent: s.agent,
    agentName: name,
    ...(model ? { model } : {}),
    where,
    remote: !!s.machine,
    ...(sandbox && { sandbox: true }),
    // v1 runLine's tip: "Default model" when none was picked, "<effort> thinking", "on <place>"
    ...((v) => (v.length ? { variant: v } : {}))(variantOf(s)),
    title: tx(`shell-list.run.tip${runs ? '' : '-default'}${s.effort ? '-effort' : ''}`, { agent: name, model: runs ?? '', effort: s.effort === 'xhigh' ? 'X-High' : s.effort ? s.effort.charAt(0).toUpperCase() + s.effort.slice(1) : '', where }),
  };
}

const SUBAGENT_NAME: Readonly<Record<string, string>> = { haiku: 'Haiku', sonnet: 'Sonnet' };

/** Try it two ways (F-comp-31): what tells a trial's two rows on one harness apart. '' subagents is the task's model. */
export function variantOf(s: SessionRow): Txt[] {
  const v = s.variant;
  const out: Txt[] = [];
  if (v?.subagentModel) out.push(tx('shell-list.run.subagents', { name: SUBAGENT_NAME[v.subagentModel] ?? v.subagentModel }));
  else if (v?.subagentModel === '') out.push(tx('shell-list.run.subagents-task'));
  if (v?.skills === 'off') out.push(tx('shell-list.run.skills-off'));
  return out;
}

function statusPill(s: SessionRow): PillView | undefined {
  if (s.state === 'waiting') return { id: 'status', label: tx(s.approval ? 'shell-list.pill.approval' : 'shell-list.pill.asking'), tone: 'warn', dot: true, live: true };
  if (s.state !== 'review') return undefined;
  if (isHeld(s.state, s)) return { id: 'status', label: tx('shell-list.pill.held'), tone: 'warn', dot: true };
  if (s.reminder) return { id: 'status', label: tx('shell-list.pill.reminder'), tone: 'info', icon: 'bell', title: raw(s.reminder.text) };
  const k = statusKey(s);
  return { id: 'status', label: tx(`shell-list.status.${k}`), tone: STATUS_TONE[k] ?? 'neutral', dot: true };
}

/** The goal's ring and round, and the overlap mark (v1 feed.js: after the state pill). */
function goalAndOverlap(s: SessionRow): Pick<RowView, 'goal' | 'overlap'> {
  const out: { goal?: RowView['goal']; overlap?: RowView['overlap'] } = {};
  const g = s.goal;
  if (g?.text) out.goal = { n: g.round ?? 0, progress: g.checks?.[g.checks.length - 1]?.progress ?? 0, paused: g.status === 'paused', title: tx('shell-list.goal.tip', { text: g.text, why: g.last?.why ?? '' }) };
  const o = s.overlaps ?? [];
  if (o.length > 0) out.overlap = { n: o.length, clash: o.some((x) => (x.conflicts?.length ?? 0) > 0), title: tx('shell-list.overlap.tip', { n: o.length, titles: o.map((x) => x.title ?? x.id).join(', ') }) };
  return out as Pick<RowView, 'goal' | 'overlap'>;
}

/** In the status pill's place while a command waits for its computer: the done the row already shows isn't sent. */
const WAITING: PillView = { id: 'status', label: tx('shell-list.pill.waiting'), tone: 'neutral', icon: 'send', variant: 'outline', title: tx('shell-list.pill.waiting-tip') };

/** What a row says about its task between the title and the time, in v1's order. `waiting`: RowContext.waiting. */
export function pillsOf(s: SessionRow, waiting = false): PillView[] {
  const out: PillView[] = [];
  const status = waiting ? WAITING : statusPill(s);
  if (status) out.push(status);
  const l = s.later?.[0];
  if (l && !isScheduled(s)) out.push({ id: 'later', label: s.later && s.later.length > 1 ? tx('shell-list.pill.later-more', { n: s.later.length - 1, at: l.at }) : tx('shell-list.pill.later', { at: l.at }), tone: 'neutral', icon: l.remind ? 'bell' : 'clock', variant: 'outline' });
  if (s.queued) out.push({ id: 'queued', label: tx('shell-list.pill.queued', { n: s.queued }), tone: s.queueHeld ? 'warn' : 'neutral', title: tx(s.queueHeld ? 'shell-list.pill.queued-held-tip' : 'shell-list.pill.queued-tip', { n: s.queued }) });
  if (s.sideOf) out.push({ id: 'side', label: tx('shell-list.pill.side'), tone: 'neutral', icon: 'discuss', variant: 'outline', title: tx('shell-list.pill.side-tip', { title: s.sideOf.title }) });
  // v1 feed.js autoPill: the run Auto's best of two picked
  if (s.autoPick && !s.pending) out.push({ id: 'autoPick', label: tx('shell-list.pill.pick'), tone: 'accent', icon: 'spark', title: tx('shell-list.pill.pick-tip', { why: s.autoPick.why }) });
  // the PR's pill (and Shipping) is wisp.ship's own, on `shell.row:head` (v1 ship.js prPill)
  // a channel's cap (F-chan-10): the task page says it with Lift; the row names it
  if (s.capped === 'plan' || s.capped === 'edits') out.push({ id: 'capped', label: tx(`shell-list.pill.capped-${s.capped}`), tone: 'warn', icon: 'lock', variant: 'outline', title: tx(`shell-list.pill.capped-${s.capped}-tip`) });
  if (s.pinned) out.push({ id: 'pinned', label: tx('shell-list.pill.pinned'), tone: 'neutral', icon: 'pin', variant: 'outline' });
  return out;
}

const ORIGIN_ICON: Readonly<Record<string, IconName>> = { recipe: 'bolt', trigger: 'replay' };

function markOf(s: SessionRow): Mark {
  if (s.kind === 'reminder') return { kind: 'bell' };
  const ic = s.origin ? ORIGIN_ICON[s.origin.kind] : undefined;
  if (ic && s.origin) return { kind: 'origin', icon: ic, title: tx('shell-list.origin', { kind: s.origin.kind, name: s.origin.name ?? '' }) };
  const status: Tone | undefined = s.state === 'running' ? 'accent' : s.state === 'waiting' ? 'warn' : undefined;
  return { kind: 'agent', live: s.state === 'running', ...(status ? { status } : {}) };
}

function timeOf(s: SessionRow): TimeView {
  if (isScheduled(s)) return { kind: 'ahead', at: nextAt(s) };
  if (isForLater(s) && s.forLater) return { kind: 'ago', at: s.forLater.at, title: tx(s.forLater.by === 'crew' ? 'shell-list.time.filed' : 'shell-list.time.saved', { at: s.forLater.at, updated: s.updatedAt }) };
  return { kind: 'ago', at: s.updatedAt };
}

/** One-click Mark done on review rows; rows without one keep its space, so columns line up. */
function quickDone(s: SessionRow, c: RowContext): RowView['quickDone'] {
  if (c.swipes) return 'none';
  if (s.parentId || s.archived || (c.open && !c.split)) return 'gap';
  return s.state === 'review' ? 'show' : s.state === 'done' ? 'reopen' : 'gap';
}

/** The row's second line on a desktop (docs/ux/2026-11-18-desktop.md #2): the answer's last line (live words, the
 *  question, the report) with the harness's logo; with none, harness · model. The machine only when it isn't this
 *  computer (a sandbox says so too): "This computer" on every row said nothing. */
/** What a row's second line holds, and whether its run line comes third: a two-line row (every list but a run's own
 * nested row) puts the answer there (with the harness's mark), else the run itself, so a row with no answer is two lines
 * like the rest, its status beside the harness (audit G tasks #10: a desktop row put an empty line between them); a
 * nested row keeps its words, then the run line under them, except on a phone, where a run's line is its second. */
export function rowLines(two: boolean, v: Pick<RowView, 'sub' | 'run'>, phone = false): { readonly sub: 'answer' | 'run' | 'words'; readonly runFoot: boolean } {
  const line = secondLine(v).line;
  if (two && line === 'sub') return { sub: 'answer', runFoot: false };
  // a phone's nested run row too: nothing said yet, harness · model is its second line (as it was)
  if ((two || phone) && line === 'run') return { sub: 'run', runFoot: false };
  return { sub: 'words', runFoot: !!v.run };
}

/** The mark before a row's answer: a factory run's own (it runs no agent: ADR-0114 rule 14, never a harness's, audit G
 * tasks #8), else its harness's logo when the harness has one. */
export function rowMark(s: Pick<SessionRow, 'agent' | 'factory'>, logoOf: (agent: string) => string | undefined): { readonly factory: true } | { readonly logo: string } | undefined {
  if (s.factory) return { factory: true };
  const logo = logoOf(s.agent);
  return logo ? { logo } : undefined;
}

export function secondLine(v: Pick<RowView, 'sub' | 'run'>): { readonly line: 'sub' | 'run' | 'none'; readonly where: boolean } {
  const where = !!v.run && (v.run.remote || !!v.run.sandbox);
  return { line: v.sub ? 'sub' : v.run ? 'run' : 'none', where };
}

/** Some row the list shows has Mark done: on touch the list keeps its 36px column (v1 feed.js `has-quick`). */
export function anyShowsDone(rows: Iterable<{ readonly key: string; readonly s: SessionRow }>, ctx: (key: string) => RowContext): boolean {
  for (const r of rows) if (quickDone(r.s, ctx(r.key)) === 'show') return true;
  return false;
}

/** Save for later beside it: triage is one click either way. */
function quickLater(s: SessionRow, c: RowContext): RowView['quickLater'] {
  if (s.pending || s.archived || s.parentId || (c.open && !c.split)) return 'gap';
  if (isForLater(s)) return 'unsave';
  return s.state === 'review' ? 'save' : 'gap';
}

export function rowView(s: SessionRow, c: RowContext): RowView {
  const sub = subOf(s, c);
  const run = runOf(s, c);
  return {
    mark: markOf(s),
    ...(sub ? { sub } : {}),
    ...(run ? { run } : {}),
    pills: pillsOf(s, c.waiting === true),
    ...goalAndOverlap(s),
    ram: resText(s),
    time: timeOf(s),
    unread: !!s.unread && !busy(s),
    quiet: s.state === 'done' || !!s.archived,
    quickLater: quickLater(s, c),
    quickDone: quickDone(s, c),
  };
}
