// Reverting in Changes, as v1 (features/changes.js revertHunk / revertFile / reverted): a hunk goes at once; a whole
// file asks first, in the danger style, saying what goes ("… is new in this task, so reverting deletes it"); either way
// the toast says Reverted with Undo, which puts back the text the file had just before (read first, so it's exact) if
// the file is still as the revert left it (its hash). Audit F tasks #2: v2 threw the work away with no question and no
// Undo, and an untracked new file was lost for good. Pure: the words and the steps Undo takes.
import '../i18n/lazy.ts';

/** What Undo puts back: a file's text from before (null: it wasn't there, so Undo removes it), if its hash is still `hash`. */
export interface UndoStep {
  readonly path: string;
  readonly text: string | null;
  readonly hash?: string | null;
}

type T = (key: string, vars?: Record<string, string | number>) => string;

/** v1's question before a whole file goes: its title, and what goes for its status (A new, D deleted, else changed). */
export function revertAsk(path: string, status: string, base: string, t: T): { readonly title: string; readonly body: string } {
  const name = path.split('/').pop() ?? path;
  const what = status === 'A' ? 'new' : status === 'D' ? 'deleted' : 'changed';
  return { title: t('shell-task.changes.revertAsk.title', { path, base: base || 'the base' }), body: t(`shell-task.changes.revertAsk.${what}`, { name }) };
}

/** Undo's steps for a revert of `path` (a rename's `from`: its old name, which the revert brought back, goes again).
 *  `before`: the file's text read just before the revert; null when it wasn't there (a deleted file), undefined when
 *  it couldn't be read as text (binary, too big): then there's no Undo (null). */
export function undoSteps(path: string, from: string | undefined, before: string | null | undefined, hash: string | null | undefined): readonly UndoStep[] | null {
  if (before === undefined) return null;
  return [{ path, text: before, hash: hash ?? null }, ...(from ? [{ path: from, text: null }] : [])];
}

/** The base a revert goes back to, as the user knows it: `origin/main`, else the branch it came from (`main`) when git's
 *  base is a bare commit (no remote), else that commit, short. */
export function baseName(...names: readonly (string | undefined)[]): string {
  const sha = (x: string): boolean => /^[0-9a-f]{40}$/.test(x);
  const named = names.find((x): x is string => !!x && x !== 'HEAD' && !sha(x));
  return named ?? names.find((x): x is string => !!x && sha(x))?.slice(0, 7) ?? '';
}
