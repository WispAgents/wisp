// A menu's entries made again from the same task are the same menu: compared by what they say and do, so a memo can
// hand the menu the array it already has. The kit keys a menu's rows by object, so new objects for the same rows threw
// away and rebuilt the whole closed menu on every patch to the open task: 404 DOM changes in 100 streamed tokens (perf
// r1 #12, #15). Pure.
import type { MenuEntry } from '@wisp/ui';

const KEYS = ['kind', 'value', 'id', 'part', 'label', 'icon', 'hint', 'keys', 'danger', 'disabled', 'checked'] as const;

function sameEntry(a: MenuEntry, b: MenuEntry): boolean {
  const x = a as unknown as Record<string, unknown>;
  const y = b as unknown as Record<string, unknown>;
  for (const k of KEYS) if (x[k] !== y[k]) return false;
  const xs = x['items'] as readonly MenuEntry[] | undefined;
  const ys = y['items'] as readonly MenuEntry[] | undefined;
  return xs === ys || (xs !== undefined && ys !== undefined && sameEntries(xs, ys));
}

/** The same rows, in the same order, saying and doing the same: a memo's `equals` keeps the menu as it is. */
export function sameEntries(a: readonly MenuEntry[], b: readonly MenuEntry[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!sameEntry(a[i] as MenuEntry, b[i] as MenuEntry)) return false;
  return true;
}
