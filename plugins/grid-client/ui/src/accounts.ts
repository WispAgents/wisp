// Several grid accounts on Settings → Cloud (ADR-0061, v1 web/js/features/cloud.js Add an account): the row's
// accounts, and what Switch and Sign out do to it at once (the optimistic patch; the server's row replaces it, a
// refusal rolls it back). Pure, so the UI test imports it without a DOM.
import type { CloudRow } from './commands.ts';

export interface Account {
  /** `<grid>#<handle>` */
  readonly key: string;
  readonly grid: string;
  readonly handle: string;
  readonly machine: string;
  readonly current: boolean;
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

/** The accounts the row lists, in sign-in order; anything malformed dropped. An older server's row (no
 *  `accounts`) still shows its one account. */
export function accountsOf(r: Pick<CloudRow, 'accounts' | 'signedIn' | 'grid' | 'handle' | 'machine'> | undefined): readonly Account[] {
  if (!r) return [];
  const list = Array.isArray(r.accounts) ? (r.accounts as readonly unknown[]) : undefined;
  if (!list) return r.signedIn && r.handle ? [{ key: `${r.grid}#${r.handle}`, grid: r.grid, handle: r.handle, machine: r.machine, current: true }] : [];
  return list.flatMap((x) => {
    const o = (x ?? {}) as Record<string, unknown>;
    const key = str(o['key']);
    return key ? [{ key, grid: str(o['grid']), handle: str(o['handle']), machine: str(o['machine']), current: o['current'] === true }] : [];
  });
}

/** The row's current-account fields from its accounts (none: signed out). */
const withCurrent = (r: CloudRow, list: readonly Account[]): Partial<CloudRow> => {
  const c = list.find((a) => a.current);
  return { accounts: list, signedIn: !!c, grid: c?.grid ?? '', handle: c?.handle ?? '', machine: c?.machine ?? '', computer: c?.key === `${r.grid}#${r.handle}` ? r.computer : r.computer ? { ...r.computer, on: false, state: 'off' } : null };
};

/** Switch: `key` current at once (unknown: nothing changes). The tunnel follows the server's answer. */
export function switched(r: CloudRow, key: string): Partial<CloudRow> {
  const list = accountsOf(r);
  if (!list.some((a) => a.key === key)) return {};
  return withCurrent(r, list.map((a) => ({ ...a, current: a.key === key })));
}

/** Sign out of `key`: gone at once; if it was current, the first one left is. */
export function removed(r: CloudRow, key: string): Partial<CloudRow> {
  const list = accountsOf(r);
  const gone = list.find((a) => a.key === key);
  if (!gone) return {};
  const left = list.filter((a) => a.key !== key);
  const next = gone.current ? left.map((a, i) => ({ ...a, current: i === 0 })) : left;
  return withCurrent(r, next);
}

/** `grid.trywisp.dev` for `https://grid.trywisp.dev`. */
export const hostOf = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');
