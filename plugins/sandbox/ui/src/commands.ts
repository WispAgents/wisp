// wisp.sandbox's commands as the app runs them (ADR-0047). Reads say what they wait for; writes the pane makes are
// shown at once where they can be (an env var's name, a sandbox going).
import { defineCommand, entityKey, type EntityKey, type PeerId } from '@wisp/client';
import { LAUNCH, type LaunchRow } from './launches.ts';
import './i18n/lazy.ts';

export interface SandboxView {
  readonly machine: string;
  readonly peer: string;
  readonly sandbox: string;
  readonly name: string;
  readonly owner: unknown;
  readonly state: string;
  /** only the grid holds it (made elsewhere, or its machine here gone): no peer to open, Delete goes by its id */
  readonly elsewhere?: boolean;
}

export interface Info {
  readonly enabled: boolean;
  readonly why?: string;
  readonly logins: readonly string[];
  readonly env: readonly string[];
  readonly sandboxes: readonly SandboxView[];
  /** how many of the user's the grid holds, and the most it keeps (a grid that doesn't say: absent) */
  readonly used?: number;
  readonly limit?: number;
}

export interface ClaudeSignIn {
  readonly state: 'idle' | 'waiting' | 'link' | 'saving' | 'done' | 'failed';
  readonly link?: string;
  readonly error?: string;
}

export interface LaunchArgs {
  readonly prompt: string;
  readonly cwd?: string;
  readonly agent?: string;
  readonly model?: string;
  readonly effort?: string;
  readonly mode?: string;
  readonly goal?: unknown;
}

/** Claude's sign-in for sandboxes as it goes (`id: claude`). */
export interface SigninRow extends ClaudeSignIn {
  readonly id: string;
}

const none = { optimistic: 'none' as const };
export const info = defineCommand<{ peer: PeerId }, Info>({ id: 'sandbox.info', owner: (a) => a.peer, wire: () => ({}), ...none, pending: 'the Sandboxes pane' });
export const launch = defineCommand<{ peer: PeerId; args: LaunchArgs }, { machine: string; peer: string; task: string; sandbox: string }>({
  id: 'sandbox.launch',
  owner: (a) => a.peer,
  wire: (a) => a.args,
  ...none,
  pending: 'the launching view (its placeholder card shows at once)',
});
const title = (a: LaunchArgs): string =>
  (a.prompt.split('\n').map((l) => l.trim()).find((l) => l) ?? '').slice(0, 80);

/** `/sandbox …`: its row shows at once (the list and its page), the sandbox comes up behind it. */
export const start = defineCommand<{ peer: PeerId; id: string; args: LaunchArgs; at: number }, Record<string, never>>({
  id: 'sandbox.start',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id, ...a.args }),
  optimistic: (_base, a) => {
    const v: LaunchRow = { id: a.id, title: title(a.args), state: 'preparing', step: 'make', at: a.at, agent: a.args.agent ?? '' };
    return [{ e: LAUNCH, id: entityKey(a.peer, a.id), op: 'upsert', v: v as never }];
  },
});
const patchRow = (key: EntityKey, row: LaunchRow | undefined, change: Partial<LaunchRow>) => (row ? [{ e: LAUNCH, id: key, op: 'upsert' as const, v: { ...row, ...change } as never }] : []);
/** Try again: preparing at once. */
export const retry = defineCommand<{ peer: PeerId; id: string }, Record<string, never>>({
  id: 'sandbox.retry',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id }),
  optimistic: (base, a) => {
    const key = entityKey(a.peer, a.id);
    return patchRow(key, base.peek<LaunchRow>(LAUNCH, key) as LaunchRow | undefined, { state: 'preparing', step: 'make', error: null });
  },
});
/** Dismiss a launch that failed: its row goes at once. */
export const dismiss = defineCommand<{ peer: PeerId; id: string }, Record<string, never>>({
  id: 'sandbox.dismiss',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id }),
  optimistic: (_base, a) => [{ e: LAUNCH, id: entityKey(a.peer, a.id), op: 'del' }],
});
export const claudeBegin = defineCommand<{ peer: PeerId }, ClaudeSignIn>({ id: 'sandbox.claude.begin', owner: (a) => a.peer, wire: () => ({}), ...none, pending: 'Sign in' });
// the pane says these refusals where they happened (under the env vars, on the row, under Sign in): `quiet`, no toast
export const claudeCode = defineCommand<{ peer: PeerId; code: string }, ClaudeSignIn>({ id: 'sandbox.claude.code', owner: (a) => a.peer, wire: (a) => ({ code: a.code }), ...none, pending: 'Done', quiet: true });
export const logout = defineCommand<{ peer: PeerId; agent: string }, Info>({ id: 'sandbox.logout', owner: (a) => a.peer, wire: (a) => ({ agent: a.agent }), ...none, pending: 'Sign out', quiet: true });
export const env = defineCommand<{ peer: PeerId; key: string; value: string }, Info>({
  id: 'sandbox.env',
  owner: (a) => a.peer,
  wire: (a) => ({ key: a.key, value: a.value }),
  ...none,
  pending: 'the env var row',
  quiet: true,
});
/** One sandbox: by its machine here, or (only the grid holds it) by its sandbox id on the grid. */
export const drop = defineCommand<{ peer: PeerId; machine?: string; sandbox?: string }, { dropped: number }>({
  id: 'sandbox.drop',
  owner: (a) => a.peer,
  wire: (a) => (a.machine ? { machine: a.machine } : { sandbox: a.sandbox }),
  ...none,
  pending: 'the sandbox row',
  quiet: true,
});
/** What Codex's sign-in answers (wisp.accounts `LoginView`): its page and code come a moment after it started. */
export interface LoginAnswer {
  readonly state?: string;
  readonly url?: string;
  readonly code?: string;
  readonly error?: string;
}
/** Codex's device login, run on the sandbox's own wisp (its peer). */
export const codexLogin = defineCommand<{ peer: PeerId }, LoginAnswer>({
  id: 'accounts.login.start',
  owner: (a) => a.peer,
  wire: () => ({ agent: 'codex' }),
  ...none,
  pending: 'Sign in to Codex there',
  quiet: true,
});
/** That sign-in as it stands there (its page once the CLI printed it, done, failed): the row follows it. */
export const codexState = defineCommand<{ peer: PeerId }, LoginAnswer>({
  id: 'accounts.login',
  owner: (a) => a.peer,
  wire: () => ({ agent: 'codex' }),
  ...none,
  pending: 'the Codex sign-in under its row',
  quiet: true,
});
