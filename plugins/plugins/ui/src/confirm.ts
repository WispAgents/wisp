// Setup audit B13: Trust and Remove took one click. Trusting a plugin of yours asks first and names what it gets; Remove
// asks first, in v1's words (spell-ui.js "Remove the spell"). Untrusting needs no ask: it only takes away. Pure, for
// tests: the pane shows the dialog this returns.
import { rowAbout, type PluginRow } from './commands.ts';

export interface Confirm {
  readonly kind: 'trust' | 'remove';
  readonly id: string;
  readonly title: string;
  readonly detail: string;
  readonly ok: string;
}

/** The ask before `kind` on `p`, or none (untrusting: `next` false). */
export function confirmOf(kind: 'trust' | 'untrust' | 'remove', p: PluginRow, say: (k: string, v?: Readonly<Record<string, string>>) => string): Confirm | undefined {
  if (kind === 'untrust') return undefined;
  const name = rowAbout(p, say).title;
  const k = kind === 'trust' ? 'plugins.trustAsk' : 'plugins.removeAsk';
  return { kind, id: p.id, title: say(`${k}.title`, { name }), detail: say(`${k}.detail`, { name }), ok: say(`${k}.ok`) };
}
