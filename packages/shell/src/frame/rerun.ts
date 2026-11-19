// Rerun… (v1 actions.js rerunDialog, F-task-43/44; ADR-0027: a rerun is the task's spec run again): a new task from
// this one's spec (`runtime.spec`: its first prompt, harness, model, thinking, mode, folder, extra folders, the steps
// after), with what you changed in the dialog, on the computer you pick. The old task is kept as it is. DOM-free.
// Its hints are lazy strings (only the Rerun dialog, a lazy part, imports this file).
import '../i18n/lazy.ts';

export interface RerunSpec {
  readonly prompt: string;
  readonly steps?: readonly string[];
  readonly agent?: string;
  readonly model?: string;
  readonly effort?: string;
  readonly mode?: string;
  readonly cwd?: string;
  readonly dirs?: readonly string[];
  /** it ran in a worktree of its own (New task's Worktree / In place): its rerun does too, as `wisp rerun` (audit B #2) */
  readonly worktree?: boolean;
  /** the kind of task (My Stuff's pane, a Cast plugin): a rerun is the same kind */
  readonly ext?: Readonly<Record<string, unknown>>;
}

export interface RerunEdits {
  readonly prompt: string;
  readonly mode: string;
  readonly agent: string;
  readonly model: string;
  readonly effort: string;
  /** the computer it runs on (a peer); the task's own when unchanged */
  readonly peer: string;
}

/** What the dialog starts with: as it ran (a harness picked from the toolbar's menu wins). A spec on Auto (or none)
 *  starts on the harness, model and thinking the task ran on (`ran`): Auto isn't offered any more, and the chip read
 *  "auto" (QA round 15 phone #4). */
export const startEdits = (s: RerunSpec, peer: string, agent?: string, ran?: { readonly agent: string; readonly model?: string | undefined; readonly effort?: string | undefined }): RerunEdits => {
  const routed = !s.agent || s.agent === 'auto';
  const base = routed && ran?.agent ? { agent: ran.agent, model: ran.model ?? '', effort: ran.effort ?? '' } : { agent: s.agent ?? '', model: s.model ?? '', effort: s.effort ?? '' };
  return {
    prompt: s.prompt,
    mode: s.mode || 'auto',
    agent: agent ?? base.agent,
    model: agent && agent !== base.agent ? '' : base.model,
    effort: agent && agent !== base.agent ? '' : base.effort,
    peer,
  };
};

/** `runtime.create`'s args (the shell's create command): a new task, its lineage pointing at the one rerun: by its bare
 *  id when it runs on the same computer, as `wisp rerun` writes it (the peer reads `rerunOf` as one of its own tasks;
 *  the entity key `laptop:s1` matched none, audit B #2), by its key on another computer. */
export function rerunArgs(s: RerunSpec, e: RerunEdits, o: { readonly of: string; readonly id: string; readonly at: number }) {
  return {
    peer: e.peer,
    id: o.id,
    prompt: e.prompt.trim(),
    agent: e.agent || 'auto',
    mode: e.mode,
    at: o.at,
    ...(e.model && { model: e.model }),
    ...(e.effort && { effort: e.effort }),
    ...(s.cwd && { cwd: s.cwd }),
    ...(s.dirs?.length && { dirs: [...s.dirs] }),
    ...(s.steps?.length && { steps: [...s.steps] }),
    ...(s.worktree !== undefined && { worktree: s.worktree }),
    ...(s.ext && Object.keys(s.ext).length > 0 && { ext: { ...s.ext } }),
    lineage: { rerunOf: o.of.startsWith(`${e.peer}:`) ? o.of.slice(e.peer.length + 1) : o.of },
  };
}

/** The hint's words for where the rerun runs (audit C #6): a task that ran in a worktree of its own reruns in a new one
 *  of its project (its spec's `cwd` is the main checkout, which the hint named); otherwise in its folder. */
export function rerunPlace(
  s: RerunSpec,
): { readonly key: 'shell.rerun.hint'; readonly vars: { readonly cwd: string } } | { readonly key: 'shell.rerun.hintWorktree'; readonly vars: { readonly project: string } } | { readonly key: 'shell.rerun.hintNoFolder'; readonly vars: Record<string, never> } {
  const cwd = s.cwd ?? '';
  if (!cwd) return { key: 'shell.rerun.hintNoFolder', vars: {} };
  if (s.worktree === true) return { key: 'shell.rerun.hintWorktree', vars: { project: cwd.split(/[\\/]/).filter(Boolean).pop() ?? cwd } };
  return { key: 'shell.rerun.hint', vars: { cwd } };
}
