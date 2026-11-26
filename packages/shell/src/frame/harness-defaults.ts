// A harness's own defaults for a new task (Settings → Agents, harness audit r2), kept apart from default-models.ts so
// only Settings' lazy chunk carries them, not the first screen (ADR-0011).
import { readLaunch, type LaunchPick } from './launch-pick.ts';

/** A harness's own defaults for a new task, as New task keeps them (`runtime.launch`): the model ('' its Default), the
 *  thinking level ('' its own) and the mode ('' New task's). Settings → Agents reads and writes these, so its rows and
 *  the composer show one answer. */
export interface HarnessDefaults {
  readonly model: string;
  readonly effort: string;
  readonly mode: string;
}

export const MODES_FOR_HARNESS = ['plan', 'edits', 'auto'] as const;

export function harnessDefaults(launch: unknown, agent: string): HarnessDefaults {
  const p = readLaunch(launch);
  return { model: p.models?.[agent] ?? '', effort: p.efforts?.[agent] ?? '', mode: p.modes?.[agent] ?? '' };
}

/** `launch` with `agent`'s defaults changed: the last pick (`agent`) and Run on (`where`) stay as they were, so setting Cursor's model in
 *  Settings doesn't move New task to Cursor. A model's `label` goes with it (the pill shows the name). */
export function withHarnessDefaults(
  launch: unknown,
  agent: string,
  change: { readonly model?: string; readonly label?: string; readonly effort?: string; readonly mode?: string },
): LaunchPick {
  const p = readLaunch(launch);
  const set = (m: Readonly<Record<string, string>> | undefined, v: string | undefined): Record<string, string> | undefined => {
    if (v === undefined) return m ? { ...m } : undefined;
    const out: Record<string, string> = { ...m };
    if (v === '') delete out[agent];
    else out[agent] = v;
    return out;
  };
  const models = set(p.models, change.model);
  const labels = change.model !== undefined ? set(p.labels, change.model === '' ? '' : (change.label ?? change.model)) : set(p.labels, undefined);
  const efforts = set(p.efforts, change.effort);
  const modes = set(p.modes, change.mode);
  return {
    ...(p.agent && { agent: p.agent }),
    ...(models && Object.keys(models).length > 0 && { models }),
    ...(labels && Object.keys(labels).length > 0 && { labels }),
    ...(efforts && Object.keys(efforts).length > 0 && { efforts }),
    ...(modes && Object.keys(modes).length > 0 && { modes }),
    ...(p.where && { where: p.where }),
  };
}
