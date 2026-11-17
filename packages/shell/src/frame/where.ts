// New task's Run on (v1 launcher.js where-btn, F-comp-9/10): this computer first, then the computers you added, then
// the places plugins add (wisp.sandbox: a fresh cloud box, TRY-19), the picked one checked; picking one makes it the
// only place (v1 0.45: one machine at a time from the menu). A sandbox is never a computer here: it belongs to the
// task, member or judges it was made for (ADR-0047) and goes with them, so a task started on it would go too.
import type { IconName, MenuEntry } from '@wisp/ui';

/** what Run on's value is for a plugin's place (never a peer id: those have no `:`) */
export const targetValue = (id: string): string => `launch:${id}`;

export interface WhereTarget {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
}

export function whereItems(
  peers: readonly string[],
  picked: string,
  home: string,
  name: (p: string) => string,
  t: (k: string) => string,
  targets: readonly WhereTarget[] = [],
): MenuEntry[] {
  const others = peers.filter((p) => p !== home);
  return [
    { value: home, label: t('shell.card.thisComputer'), icon: 'laptop', checked: picked === home },
    ...others.map((p): MenuEntry => ({ value: p, label: name(p), icon: 'server', checked: picked === p })),
    ...targets.map((x): MenuEntry => ({ value: targetValue(x.id), label: x.label, icon: x.icon, checked: picked === targetValue(x.id) })),
  ];
}

/** The machines wisp keeps that are sandboxes (`machines_peer` rows whose transport data says `sandbox`), by peer id
 *  (the app names a kept machine's peer by its machine id: apps/web main.tsx followMachines). */
export function sandboxPeers(rows: readonly { readonly id: unknown; readonly body?: { readonly data?: unknown } | undefined }[]): ReadonlySet<string> {
  const out = new Set<string>();
  for (const r of rows) {
    const data = r.body?.data;
    if (data && typeof data === 'object' && (data as Record<string, unknown>)['sandbox']) {
      const id = String(r.id);
      out.add(id.slice(id.indexOf(':') + 1));
    }
  }
  return out;
}
