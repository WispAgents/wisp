// A task's subagents beside its crew jobs, in its Team tab (TRY-24): wisp's own delegates that aren't crew jobs (a
// task whose parent is this one, on any harness) and the harness's own subagents (Claude Code's Agent/Task calls,
// from the task's steps: their current step is the newest call that names them as `parent`, their result is the
// call's output). One row each: name, harness and model, what it does now or came back with, its state and time.
// Pure: the rows from the task's rows and steps; the pane lays them out and opens them.
import { computed, type PluginClient, type Readable } from '@wisp/client';
import type { Say, TeamRow } from './team.ts';
import './i18n/lazy.ts';

/** A task row as the list sends it (runtime's `session` row). */
export interface TaskRowLike {
  readonly id: string;
  readonly parent_id?: string | null;
  readonly title?: string;
  readonly state?: string;
  readonly agent?: string;
  readonly created_at?: number;
  readonly summary?: Readonly<Record<string, unknown>>;
  readonly body?: Readonly<Record<string, unknown>>;
}

/** One step of the task (runtime's `event` row). */
export interface StepRowLike {
  readonly session_id: string;
  readonly seq: number;
  readonly t: number;
  readonly kind: string;
  readonly body: Readonly<Record<string, unknown>>;
}

/** A row of the Team tab that isn't a crew job. */
export interface SubRow extends TeamRow {
  readonly kind: 'delegate' | 'builtin';
  /** the harness it runs on, for its logo */
  readonly agent: string;
  /** a built-in subagent's call id (its own view lists its steps) */
  readonly call?: string;
}

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const num = (v: unknown): number | undefined => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : undefined);
const firstLine = (s: string): string => s.trim().split('\n')[0]?.trim() ?? '';
/** `text` with the task's own folder taken out of its paths (as its transcript shows them): `src/a.ts`, not the full path. */
export function relative(text: string, cwd: string): string {
  if (!cwd) return text;
  const dir = cwd.replace(/^\/private/, '').replace(/\/+$/, '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return text.replace(new RegExp(`(/private)?${dir}/?`, 'g'), '');
}
const isCrewJob = (r: TaskRowLike): boolean => !!(r.body?.['ext'] as Readonly<Record<string, unknown>> | undefined)?.['wisp.crew'];

/** A subagent runs where its lead does: no place chip (a crew job's row names its machine). */
const here = (_say: Say): TeamRow['place'] => ({ name: '', icon: 'laptop', away: false, tip: '' });

function stateOf(state: string, status: string, say: Say): Pick<TeamRow, 'status' | 'tone' | 'live' | 'running'> {
  if (state === 'running') return { status: say('wisp.crew.job.working'), tone: 'accent', live: true, running: true };
  if (state === 'waiting') return { status: say('wisp.crew.job.asking'), tone: 'warn', live: false, running: true };
  if (status === 'error' || status === 'failed' || state === 'error') return { status: say('wisp.crew.team.failed'), tone: 'bad', live: false, running: false };
  if (status === 'stopped') return { status: say('wisp.crew.team.stopped'), tone: 'neutral', live: false, running: false };
  return { status: say('wisp.crew.team.done'), tone: 'good', live: false, running: false };
}

/** wisp's own delegates of task `lead` that aren't crew jobs (those are the crew's rows), oldest first. */
export function delegateRows(tasks: readonly TaskRowLike[], lead: string, harnessName: (id: string) => string, say: Say): SubRow[] {
  const id = bare(lead);
  return tasks
    .filter((r) => !!r.parent_id && (r.parent_id === lead || bare(r.parent_id) === id) && !isCrewJob(r))
    .slice()
    .sort((a, b) => (a.created_at ?? 0) - (b.created_at ?? 0) || a.id.localeCompare(b.id))
    .map((r) => {
      const s = r.summary ?? {};
      const st = stateOf(r.state ?? '', str(s['status']), say);
      const model = str(s['model']);
      const harness = harnessName(r.agent ?? '');
      const startedAt = num(s['startedAt']);
      const endedAt = num(s['endedAt']);
      return {
        kind: 'delegate',
        id: r.id,
        agent: r.agent ?? '',
        member: '',
        name: harness,
        avatar: '',
        color: '',
        handle: r.title || harness,
        line: [model ? `${harness} · ${model}` : harness, (st.live ? str(s['live']) : str(s['preview'])) || ''].filter(Boolean).join(' — '),
        ...st,
        pending: false,
        place: here(say),
        ...(startedAt !== undefined && { startedAt }),
        ...(endedAt !== undefined && { endedAt }),
      };
    });
}

/** The harness's own subagents of task `lead` (Agent/Task calls in its steps), in the order they started. `running`:
 *  the lead still works (a call without an answer is then still working, else it was cut off). */
export function builtinRows(
  steps: readonly StepRowLike[],
  lead: string,
  agent: string,
  model: string,
  running: boolean,
  harnessName: (id: string) => string,
  say: Say,
  cwd = '',
): SubRow[] {
  const id = bare(lead);
  const mine = steps.filter((e) => e.session_id === lead || e.session_id === id).slice().sort((a, b) => a.seq - b.seq);
  const calls = mine.filter((e) => e.kind === 'tool' && e.body['tool'] === 'agent' && str(e.body['callId']));
  const harness = harnessName(agent);
  return calls.map((c) => {
    const call = str(c.body['callId']);
    const answer = mine.find((e) => e.kind === 'tool_output' && e.body['callId'] === call);
    const output = str(c.body['output']) || str(answer?.body['text']);
    const now = [...mine].reverse().find((e) => e.kind === 'tool' && e.body['parent'] === call);
    const back = !!answer || !!str(c.body['output']);
    // no answer: still working while the lead works, else it was cut off with it
    const st = back ? stateOf('done', '', say) : running ? stateOf('running', '', say) : stateOf('done', 'stopped', say);
    const label = str(c.body['label']).replace(/^Delegating:\s*/, '');
    const doing = relative(back ? firstLine(output) : str(now?.body['label']), cwd);
    return {
      kind: 'builtin',
      id: `${lead}#${call}`,
      call,
      agent,
      member: '',
      name: harness,
      avatar: '',
      color: '',
      handle: label || say('wisp.crew.team.subagent'),
      line: [model ? `${harness} · ${model}` : harness, doing].filter(Boolean).join(' — '),
      ...st,
      pending: false,
      place: here(say),
      startedAt: c.t,
      ...(back && answer && { endedAt: answer.t }),
    };
  });
}

/** Whether task `lead` has any subagent of its own (its Team tab shows for them too). */
export const hasSubagents = (tasks: readonly TaskRowLike[], steps: readonly StepRowLike[], lead: string): boolean =>
  tasks.some((r) => !!r.parent_id && (r.parent_id === lead || bare(r.parent_id) === bare(lead)) && !isCrewJob(r)) ||
  steps.some((e) => (e.session_id === lead || e.session_id === bare(lead)) && e.kind === 'tool' && e.body['tool'] === 'agent');

/** A built-in subagent's own steps (its view): every call that names it as parent, oldest first, and its answer. */
export function builtinSteps(steps: readonly StepRowLike[], lead: string, call: string, cwd = ''): { readonly steps: readonly string[]; readonly answer: string } {
  const mine = steps.filter((e) => e.session_id === lead || e.session_id === bare(lead)).slice().sort((a, b) => a.seq - b.seq);
  const c = mine.find((e) => e.kind === 'tool' && e.body['callId'] === call);
  const answer = str(c?.body['output']) || str(mine.find((e) => e.kind === 'tool_output' && e.body['callId'] === call)?.body['text']);
  return { steps: mine.filter((e) => e.kind === 'tool' && e.body['parent'] === call).map((e) => relative(str(e.body['label']), cwd)), answer: relative(answer, cwd) };
}

/** The task rows and steps the Team tab reads, made once per client: a readable made inside a computed each time it
 *  runs isn't one it follows, so a tab that should appear as a run's first subagent starts would wait for a reload. */
export interface SubagentData {
  readonly tasks: Readable<readonly TaskRowLike[]>;
  readonly steps: Readable<readonly StepRowLike[]>;
}
const madeFor = new WeakMap<PluginClient, SubagentData>();
export function subagentData(client: PluginClient): SubagentData {
  const had = madeFor.get(client);
  if (had) return had;
  // a task's steps come as one list row per task while it's open (its live tail: an array), or one row per step
  const rows = client.entities.list<StepRowLike | readonly StepRowLike[]>('event');
  const steps = computed(() => (rows() as readonly (StepRowLike | readonly StepRowLike[])[]).flatMap((r) => (Array.isArray(r) ? (r as readonly StepRowLike[]) : [r as StepRowLike])));
  const d: SubagentData = { tasks: client.entities.list<TaskRowLike>('session'), steps };
  madeFor.set(client, d);
  return d;
}
