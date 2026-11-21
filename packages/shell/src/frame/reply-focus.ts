// When an open task's reply box may take the caret (audit H tasks, F #1 partly fixed): it takes it once the task's body
// is built, a frame later. On a slow page (WebKit, a loaded machine) that came about 3 s after a deep link
// (`?side=shell:…`) had given the Shell the keys, so `echo z` landed in the reply box. Focus inside the side panel (the
// Shell, a file, a tab) stays; a page opened on the Shell leaves the caret to the Shell, which takes it when it draws
// (ShellPane: and gives it to the reply box when it can't start). Anything else (the list's row or search a key opened
// the task from) as before: the reply box takes it.
export function mayTakeCaret(active: Element | null | undefined, doc: { readonly body: Element | null }, side = 'none'): boolean {
  if (side === 'shell') return false;
  if (!active || active === doc.body) return true;
  return !active.closest("[data-part~='side.panel'], [data-pane='side']");
}
