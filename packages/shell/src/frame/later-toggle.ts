// The head's ⋯ Save for later ↔ Unsave, said as the row menu and the hover button say it (shell-list actions.ts
// toggleForLater): "Saved for later" / "Unsaved", each with Undo; Undo of Unsave puts back the time it was saved at
// (audit B #7: the head saved quietly, with no Undo). Pure: TaskView sends and toasts.
import { isSaved } from './rows-view.ts';
import '../i18n/lazy.ts';

type Later = { readonly forLater: boolean; readonly at?: number };

export function laterToggle(was: unknown): { readonly next: Later; readonly toast: string; readonly undo: Later } {
  if (!isSaved(was)) return { next: { forLater: true }, toast: 'shell.toast.savedForLater', undo: { forLater: false } };
  const at = (was as { at?: unknown }).at;
  return { next: { forLater: false }, toast: 'shell.toast.unsaved', undo: { forLater: true, ...(typeof at === 'number' && { at }) } };
}
