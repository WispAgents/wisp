// How Continue on… opens (ADR-0125): its own module, as only the lazy menus and cards call it.
import { CONTINUE_VIEW, type ContinueParams } from './view.ts';

/** Opens Continue on… for a task (every way in goes through here: the task menu, the palette, its key, `/continue` in
 *  the reply box, and a refused or limited turn's offer). */
export function openContinue(client: { navigate(i: unknown): void }, p: ContinueParams): void {
  client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: CONTINUE_VIEW, params: p } });
}
