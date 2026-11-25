// SettingsSplit's keys, plain (a test reads them without the kit): Up and Down step the pick (no wrap: a list of a
// few, its ends are where they are), Home and End jump to its ends, Enter and Right go into the picked one's detail.

/** What a key does on the list: the id to pick, and whether focus goes on into the detail; undefined: not ours. */
export function listStep(key: string, ids: readonly string[], current: string): { pick?: string; detail?: true } | undefined {
  if (ids.length === 0) return undefined;
  const at = ids.indexOf(current);
  const to = (i: number): { pick: string } => ({ pick: ids[Math.max(0, Math.min(ids.length - 1, i))] as string });
  switch (key) {
    case 'ArrowDown':
      return to(at < 0 ? 0 : at + 1);
    case 'ArrowUp':
      return to(at < 0 ? 0 : at - 1);
    case 'Home':
      return to(0);
    case 'End':
      return to(ids.length - 1);
    case 'Enter':
    case 'ArrowRight':
      return at < 0 ? { ...to(0), detail: true } : { detail: true };
    default:
      return undefined;
  }
}
