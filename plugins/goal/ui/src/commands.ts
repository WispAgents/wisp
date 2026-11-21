// The goal's writes (reference api COMMANDS: goal.set, goal.pause, goal.resume, goal.drop,
// goal.check). Each runs on the peer that owns the task and shows at once through an optimistic
// patch on the session row's `goal`; a refusal rolls it back and the shell shows the toast.
// `session` is the task's entity key (`<peer>:<id>`); `sessionId` is what the peer reads.

import { defineCommand, type EntityKey, type EntityOps, type EntityReader, type PeerId } from '@wisp/client';
import { optimisticGoal } from './rules.ts';
import { KIND, type GoalView, type Judge, type SessionRow, type Vote } from './types.ts';

export interface TaskArgs {
  readonly session: EntityKey;
  readonly sessionId: string;
}

export interface SetArgs extends TaskArgs {
  readonly goal: { readonly text: string; readonly judges?: readonly Judge[]; readonly vote?: Vote };
  /** check it at once when the task is idle (it may already hold) */
  readonly kick?: boolean;
  /** a follow-up goes with it: the goal waits for that turn instead of a check now */
  readonly sending?: boolean;
  /** the page's clock, for setAt on a new goal */
  readonly now: number;
}

const rowOf = (base: EntityReader, key: EntityKey): SessionRow | undefined => base.peek<SessionRow>(KIND.session, key) as SessionRow | undefined;

const putGoal = (key: EntityKey, goal: GoalView): EntityOps => [{ e: KIND.session, id: key, op: 'patch', ops: [{ op: 'add', path: '/goal', value: goal }] }];

/** A judge as `goal.set` and a task's spec name it (wisp.goal host `pick_of`). */
export const judgeWire = (j: Judge): string => ('crew' in j ? j.crew : [j.agent, j.model, j.effort].join('/').replace(/\/+$/, ''));

export const setGoal = defineCommand<SetArgs, GoalView>({
  id: 'goal.set',
  owner: (a) => a.session,
  // the judges and vote go too (B44: they were dropped), each judge as the server names one: a crew member or profile by
  // name, else `harness/model/effort` (the trailing empty parts left out)
  wire: (a) => ({
    id: a.session.slice(a.session.indexOf(':') + 1),
    text: a.goal.text,
    ...(a.kick && { kick: true }),
    // given (even empty: no judges, B49) or not (the spec's, else the defaults)
    ...(a.goal.judges && { judges: a.goal.judges.map(judgeWire) }),
    ...(a.goal.vote && { vote: a.goal.vote }),
  }),
  optimistic: (base, a) => {
    const row = rowOf(base, a.session);
    if (!row) return [];
    const next = optimisticGoal(row.goal, a.goal.text, {
      sending: a.sending === true,
      running: row.state === 'running',
      now: a.now,
      ...(a.goal.judges && { judges: a.goal.judges }),
      ...(a.goal.vote && { vote: a.goal.vote }),
    });
    return putGoal(a.session, next);
  },
  dedupe: (a) => `goal.set:${a.session}:${a.goal.text}`,
});

/** Pause and Resume: a round that waited for the plan leaves Scheduled with it (v1 goalAction). */
function moved(base: EntityReader, a: TaskArgs, status: 'paused' | 'working', why?: string): EntityOps {
  const row = rowOf(base, a.session);
  if (!row?.goal) return [];
  const { held: _held, why: _why, ...rest } = row.goal;
  return putGoal(a.session, { ...rest, status, ...(why && { why }) });
}

export const pauseGoal = defineCommand<TaskArgs & { readonly why?: string }, GoalView>({
  id: 'goal.pause',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.session.slice(a.session.indexOf(':') + 1), ...('why' in a && a.why !== undefined && { why: a.why }) }),
  optimistic: (base, a) => moved(base, a, 'paused', a.why),
});

export const resumeGoal = defineCommand<TaskArgs, GoalView>({
  id: 'goal.resume',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.session.slice(a.session.indexOf(':') + 1) }),
  optimistic: (base, a) => moved(base, a, 'working'),
});

export const dropGoal = defineCommand<TaskArgs, { dropped: boolean }>({
  id: 'goal.drop',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.session.slice(a.session.indexOf(':') + 1) }),
  optimistic: (base, a) => (rowOf(base, a.session)?.goal ? [{ e: KIND.session, id: a.session, op: 'patch', ops: [{ op: 'remove', path: '/goal' }] }] : []),
});

/** Check now: a task whose turn ended, if its goal is working. The server's patch says checking. */
export const checkGoal = defineCommand<TaskArgs, Record<string, never>>({
  id: 'goal.check',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.session.slice(a.session.indexOf(':') + 1) }),
  optimistic: 'none', pending: 'the goal.check control',
});

export const COMMANDS = [setGoal, pauseGoal, resumeGoal, dropGoal, checkGoal] as const;

// New task's Judges (HomeJudges.tsx): the `goal` resource's defaults, read and kept (v1 defaults.goalJudges / goalVote)
export interface GoalResource {
  readonly kind: string;
  readonly name: string;
  readonly spec: Record<string, unknown>;
}

/** The `goal` resource as the server has it (the default judges, vote and round limit). */
export const goalDefaultsNow = defineCommand<{ peer: PeerId }, GoalResource[]>({
  id: 'resources.export',
  owner: (a) => a.peer,
  wire: () => ({ kinds: ['goal'] }),
  optimistic: 'none',
  pending: 'the judges',
});

/** The `goal` resource with these judges and vote; its other fields (the round limit) as they were. */
export function goalDefaults(have: readonly GoalResource[], judges: readonly Judge[], vote: Vote): GoalResource {
  const spec = { ...(have.find((r) => r.kind === 'goal' && r.name === 'default')?.spec ?? {}) };
  delete spec['judges'];
  delete spec['vote'];
  return { kind: 'goal', name: 'default', spec: { ...spec, ...(judges.length && { judges: judges.map(judgeWire) }), vote } };
}

/** A judge as the server names it (`judgeWire`'s inverse): a crew member's id, else `harness/model/effort` or a harness. */
export function judgeOf(name: string, crew: readonly { readonly id: string }[]): Judge {
  const [agent = '', model = '', effort = ''] = name.split('/');
  return !name.includes('/') && crew.some((m) => m.id === name) ? { crew: name } : { agent, model, effort };
}

/** A judge from a goal's view: the server sends each as `{agent: <its wire name>, model: '', effort: ''}`; a name with
 *  parts or a crew id is read back as the judge it names, one already split stays as it is. */
export function viewJudge(j: Judge, crew: readonly { readonly id: string }[]): Judge {
  if ('crew' in j || j.model || j.effort) return j;
  const back = judgeOf(j.agent, crew);
  return j.name ? { ...back, name: j.name } : back;
}

/** Keep these as the default judges and vote. */
export const saveGoalDefaults = defineCommand<{ peer: PeerId; judges: readonly Judge[]; vote: Vote; have: readonly GoalResource[] }>({
  id: 'resources.apply',
  owner: (a) => a.peer,
  wire: (a) => ({ resources: [goalDefaults(a.have, a.judges, a.vote)], trust: true }),
  // the button shows them at once (this device's store); a refusal is the shell's toast
  optimistic: 'none',
  pending: 'Judges',
});

/** The goals declared by name on peer `peer` (`goals:` in a formula, ADR-0110): `/goal ` suggests them. A read. */
export const goalsDeclared = defineCommand<{ peer: PeerId }, unknown>({ id: 'goal.declared', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the goals', quiet: true });
