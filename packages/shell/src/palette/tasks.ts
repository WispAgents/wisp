// ⌘K's task pieces that need no component: a task's topics and past work (the palette's deep search). Plain so
// their tests needn't compile builtins.tsx and the views it registers.
import type { PaletteItem, ShellContext } from '../contract.ts';
import { searchTasks } from '../frame/commands.ts';
import { projectOf, sessionKey, shortAgo, statusOf } from '../frame/rows-view.ts';
import { projectsNow } from '../frame/project-switch.ts';
import { SESSION, type SessionRow } from '../rows.ts';

/** A task's topics (runtime's `tags`, set by wisp_tag), lower case: on the row's body, or the summary. */
export const tagsOf = (s: SessionRow): string[] => {
  const raw = (s.body as { tags?: unknown } | undefined)?.tags ?? (s.summary as { tags?: unknown }).tags;
  return Array.isArray(raw) ? raw.filter((t): t is string => typeof t === 'string' && t !== '').map((t) => t.toLowerCase()) : [];
};

/**
 * ⌘K past work (v1 F-task-20): three letters or more find words inside prompts and answers (`runtime.search`), each hit
 * a task with the words it matched; archived ones too. What the palette already lists by title stays where it is.
 */
export async function pastWork(ctx: ShellContext, q: string, signal: AbortSignal): Promise<PaletteItem[]> {
  const peer = ctx.client.plugin('shell').home;
  const hits = await ctx.client.commands.run(searchTasks, { peer, q, limit: 20 });
  if (signal.aborted) return [];
  const now = ctx.client.scope.timers.now();
  return hits.map((h) => {
    const id = h.id.includes(':') ? h.id : `${peer}:${h.id}`;
    return {
      id: `task:${id}`,
      group: 'shell.palette.group.tasks',
      label: h.title,
      sub: [h.snippet.replace(/\s+/g, ' ').trim(), shortAgo(h.updatedAt, now)].filter(Boolean).join(' · '),
      keywords: q,
      at: h.updatedAt,
      run: () => ctx.navigate({ t: 'openTask', id: id as never }),
    };
  });
}

/** Tasks: the title, then project · state · age, led by the project's dot (v1 taskSub), newest first. */
export function taskItems(ctx: ShellContext): PaletteItem[] {
  const g = 'shell.palette.group.tasks';
  const rows = ctx.client.entities.list<SessionRow>(SESSION)();
  // rows carry bare ids; the task page opens by entity key (`<peer>:<id>`): a bare id opened a blank page (owner, 2026-11-17)
  const keys = ctx.client.entities.keys(SESSION)();
  const projects = projectsNow(ctx);
  const now = ctx.client.scope.timers.now();
  const out: PaletteItem[] = [];
  for (const s of rows) {
    if (s.archived) continue;
    const sum = s.summary as Record<string, unknown>;
    const p = projectOf(String(sum['cwd'] ?? ''), projects);
    const name = p?.name ?? (typeof sum['project'] === 'string' ? sum['project'] : '');
    const color = p?.color ?? (typeof sum['color'] === 'string' && sum['color'] ? sum['color'] : 'slate');
    out.push({
      id: `task:${sessionKey(keys, s.id)}`,
      group: g,
      label: s.title,
      sub: [name, statusOf(s, ctx.t).label, shortAgo(s.updated_at, now)].filter(Boolean).join(' · '),
      color: color || 'slate',
      keywords: [sum['preview'], sum['live'], s.agent].filter((x) => typeof x === 'string' && x).join(' '),
      // what agents tagged it with (wisp_tag): a topic in ⌘K ("#auth", or the word) finds it (v1 F-task-21)
      ...(tagsOf(s).length > 0 && { tags: tagsOf(s) }),
      // live and waiting work floats up while typing (v1 rank)
      rank: (s.state === 'waiting' ? 3 : 0) + (s.unread ? 2 : 0) + (s.state === 'running' ? 1 : 0),
      at: s.updated_at,
      run: () => ctx.navigate({ t: 'openTask', id: sessionKey(keys, s.id) as never }),
    });
  }
  return out.sort((a, b) => (b.at ?? 0) - (a.at ?? 0));
}
