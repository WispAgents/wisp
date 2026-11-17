// Delegate's team (v1 team.js): who the lead can hand work to, each on a harness, model and thinking level, with
// the lead or on another machine, in copies, with the name the lead calls it and what it's for. Pure: the rows,
// their names and what New task sends (`runtime.delegate` reads `{name, agent, model?, effort?, count?, machine?,
// use?}`, plugins/runtime/host/src/more/delegate.rs).

/** One delegate as the panel edits it. `modelLabel` is only for its button; it isn't sent. */
export interface Delegate {
  readonly name: string;
  readonly agent: string;
  readonly model?: string;
  readonly modelLabel?: string;
  readonly effort?: string;
  readonly use?: string;
  /** identical copies that take tasks in parallel (v1 MAX_COPIES 5) */
  readonly count?: number;
  /** '' or absent: with the lead; `here`, `sandbox` or a machine's peer id: away, on a copy of the lead's work */
  readonly machine?: string;
}

/** v1 team.js MAX_COPIES: parallel copies of one row */
export const MAX_COPIES = 5;

export const copiesOf = (m: Pick<Delegate, 'count'>): number => Math.max(1, Math.min(MAX_COPIES, Math.floor(m.count ?? 1)));

/** Agents in all, counting copies (v1 teamSize: the foot's "2 agents"). */
export const teamSize = (team: readonly Delegate[]): number => team.reduce((n, m) => n + copiesOf(m), 0);

const slug = (s: string): string =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 24);

/** The names the lead calls them (v1 uniqueNames, the server's rule): a blank one is its harness's id, a repeat
 *  gets -2, -3… */
export function uniqueNames<M extends Pick<Delegate, 'name' | 'agent'>>(members: readonly M[]): M[] {
  const taken = new Set<string>();
  return members.map((m) => {
    const base = slug(m.name) || m.agent;
    let name = base;
    for (let i = 2; taken.has(name); i++) name = `${base}-${i}`;
    taken.add(name);
    return { ...m, name };
  });
}

/** Add delegate's harness (v1 team.js add): the first one not yet on the team (nor the lead), else any but the
 *  lead, else the first. `ids`: the installed harnesses, in their order. */
export function nextAgent(lead: string, team: readonly Delegate[], ids: readonly string[]): string | undefined {
  const used = new Set([lead, ...team.filter((m) => !m.machine).map((m) => m.agent)]);
  return ids.find((a) => !used.has(a)) ?? ids.find((a) => a !== lead) ?? ids[0];
}

/** The team Settings starts New task with (`runtime.subagents`, v1 defaults.subagents): its well-formed rows. */
export function teamFromSetting(setting: unknown): Delegate[] {
  if (!Array.isArray(setting)) return [];
  return setting.flatMap((r: unknown): Delegate[] => {
    if (!r || typeof r !== 'object') return [];
    const o = r as Record<string, unknown>;
    if (typeof o['agent'] !== 'string' || !o['agent']) return [];
    const str = (k: string): string | undefined => (typeof o[k] === 'string' && o[k] ? (o[k] as string) : undefined);
    const count = typeof o['count'] === 'number' ? o['count'] : undefined;
    return [
      {
        name: str('name') ?? '',
        agent: o['agent'] as string,
        ...((v) => (v ? { model: v } : {}))(str('model')),
        ...((v) => (v ? { effort: v } : {}))(str('effort')),
        ...((v) => (v ? { use: v } : {}))(str('use')),
        ...((v) => (v ? { machine: v } : {}))(str('machine')),
        ...(count !== undefined && count > 1 && { count: copiesOf({ count }) }),
      },
    ];
  });
}

/** A delegate as New task sends it. */
export interface SentDelegate {
  readonly name: string;
  readonly agent: string;
  readonly model?: string;
  readonly effort?: string;
  readonly use?: string;
  readonly count?: number;
  readonly machine?: string;
}

/** What New task sends as its team: every row (no cap, v1 #team-cap), named as the lead will call them, without the
 *  words that are only for the panel; one copy and "with the lead" are left out. */
export function teamToSend(team: readonly Delegate[]): SentDelegate[] {
  return uniqueNames(team).map((m) => ({
    name: m.name,
    agent: m.agent,
    ...(m.model && { model: m.model }),
    ...(m.effort && { effort: m.effort }),
    ...(m.use?.trim() && { use: m.use.trim() }),
    ...(copiesOf(m) > 1 && { count: copiesOf(m) }),
    ...(m.machine && { machine: m.machine }),
  }));
}
