// What the prompt box and the reply box offer to pick (v1 launcher.js, composer.js): the mode, the
// harness, when it starts (who the lead may delegate to: team.ts). Pure: the entries and their words.

import type { IconName, MenuEntry } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import type { HarnessUiDef } from '../contract.ts';
import { harnessOf } from './harness.ts';

export type Mode = 'plan' | 'edits' | 'auto';
export const MODES: readonly Mode[] = ['plan', 'edits', 'auto'];
export const MODE_ICON: Readonly<Record<Mode, IconName>> = { plan: 'plan', edits: 'pen', auto: 'spark' };

export const modeMenu = (t: ShellContext['t'], on: Mode): MenuEntry[] =>
  MODES.map((m) => ({ value: m, label: t(`shell.mode.${m}`), hint: t(`shell.mode.${m}.hint`), icon: MODE_ICON[m], checked: m === on }));

/** The harnesses: Auto (wisp picks per task) and each one the harness plugins registered. */
export const agentName = (t: ShellContext['t'], all: readonly HarnessUiDef[], a: string): string => (a === 'auto' ? t('shell.home.auto') : harnessOf(all, a).name);
export const agentMenu = (t: ShellContext['t'], all: readonly HarnessUiDef[], on: string, withAuto = true): MenuEntry[] =>
  [...(withAuto ? ['auto'] : []), ...all.map((h) => h.id)].map((a) => ({ value: a, label: agentName(t, all, a), checked: a === on }));

/** When it starts, if not now (v1 later.js timeMenu: In 10 min, In 30 min, In 1 hour, In 3 hours, Tomorrow morning).
 *  Each is counted from the moment it's sent, so none is ever already gone (QA r1 #6: "This evening, 18:00" at 21:43). */
export const WHENS = ['10m', '30m', 'hour', '3h', 'morning'] as const;
export type When = (typeof WHENS)[number];

const clock = (at: number): string => new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

/** The menu: each choice with the time it means now under it (v1's time column). */
export const whenMenu = (t: ShellContext['t'], now: number = Date.now()): MenuEntry[] =>
  WHENS.map((w) => ({ value: w, label: t(`shell.when.${w}`), hint: clock(whenAt(w, now)), icon: 'clock' as const }));

/** The time a `when` names, from `now` (local time): minutes or hours ahead, or tomorrow 9:00. */
export function whenAt(w: When, now: number): number {
  if (w === '10m') return now + 600_000;
  if (w === '30m') return now + 1_800_000;
  if (w === 'hour') return now + 3_600_000;
  if (w === '3h') return now + 3 * 3_600_000;
  const d = new Date(now);
  d.setDate(d.getDate() + 1);
  d.setHours(9, 0, 0, 0);
  return d.getTime();
}
