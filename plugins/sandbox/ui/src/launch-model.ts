// A launch's rows and steps, pure (the list and the page read them; tests too).
import { t } from './i18n.ts';

/** A step as the kit's Steps draws it. */
export interface StepItem {
  readonly title: string;
  readonly status: 'done' | 'current' | 'pending' | 'failed';
}

export type LaunchStep = 'make' | 'wake' | 'copy' | 'start';

/** A `/sandbox` launch, until its task started there (then the task's own row). */
export interface LaunchRow {
  readonly id: string;
  readonly title: string;
  /** preparing (`step` says where), signin (Claude's sign-in for sandboxes first), failed (`error`), started */
  readonly state: 'preparing' | 'signin' | 'failed' | 'started';
  readonly step: LaunchStep | '';
  readonly error?: string | null;
  readonly at: number;
  readonly agent: string;
  readonly peer?: string | null;
  readonly task?: string | null;
}

/** The rows the list shows: a started one is its task's own row by then. */
export const shown = (rows: readonly LaunchRow[]): LaunchRow[] => rows.filter((r) => r.state !== 'started').sort((a, b) => b.at - a.at);

const STEPS: readonly LaunchStep[] = ['make', 'wake', 'copy', 'start'];

/** Each step done, being done or still to come, from where the launch is. */
export function stepsOf(row: LaunchRow | undefined): StepItem[] {
  const at = row ? STEPS.indexOf(row.step as LaunchStep) : 0;
  const failed = row?.state === 'failed';
  return STEPS.map((s, i) => ({
    title: t(`sandbox.step.${s}`),
    status: i < at ? 'done' : i === at ? (failed ? 'failed' : 'current') : 'pending',
  }));
}
