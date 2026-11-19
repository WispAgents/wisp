// The Shell pane's size on attach (audit 2026-11-19), DOM-free: a shell started at another width (another window, the
// panel dragged since) says its old size in its hello; the pane, once it has measured its box, tells it its own, as v1's
// term-view.js fit the screen and resized the pty.

type Size = { readonly cols: number; readonly rows: number };

/** The size to send the shell after its hello (`hello` what it runs at, `pane` what the pane measured, undefined until
 *  it has): none when they agree. */
export function resizeOnHello(hello: Size, pane: Size | undefined): Size | undefined {
  if (!pane || (pane.cols === hello.cols && pane.rows === hello.rows)) return undefined;
  return { cols: pane.cols, rows: pane.rows };
}
