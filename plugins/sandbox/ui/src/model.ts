// Sandboxes as the app shows them (ADR-0047). Pure: `/sandbox …` read off New task's text, an owner said in words,
// and which machines are sandboxes.

/** `/sandbox <what to do>`: the task's prompt, or null when the text isn't one ('' when nothing came after it). */
export function sandboxPrompt(text: string): string | null {
  const m = /^\/sandbox(?:\s+([\s\S]*))?$/i.exec(text.trimStart());
  if (!m) return null;
  return (m[1] ?? '').trim();
}

/** A sandbox's owner (wisp-sandbox-core `Owner`), as stored on its machine. */
export type Owner =
  | { readonly task: string }
  | { readonly pending: number }
  | { readonly lead: string; readonly member: string }
  | { readonly crew: string; readonly job: string }
  | { readonly crew: string }
  | { readonly judges: string };

export type Say = (key: string, vars?: Readonly<Record<string, string | number>>) => string;

/** Whose it is, in words: a task's, a delegate's, a crew job's or member's own, a goal's judges'. */
export function ownerWords(owner: unknown, say: Say, name: (crewId: string) => string = (c) => c): string {
  const o = (owner ?? {}) as Record<string, unknown>;
  if (typeof o['task'] === 'string') return say('sandbox.owner.task');
  if (typeof o['pending'] === 'number') return say('sandbox.owner.pending');
  if (typeof o['lead'] === 'string') return say('sandbox.owner.delegate', { member: String(o['member'] ?? '') });
  if (typeof o['crew'] === 'string' && typeof o['job'] === 'string') return say('sandbox.owner.job', { name: name(o['crew'] as string) });
  if (typeof o['crew'] === 'string') return say('sandbox.owner.member', { name: name(o['crew'] as string) });
  if (typeof o['judges'] === 'string') return say('sandbox.owner.judges');
  return say('sandbox.owner.none');
}

/** The task a sandbox belongs to, when it's a task's (to open it). */
export const ownerTask = (owner: unknown): string | undefined => {
  const t = (owner as { task?: unknown } | null)?.task;
  return typeof t === 'string' && t ? t : undefined;
};

/** Why an env var's name won't go, as the grid's `env_key_ok` says it (apps/grid sandboxes.rs), so Add says it before
 *  sending: `empty`, `long` (over 64), `digit` (starts with one), `shape` (not letters, digits and _), `reserved`
 *  (wisp's own: WISP_…, LD_…, PATH, HOME, …). Undefined: it may go. */
export type EnvKeyProblem = 'empty' | 'long' | 'digit' | 'shape' | 'reserved';
const RESERVED = ['CLAUDE_CODE_OAUTH_TOKEN', 'PATH', 'HOME', 'USER', 'SHELL', 'NODE_OPTIONS', 'HOSTNAME'];
export function envKeyProblem(name: string): EnvKeyProblem | undefined {
  if (!name) return 'empty';
  if (name.length > 64) return 'long';
  if (/^[0-9]/.test(name)) return 'digit';
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return 'shape';
  if (name.startsWith('WISP_') || name.startsWith('LD_') || RESERVED.includes(name)) return 'reserved';
  return undefined;
}

/** One sandbox as the pane lists it (wisp.sandbox `SandboxView`): `elsewhere`, only the grid holds it. */
export interface Listed {
  readonly machine: string;
  readonly peer: string;
  readonly sandbox: string;
  readonly name: string;
  readonly owner: unknown;
  readonly state: string;
  readonly elsewhere?: boolean;
}

/** A `machines_peer` row and a `machines_status` row, as the entity store keeps them (`<home>:<machine>`). */
export interface MachineEntity {
  readonly id: string;
  readonly body?: { readonly id?: string; readonly name?: string; readonly data?: Readonly<Record<string, unknown>> | null } | null;
}
export interface StatusEntity {
  readonly id: string;
  readonly body?: { readonly id?: string; readonly state?: string } | null;
}

const machineOf = (r: { readonly id: string; readonly body?: { readonly id?: string } | null }): string => r.body?.id ?? r.id.slice(r.id.indexOf(':') + 1);

/** The pane's list, live: the sandboxes this computer keeps as its machine rows say them now (a launch's shows as it
 *  lands, a dropped one goes, online/asleep follows its link), then the ones only the grid holds (from the last
 *  `sandbox.info`). Before the machine rows are in, `sandbox.info`'s own. `gone`: sandbox ids deleted here, kept out
 *  whatever an older answer still says. */
export function liveList(info: readonly Listed[], machines: readonly MachineEntity[], status: readonly StatusEntity[], gone: ReadonlySet<string>): Listed[] {
  const state = new Map(status.map((s) => [machineOf(s), s.body?.state ?? '']));
  const kept: Listed[] = [];
  for (const m of machines) {
    const data = m.body?.data;
    const sandbox = data && typeof data['sandbox'] === 'string' ? data['sandbox'] : '';
    if (!sandbox) continue;
    const id = machineOf(m);
    const was = info.find((l) => l.machine === id);
    kept.push({ machine: id, peer: id, sandbox, name: m.body?.name ?? was?.name ?? sandbox, owner: data?.['owner'] ?? null, state: state.get(id) ?? was?.state ?? '' });
  }
  const here = kept.length > 0 ? kept : info.filter((l) => !l.elsewhere);
  const out = [...here, ...info.filter((l) => l.elsewhere && !here.some((h) => h.sandbox === l.sandbox))];
  return out.filter((l) => !gone.has(l.sandbox));
}

/** Codex signing in on a sandbox, as its row shows it: started (no page yet), the page and its code, done, failed. */
export type CodexFlow =
  | { readonly state: 'starting' }
  | { readonly state: 'link'; readonly url: string; readonly code?: string }
  | { readonly state: 'done' }
  | { readonly state: 'failed'; readonly error: string };

/** `accounts.login(.start)`'s answer there, as the row's flow: its page once it has one (the CLI prints it a moment
 *  after it started), done, failed or cancelled. */
export function codexFlow(r: { readonly state?: string; readonly url?: string | null; readonly code?: string | null; readonly error?: string | null } | undefined): CodexFlow {
  if (r?.state === 'done') return { state: 'done' };
  if (r?.state === 'failed' || r?.state === 'cancelled') return { state: 'failed', error: r.error ?? '' };
  if (r?.url) return r.code ? { state: 'link', url: r.url, code: r.code } : { state: 'link', url: r.url };
  return { state: 'starting' };
}

/** The flow's end: nothing more to follow. */
export const codexEnded = (f: CodexFlow | undefined): boolean => f?.state === 'done' || f?.state === 'failed';

/** An error as the pane shows it. */
export const errorText = (e: unknown): string => (e instanceof Error ? e.message : typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : String(e));
