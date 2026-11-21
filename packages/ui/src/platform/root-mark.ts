// A mark on <body> while a phone component is on screen (ADR-0065 build step 5): `data-fab` while Android's FAB is
// up, `data-nav-bar` while the navigation bar / tab bar is, so a toast sits above them (M3 snackbar: above the FAB and
// the navigation bar; the iOS capsule: above the tab bar) without `:has()` on the page (the platform sheets' rules all
// start `:root[data-platform=…] `, so the mark sits one level down). Counted: two marks of the same name hold it until
// both are gone. Only phone looks mount the components that call it.

const held = new Map<string, number>();

/** Sets `attr` on the page's body until the returned function runs (as many times as it's held). */
export function markRoot(attr: string, root: Element | undefined = typeof document === 'undefined' ? undefined : document.body): () => void {
  if (!root) return () => undefined;
  held.set(attr, (held.get(attr) ?? 0) + 1);
  root.setAttribute(attr, '');
  let done = false;
  return () => {
    if (done) return;
    done = true;
    const n = (held.get(attr) ?? 1) - 1;
    held.set(attr, n);
    if (n <= 0) root.removeAttribute(attr);
  };
}
