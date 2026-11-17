// The crew editor's open intent, read when a link or Back opens it. Kept apart from editor.ts so the plugin's
// registration (index.ts) doesn't pull the editor's model into the first page (initial JS budget, ADR-0011):
// `again` passes through as given, and CrewEditor makes a Draft of it.
import type { Draft } from './editor.ts';

/** the editor's params: a member to edit (`id`), or a task to make a member's first job (`task`, its entity key:
 *  v1 Add to crew…) */
export const parseEditor = (p: unknown): { id?: string; task?: string; again?: Partial<Draft> } => {
  const o = p as { id?: unknown; task?: unknown; again?: unknown } | null;
  // `again`: the form as you left it when the peer refused the add (v1 `again()`)
  const again = o?.again && typeof o.again === 'object' && typeof (o.again as Draft).name === 'string' ? (o.again as Partial<Draft>) : undefined;
  return { ...(typeof o?.id === 'string' && o.id && { id: o.id }), ...(typeof o?.task === 'string' && o.task.includes(':') && { task: o.task }), ...(again && { again }) };
};
