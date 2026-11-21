// "All goals…" and "All recipes…" (owner-list #63): every row of a menu that grows with the formula, in a searchable
// picker: the palette's sheet (two lines a row: the name, what it means), a dialog on a desktop, a sheet from the bottom
// on a touch-first or narrow screen. What it lists and what a pick does stay here, keyed by the overlay's `key`
// param (an overlay's params are plain data): opened again after a reload, a key that's gone closes it.
import type { ExtensionId } from '@wisp/client';
import { SHEET_SCREEN } from '@wisp/ui/pure';
import type { ShellContext } from '../contract.ts';

/** the overlay view (builtins.tsx) */
export const PICK_VIEW = 'shell.pick';

export interface PickRow {
  readonly value: string;
  readonly label: string;
  readonly hint?: string;
}

export interface PickSpec {
  readonly title: string;
  readonly placeholder: string;
  readonly rows: readonly PickRow[];
  readonly onPick: (value: string) => void;
}

const open = new Map<string, PickSpec>();
let next = 0;

export const pickSpec = (key: string): PickSpec | undefined => open.get(key);
export const pickDone = (key: string): boolean => open.delete(key);

/** Opens the picker over `spec`'s rows; only the newest one stays (a second All goals… replaces the first). */
export function openPick(shell: Pick<ShellContext, 'navigate'>, spec: PickSpec): string {
  open.clear();
  const key = `p${++next}`;
  open.set(key, spec);
  shell.navigate({ t: 'openOverlay', overlay: { kind: globalThis.matchMedia?.(SHEET_SCREEN).matches ? 'sheet' : 'dialog', id: PICK_VIEW as ExtensionId, params: { key } } });
  return key;
}
