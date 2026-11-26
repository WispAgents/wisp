// Fork… (v1 actions.js forkTask, QA round 14 desktop #1): a new task that carries on from one of this one's turns (the
// latest finished by default), with the conversation up to there and its files as they were then, on a branch of its
// own; this one stays as it is. An earlier turn forks only where its harness kept where that turn left the conversation
// (runtime's fork.rs: Claude Code's `--resume-session-at`); elsewhere only the latest does, never with later context.
// The dialog asks what the fork should try (it's named after it), its mode, harness, model and thinking among the
// harnesses on the task's machine; nothing is sent until its Fork. DOM-free.

export interface ForkPick {
  readonly prompt: string;
  readonly mode: string;
  readonly agent: string;
  readonly model: string;
  readonly effort: string;
}

export interface ForkSource {
  readonly agent: string;
  readonly model?: string | undefined;
  readonly effort?: string | undefined;
  readonly mode?: string | undefined;
}

const MODES = ['plan', 'edits', 'auto'];

/** What the dialog starts with (v1): the task's harness if its machine has it (else the first it has), its model and
 *  thinking on that harness, its mode. */
export function startPick(s: ForkSource, here: readonly string[]): ForkPick {
  const agent = here.length === 0 || here.includes(s.agent) ? s.agent : (here[0] ?? s.agent);
  const same = agent === s.agent;
  return { prompt: '', mode: s.mode && MODES.includes(s.mode) ? s.mode : 'auto', agent, model: same ? (s.model ?? '') : '', effort: same ? (s.effort ?? '') : '' };
}

/** The turn it forks from: the one asked for, else the latest that finished; none while the first still runs. */
export function forkTurn<T extends { readonly id: string; readonly status?: string | undefined }>(turns: readonly T[], asked?: string): T | undefined {
  const done = turns.filter((t) => t.status !== 'running' && t.status !== 'starting');
  const at = asked ? turns.find((t) => t.id === asked) : undefined;
  return at && at.status !== 'running' && at.status !== 'starting' ? at : done.at(-1);
}

/** A turn as far as `forkReach` reads it: a store row's ext is under `body`, `runtime.turns`' at the top. */
export interface ForkPoint {
  readonly id: string;
  readonly ext?: Readonly<Record<string, unknown>> | undefined;
  readonly body?: { readonly ext?: Readonly<Record<string, unknown>> | undefined } | undefined;
}

/** What a fork from `at` carries on from, as runtime's fork does it: `whole` from the latest turn; `upTo` an earlier
 *  turn whose harness kept where it left the conversation (`resumeAt`); `latestOnly` an earlier one without (Codex and
 *  the rest): runtime refuses it rather than fork with the turns after it, so the dialog says so and won't send it. */
export function forkReach(turns: readonly ForkPoint[], at: ForkPoint | undefined): 'whole' | 'upTo' | 'latestOnly' {
  if (!at || at.id === turns.at(-1)?.id) return 'whole';
  const own = (at.ext ?? at.body?.ext)?.['wisp.runtime'] as { readonly resumeAt?: unknown } | undefined;
  return typeof own?.resumeAt === 'string' && own.resumeAt !== '' ? 'upTo' : 'latestOnly';
}

/** The two commands its Fork sends, in order: the copy (`runtime.fork`, after `turnId`), then what it should try as the
 *  copy's first follow-up, on the harness and mode picked (`runtime.followUp`). */
export function forkSteps(
  p: ForkPick,
  o: { readonly of: string; readonly newId: string; readonly turnId: string | undefined; readonly firstTurnId: string; readonly idx: number; readonly at: number },
) {
  return {
    // named after what it's asked (v1): its first line, cut at a word near 60
    fork: { id: o.of, newId: o.newId, at: o.at, title: forkTitle(p.prompt), ...(p.agent && { agent: p.agent }), ...(o.turnId && { turnId: o.turnId }) },
    followUp: {
      id: o.newId,
      text: p.prompt.trim(),
      turnId: o.firstTurnId,
      idx: o.idx,
      at: o.at,
      mode: p.mode,
      run: { ...(p.agent && { agent: p.agent }), ...(p.model && { model: p.model }), ...(p.effort && { effort: p.effort }) },
    },
  };
}

/** The fork's name: what it was asked, its first line, cut at a word before 60 characters. */
export function forkTitle(prompt: string): string {
  const line = prompt.trim().split('\n')[0]?.trim() ?? '';
  if (line.length <= 60) return line;
  const cut = line.slice(0, 60);
  const at = cut.lastIndexOf(' ');
  return `${(at > 30 ? cut.slice(0, at) : cut).trimEnd()}…`;
}
