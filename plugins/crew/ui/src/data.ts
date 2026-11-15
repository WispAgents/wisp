// The crew as the screens read it: members from the entity store (topic `crew`), their jobs from
// the task list (topic `list`), and what each is up to. Selectors only: one per plugin client,
// incremental where it matters (filterBy patches in place when a token updates one of 5,000 tasks,
// so the crew's own work stays proportional to its few dozen jobs).

import { computed, filterBy, groupBy, ownerOf, type EntityKey, type PeerId, type PluginClient, type Readable } from '@wisp/client';
import { jobOf } from './jobs.ts';
import { rankCrew } from './model.ts';
import { memberState, type MemberState } from './rules/feed.ts';
import { KIND, type MemberView, type SessionRow } from './types.ts';

/** A member row with the key the store holds it under (its peer is the key's prefix). */
export type CrewMember = MemberView & { readonly key: EntityKey; readonly peer: PeerId };

export interface CrewData {
  readonly members: Readable<readonly CrewMember[]>;
  /** every task that is a crew job (the list's rows with `crew`) */
  readonly jobs: Readable<readonly SessionRow[]>;
  readonly states: Readable<ReadonlyMap<string, MemberState<SessionRow>>>;
  /** most-needed first */
  readonly ranked: Readable<readonly CrewMember[]>;
  byId(id: string | undefined): CrewMember | undefined;
  /** the task row's key, for opening it and for commands */
  sessionKey(id: string): EntityKey | undefined;
}

const made = new WeakMap<PluginClient, CrewData>();

const EMPTY: MemberState<SessionRow> = { jobs: [], sent: [], asks: false, live: [], unread: 0, mood: 'idle' };
export const emptyState = (paused: boolean): MemberState<SessionRow> => ({ ...EMPTY, mood: paused ? 'sleep' : 'idle' });

/** wisp.machines' row for a machine this wisp follows as a peer of its own (`machines_peer`, `{id, body: {peer, name}}`). */
interface MachinePeerRow {
  readonly id: string;
  readonly body?: { readonly peer?: string | null; readonly name?: string };
}
const MACHINES_PEER = 'machines_peer';

/** The machines this wisp follows, by the peer id the app gives them (their own id, apps/web followMachines) → name. */
export function machinesByPeer(rows: readonly MachinePeerRow[]): ReadonlyMap<string, string> {
  const out = new Map<string, string>();
  for (const r of rows) {
    if (!r.body?.peer) continue;
    const id = String(r.id).slice(String(r.id).indexOf(':') + 1);
    out.set(id, r.body.name || id);
  }
  return out;
}

/** A `crew_member` row as the crew host writes it (`{id, name, updated_at, body: Member}`), read as the member; a row
 * already flat (dev data, an optimistic upsert) is its own. Before this the screens read `role`, `avatar`, `paused`… off
 * the row itself, where they never are (round 16). */
export function memberOf(row: unknown): MemberView {
  const r = (row ?? {}) as Record<string, unknown>;
  const body = r['body'];
  if (!body || typeof body !== 'object') return r as unknown as MemberView;
  const { body: _b, ...top } = r;
  return { ...(body as Record<string, unknown>), ...top } as unknown as MemberView;
}

export function crewData(client: PluginClient): CrewData {
  const had = made.get(client);
  if (had) return had;
  const e = client.entities;
  const keys = e.keys(KIND.member);
  const rows = e.list<MemberView>(KIND.member);
  const members = computed(() => {
    const ks = keys();
    return rows().map((v, i) => {
      const key = ks[i] as EntityKey;
      return { ...memberOf(v), key, peer: ownerOf(key) } as CrewMember;
    });
  });
  // every peer's task rows, each read as a job when it is one (jobs.ts: body.ext["wisp.crew"]); a job on a member's
  // machine is that machine's row: it says which peer it came from and the machine's name (wisp.machines' rows)
  const rowsOf = e.list<Parameters<typeof jobOf>[0]>(KIND.session);
  const sessionKeys = e.keys(KIND.session);
  const machineRows = e.list<MachinePeerRow>(MACHINES_PEER);
  const machineNames = computed(() => machinesByPeer(machineRows()));
  const sessions = computed(() => {
    const ks = sessionKeys();
    const names = machineNames();
    return rowsOf().map((r, i) => {
      const job = jobOf(r);
      if (!job) return { id: r.id, title: r.title ?? '', agent: r.agent ?? '', cwd: '', state: r.state ?? 'idle', createdAt: r.created_at ?? 0, updatedAt: r.updated_at ?? 0 } as SessionRow;
      const k = String(ks[i] ?? '');
      const peer = k.includes(':') ? k.slice(0, k.indexOf(':')) : '';
      const machine = names.get(peer);
      return { ...job, ...(peer && { peer }), ...(machine && { machine }) };
    });
  }) as Readable<readonly SessionRow[]>;
  const jobs = filterBy(sessions, (s) => !!s.crew);
  const byCrew = groupBy(jobs, (s) => s.crew?.id ?? '');
  const states = computed(() => {
    const g = byCrew();
    return new Map(members().map((m) => [m.id, memberState(m, g.get(m.id) ?? [])]));
  });
  const ranked = computed(() => rankCrew(members(), states(), client.scope.timers.now()));
  const data: CrewData = {
    members,
    jobs,
    states,
    ranked,
    byId: (id) => (id ? members().find((m) => m.id === id || m.key === id) : undefined),
    sessionKey: (id) => sessionKeys().find((k) => k === id || k.endsWith(`:${id}`)),
  };
  made.set(client, data);
  return data;
}
