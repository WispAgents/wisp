// New task's `+` → Formula or task file… (owner-list #61, ADR-0111): pick a formula (`wisp.yml`) or a task file
// (`x.task.yml`, whose `formula:` may name files and modules beside it); the peer reads it as one task's overlay,
// nothing applied (`resources.overlay`, the same answer `wisp run --formula` and `wisp_launch` get), and checks every
// name its task gives. A task file fills the prompt and its fields; the formula goes with the task as a chip until
// cleared. Off the first screen (ADR-0105): loaded when the row is pressed.
import { defineCommand } from '@wisp/client';
import type { ShellContext } from '../contract.ts';
import type { CreateArgs } from './commands.ts';
import '../i18n/lazy.ts';

/** What `resources.overlay` answers (wisp_resources_api::OverlayOut), the parts New task reads. */
export interface OverlayOut {
  readonly overlay?: unknown;
  readonly task?: Readonly<Record<string, unknown>>;
  readonly problems?: readonly string[];
  /** how it was read (a picked task file's `./x` read from the project folder: its own isn't known) */
  readonly notes?: readonly string[];
}

/** The picked file: its name (the chip's) and what it adds to `runtime.create`'s args. */
export interface FormulaPick {
  readonly name: string;
  readonly args: Partial<Pick<CreateArgs, (typeof TASK_FIELDS)[number] | 'formula'>>;
}

export const overlayRead = defineCommand<{ peer: string; text: string; name: string; cwd?: string }, OverlayOut>({
  id: 'resources.overlay',
  owner: (a) => a.peer,
  wire: ({ peer: _peer, ...a }) => a,
  optimistic: 'none',
  pending: 'the formula',
  quiet: true,
});

/** A task file's fields New task sends as they are (its prompt fills the box; its folder is New task's Where). */
const TASK_FIELDS = ['agent', 'model', 'effort', 'mode', 'goal', 'steps', 'connectors', 'crew', 'notes', 'worktree', 'title'] as const;

/** What a picked file adds to the task (the formula as its overlay, a task file's own fields), and a task file's prompt. */
export function pickArgs(out: OverlayOut): { readonly args: FormulaPick['args']; readonly prompt?: string } {
  const args: Record<string, unknown> = {};
  const o = out.overlay;
  if (o !== null && typeof o === 'object' && Object.keys(o).length > 0) args['formula'] = o;
  const task = out.task ?? {};
  for (const k of TASK_FIELDS) if (task[k] !== undefined && task[k] !== '') args[k] = task[k];
  const prompt = typeof task['prompt'] === 'string' && task['prompt'].trim() !== '' ? task['prompt'] : undefined;
  // the peer's answer is `runtime.create`'s own args: they go as they came
  const typed = args as FormulaPick['args'];
  return prompt === undefined ? { args: typed } : { args: typed, prompt };
}

/** The `+` row's press: the file dialog, the peer's reading, then the chip (`set`) and a task file's prompt (`fill`). A
 *  name that names nothing (or a file that doesn't read) is a toast, saying what there is; nothing is attached. */
export async function pickFormula(
  shell: Pick<ShellContext, 'platform' | 'client' | 'toast'>,
  peer: string,
  cwd: string | undefined,
  set: (p: FormulaPick) => void,
  fill: (prompt: string) => void,
): Promise<void> {
  const [file] = await shell.platform.pickFiles({ accept: '.yml,.yaml,.json,.toml' });
  if (!file) return;
  try {
    const out = await shell.client.commands.run(overlayRead, { peer, text: await file.text(), name: file.name, ...(cwd && { cwd }) });
    const problems = out.problems ?? [];
    if (problems.length > 0) return void shell.toast.error(problems.join('\n'));
    const { args, prompt } = pickArgs(out);
    set({ name: file.name, args });
    for (const n of out.notes ?? []) shell.toast.show(n, { tone: 'info' });
    if (prompt !== undefined) fill(prompt);
  } catch (err) {
    shell.toast.error(err instanceof Error ? err.message : String(err));
  }
}
