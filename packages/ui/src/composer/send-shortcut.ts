// How Enter sends a prompt from New task or a task's reply box (Settings → Chat → Send shortcut).
// Shift+Enter always inserts a new line; Alt+Enter is reserved (hold bar's Wait until reset).

export type SendShortcut = 'enter' | 'mod-enter-multiline' | 'mod-enter';

export function shouldSendOnKey(
  e: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'metaKey' | 'ctrlKey' | 'altKey' | 'isComposing'>,
  value: string,
  shortcut: SendShortcut = 'enter',
): boolean {
  if (e.key !== 'Enter' || e.shiftKey || e.altKey || e.isComposing) return false;
  if (shortcut === 'mod-enter') return e.metaKey || e.ctrlKey;
  if (shortcut === 'mod-enter-multiline') return !value.includes('\n') || e.metaKey || e.ctrlKey;
  return true;
}
