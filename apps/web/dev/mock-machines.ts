// The mock's machines (apps/web/dev): v1's seeded three as the `machine` rows the seed has always sent, and, for
// ?seed=machines, as wisp.machines' own rows (`machines_peer`, `machines_status`) and `machines.list`'s views.
const HOUR = 3_600_000;
type Row = { e: string; id: string; v: unknown };

export const MACHINES = [
  {
    id: 'studio',
    peer: 'studio',
    name: 'studio-mac',
    transport: 'ssh',
    address: 'studio.local',
    port: 22,
    kind: 'ssh',
    target: 'studio.local',
    state: 'online',
    steps: [],
    error: '',
    stale: false,
    attempt: 0,
    projects: 3,
    working: 1,
    remote: { version: '1.0.0-alpha.1', platform: 'darwin', agents: [] },
    outdated: null,
    log: [],
  },
  {
    id: 'box',
    peer: 'box',
    name: 'mokh.grid',
    transport: 'grid',
    address: 'mokh.grid.trywisp.dev',
    port: 443,
    kind: 'grid',
    target: 'mokh.grid.trywisp.dev',
    state: 'asleep',
    steps: [],
    error: '',
    stale: false,
    attempt: 0,
    projects: 1,
    working: 0,
    remote: null,
    outdated: null,
    log: [],
  },
  {
    id: 'pi',
    peer: 'pi',
    name: 'raspberry',
    transport: 'ssh',
    address: '192.168.1.40',
    port: 22,
    kind: 'ssh',
    target: 'pi@192.168.1.40',
    state: 'error',
    steps: [],
    error: 'Connection refused',
    hint: { text: 'Is sshd running on the Pi?' },
    stale: false,
    attempt: 3,
    projects: 0,
    working: 0,
    remote: null,
    outdated: '1.0.0-alpha.0',
    log: [],
  },
];

/** MACHINES as wisp.machines' own rows: `machines_peer` (the peer as stored) and `machines_status` (its state). */
export function machineRows(peer: string, now: number): Row[] {
  return MACHINES.flatMap((m) => [
    { e: 'machines_peer', id: `${peer}:${m.id}`, v: { id: m.id, body: { id: m.id, name: m.name, transport: m.transport, address: m.target, port: m.port, addedAt: now - 30 * 24 * HOUR, autoConnect: true, ...(m.transport === 'grid' && { data: { grid: 'https://grid.trywisp.dev' } }) } } },
    {
      e: 'machines_status',
      id: `${peer}:${m.id}`,
      v: { id: m.id, body: { id: m.id, state: m.state, error: m.error, steps: m.steps, hint: m.hint ?? null, attempt: m.attempt, stale: null, remote: m.state === 'online' ? { version: '1.0.0-alpha.15', agents: [{ id: 'claude', name: 'Claude Code', available: true }, { id: 'codex', name: 'Codex', available: true }, { id: 'opencode', name: 'opencode', available: true }] } : m.remote } },
    },
  ]);
}

/** What `machines.list` adds to those rows (wisp.machines' PeerView): an update for the online one, its working tasks. */
export const MACHINE_VIEWS = MACHINES.map((m) => ({ id: m.id, outdated: m.state === 'online' ? '1.0.0-alpha.17' : null, working: m.working, log: m.log, defaultCommand: 'wisp' }));

