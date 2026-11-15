// Add project's names (Projects.tsx): where a found repo is and the name a new project gets. Plain, so their
// tests needn't compile the pane.

/** Where a found repo is, as v1 showed it: its folder's parent, your home as `~` (`/Users/me/code/web` → `~/code`). */
export function whereIs(path: string): string {
  const parent = path.replace(/\/+$/, '').replace(/\/[^/]*$/, '') || '/';
  return parent.replace(/^\/(?:Users|home)\/[^/]+/, '~').replace(/^[A-Za-z]:\\Users\\[^\\]+/, '~');
}

/** A project's name from its folder or URL: its last part, without `.git`, as a resource name. */
export function nameFor(source: string, taken: readonly string[]): string {
  const last = source.trim().replace(/\/+$/, '').split(/[/:]/).pop() ?? '';
  const base =
    last
      .replace(/\.git$/, '')
      .replace(/[^A-Za-z0-9._-]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'project';
  let name = base;
  for (let i = 2; taken.includes(name); i++) name = `${base}-${i}`;
  return name;
}

/** A typed folder without its trailing `/`, `/.` (`/repo/.` is `/repo`), as the server keeps it. */
export function folderOf(path: string): string {
  return path.trim().replace(/(?:[/\\]\.?)+$/, '');
}

/**
 * The name `runtime.projectAdd` will most likely give a new folder (ADR-0077's rule: the name asked for, else its last
 * part without `.git`, lower case, spaces as `-`; `-2`, `-3`… when taken). Only the optimistic row's until the server's
 * answer lands: the server names it, in its store's transaction.
 */
export function addedName(source: string, taken: readonly string[], asked?: string): string {
  const last = (folderOf(source).split(/[/\\:]/).pop() ?? '').replace(/\.git$/, '').toLowerCase().replace(/ /g, '-');
  const base = asked?.trim() || last || 'project';
  let name = base;
  for (let i = 2; taken.includes(name); i++) name = `${base}-${i}`;
  return name;
}
