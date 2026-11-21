// A recipe as Save as recipe… starts it (v1 saveAsRecipe: the task's title and first prompt, its mode and runner),
// and the `{field}`s its prompt asks for when it's run (v1 recipeParams). DOM-free.

export interface RecipeDraft {
  readonly name: string;
  readonly prompt: string;
  /** a project's folder; '' = whichever project it's run in */
  readonly project?: string;
  /** '' = the launcher's default */
  readonly mode?: string;
  readonly agent?: string;
  readonly model?: string;
  readonly effort?: string;
  /** the task it's saved from (an entity key): its first prompt is read when the editor opens */
  readonly session?: string;
  /** Edit (v1 recipeForm with `d.id`): the recipe's name as saved; Save replaces it (a new name renames it) */
  readonly editing?: string;
}

export function parseDraft(p: unknown): RecipeDraft {
  const o = (p ?? {}) as Record<string, unknown>;
  const s = (k: string): string => (typeof o[k] === 'string' ? (o[k] as string) : '');
  return {
    name: s('name'),
    prompt: s('prompt'),
    project: s('project'),
    mode: s('mode'),
    agent: s('agent'),
    model: s('model'),
    effort: s('effort'),
    ...(s('session') && { session: s('session') }),
    ...(s('editing') && { editing: s('editing') }),
  };
}

/** `{version}` and `{name}`, once each, in order; not `{ }` or `{1x}` */
export function recipeParams(prompt: string): string[] {
  const out: string[] = [];
  for (const m of prompt.matchAll(/\{([A-Za-z_][\w-]*)\}/g)) if (m[1] && !out.includes(m[1])) out.push(m[1]);
  return out;
}

/** A recipe's name as the server keeps it (a task's name: letters, digits, `-`, `_`, `.`, which `wisp run <name>`
 * types): "Ship the release" → "Ship-the-release". Save as recipe… fills it in from the task's title, so it must be one
 * the server takes (audit A tasks #7); what the user typed is kept as far as it can be. */
export const recipeName = (s: string): string =>
  s
    .trim()
    .replace(/[^\p{L}\p{N}._-]+/gu, '-')
    .replace(/^[-.]+|[-.]+$/g, '')
    .slice(0, 60);

/** The draft from a task: its title, its first prompt, its mode and runner (v1 saveAsRecipe). */
export function draftOf(task: { readonly title?: string; readonly mode?: string; readonly agent?: string; readonly model?: string; readonly effort?: string }, firstPrompt: string): RecipeDraft {
  return { name: recipeName(task.title ?? ''), prompt: firstPrompt, project: '', mode: task.mode ?? '', agent: task.agent ?? '', model: task.model ?? '', effort: task.effort ?? '' };
}

/** Every `{field}` a recipe's prompt and steps ask for when it's run, once each, in order (v1 recipeParams). */
export const fieldsOf = (r: { readonly prompt: string; readonly steps?: readonly string[] }): string[] => [...new Set([r.prompt, ...(r.steps ?? [])].flatMap(recipeParams))];

/** The run dialog's params: the recipe's name, and the launcher's project (its folder, '' for none). */
export const parseRun = (p: unknown): { name: string; project: string } => {
  const o = (p ?? {}) as Record<string, unknown>;
  return { name: typeof o['name'] === 'string' ? o['name'] : '', project: typeof o['project'] === 'string' ? o['project'] : '' };
};

/** What Create recipe / Save sends (`automations.task.set`, which replaces the whole task): the editor's fields, over
 * the recipe as saved when it's an edit, so what the editor doesn't show (steps, a schedule, a goal from a formula)
 * stays. A field emptied in the editor (Any project, Launcher default) is left out. */
export function recipeBody(d: RecipeDraft, base: Readonly<Record<string, unknown>> = {}): Record<string, unknown> {
  const out: Record<string, unknown> = { ...base, name: recipeName(d.name), prompt: d.prompt };
  for (const k of ['project', 'mode', 'agent', 'model', 'effort'] as const) {
    const v = d[k];
    if (v) out[k] = v;
    else if (k === 'project' || k === 'mode') delete out[k];
  }
  return out;
}

/** Where a recipe tied to no project runs when its dialog opens (audit B #11: Run from Settings said No folder): the
 *  project it was opened in, else the launcher's (the first project picked over the list, else the first project),
 *  else No folder (''). */
export function runProject(given: string, pick: string | undefined, projects: readonly { readonly id: string; readonly path: string }[]): string {
  if (given) return given;
  const ids = !pick || pick === 'all' ? [] : pick.split(',').map((x) => x.trim());
  return (ids.map((id) => projects.find((p) => p.id === id)).find((p) => p !== undefined) ?? projects[0])?.path ?? '';
}
