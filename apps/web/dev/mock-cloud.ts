// Settings → Cloud's accounts on the mock (ADR-0061): `grid-client.accounts`, `.accountSwitch` and `.accountRemove`
// answered as wisp.grid-client answers them (plugins/grid-client/api/bindings/grid-client/{Status,AccountInfo}.ts),
// each rewriting the cloud row as the plugin's view.rs does. Signed in with two accounts on ?seed=cloud
// (MOCK_CLOUD=1): ada on grid.trywisp.dev (current) and bo on wisp.acme.dev.

import type { EntityOp, Topic } from '@wisp/protocol';
import type { CommandCtx } from './mock-commands.ts';

export const CLOUD_TOPIC = 'plugin:wisp.grid-client:cloud' as Topic;

export interface MockAccount {
  key: string;
  grid: string;
  handle: string;
  machine: string;
  current: boolean;
}

const account = (grid: string, handle: string, current: boolean): MockAccount => ({ key: `${grid}#${handle}`, grid, handle, machine: '', current });

/** The seeded accounts (seed.ts's cloud row). */
export const SEED_ACCOUNTS: readonly MockAccount[] = [account('https://grid.trywisp.dev', 'ada', true), account('https://wisp.acme.dev', 'bo', false)];

type Row = Record<string, unknown> & { accounts?: MockAccount[] };

const refuse = (code: string, message: string): never => {
  throw { code, message };
};

/** The cloud row as it is now, and the id it's kept under. */
function current(ctx: CommandCtx): { id: string; row: Row } {
  for (const id of ['this', `${ctx.peer}:this`]) {
    const row = ctx.row(CLOUD_TOPIC, 'grid-client.cloud', id) as Row | undefined;
    if (row) return { id, row };
  }
  return { id: 'this', row: { signedIn: false, grid: '', handle: '', machine: '', computer: null, signin: null, accounts: [] } };
}

/** The row with `list` as its accounts, its own fields the current one's (view.rs `row`). */
function rewrite(ctx: CommandCtx, list: MockAccount[]): { ops: EntityOp[]; result: unknown } {
  const { id, row } = current(ctx);
  const c = list.find((a) => a.current);
  const v = { ...row, signedIn: !!c, grid: c?.grid ?? '', handle: c?.handle ?? '', machine: c?.machine ?? '', accounts: list };
  const status = { signedIn: v.signedIn, grid: v.grid, handle: v.handle, machine: v.machine, computer: row['computer'] ?? { on: false, state: 'off', label: '', url: '', error: '' }, accounts: list };
  return { ops: [{ topic: CLOUD_TOPIC, e: 'grid-client.cloud', id, op: 'upsert', v }], result: status };
}

const keyOf = (args: unknown): string => {
  const k = (args as { key?: unknown } | null)?.key;
  return typeof k === 'string' ? k : '';
};

export const CLOUD_COMMANDS: Record<string, (args: unknown, ctx: CommandCtx) => { ops: EntityOp[]; result?: unknown }> = {
  'grid-client.accounts': (_a, ctx) => ({ ops: [], result: current(ctx).row.accounts ?? [] }),
  'grid-client.accountSwitch': (a, ctx) => {
    const list = current(ctx).row.accounts ?? [];
    const key = keyOf(a);
    if (!list.some((x) => x.key === key)) return refuse('not_found', 'Not signed in to that account here.');
    return rewrite(ctx, list.map((x) => ({ ...x, current: x.key === key })));
  },
  'grid-client.accountRemove': (a, ctx) => {
    const list = current(ctx).row.accounts ?? [];
    const key = keyOf(a);
    const gone = list.find((x) => x.key === key);
    if (!gone) return refuse('not_found', 'Not signed in to that account here.');
    const left = list.filter((x) => x.key !== key);
    return rewrite(ctx, gone.current ? left.map((x, i) => ({ ...x, current: i === 0 })) : left);
  },
};
