import '../i18n/home.ts';
// What the reply box under a task says while the task asks you (v1 composer.js placeholder): one box answers, so a
// question reads "Answer <name>…" and a live approval "Or say what to do instead (denies it)…" (runtime's
// approval_decision: typed words that aren't a yes or no deny the call and go to the agent). Pure.

/** the i18n key for the reply box's placeholder */
/** `question`: the latest answer ends in a live `question` block (its options above the box, UX 2026-11-18 #1) */
export function replyPlaceholderKey(row: { readonly state?: string; readonly body?: Readonly<Record<string, unknown>> } | undefined, question = false): string {
  const ext = row?.body?.['ext'] as Record<string, unknown> | undefined;
  const ask = (ext?.['wisp.runtime'] as { ask?: unknown } | undefined)?.ask;
  if (!ask || typeof ask !== 'object') return question ? 'shell.task.replyOwn' : 'shell.task.reply';
  return (ask as { permission?: unknown }).permission ? 'shell.task.replyDenies' : 'shell.task.replyAnswer';
}

/** New task's placeholder (v1 launcher.js hint, QA r6 phone #11, r7 phone #3): a touch screen (the phone apps, a phone's
 *  browser) in a phone's words, nothing to drop and no shortcut; a pointer's v1's words while the caret is in it, else
 *  the shortcut that brings it back */
export const homePlaceholderKey = (o: { readonly touch: boolean; readonly focused: boolean }): string =>
  o.touch ? 'shell.home.placeholderPhone' : o.focused ? 'shell.home.placeholder' : 'shell.home.placeholderKey';
