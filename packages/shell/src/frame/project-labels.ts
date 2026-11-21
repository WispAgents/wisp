// Same-named projects told apart (r285): two projects called "wisp" read "wisp (Studio)" and "wisp (This computer)"
// when they're on different computers, "wisp (~/code)" and "wisp (~/Downloads)" when they're in different folders
// (where each is, as v1 showed a found repo: project-names.ts whereIs), both when it takes both. A name only one
// project has stays as it is. v1 never needed this in its chips (it merged a repo's checkouts on several machines into
// one project), so it's the phone chips' rule, where every computer's projects show at once. Pure.
import { whereIs } from './project-names.ts';

export interface Labelled {
  readonly name: string;
  readonly path: string;
  /** the computer it's on */
  readonly peer?: string;
}

/** Each project's label, index for index: its name, with what tells it apart from the others of that name. */
export function projectLabels(projects: readonly Labelled[], computer: (peer: string) => string): string[] {
  const groups = new Map<string, Labelled[]>();
  for (const p of projects) groups.set(p.name, [...(groups.get(p.name) ?? []), p]);
  return projects.map((p) => {
    const same = groups.get(p.name) ?? [];
    if (same.length < 2) return p.name;
    const parts: string[] = [];
    if (new Set(same.map((q) => q.peer ?? '')).size > 1 && p.peer) parts.push(computer(p.peer));
    if (new Set(same.map((q) => whereIs(q.path))).size > 1 && p.path) parts.push(whereIs(p.path));
    return parts.length ? `${p.name} (${parts.join(' · ')})` : p.name;
  });
}
