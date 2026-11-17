// New task's pick, kept (owner #36, v1 launcher.js: S.config.defaults.agent / models / efforts, PATCH /config on each
// pick): the harness last picked, and each harness's model (and its name for the pill) and thinking level, in the
// server setting `runtime.launch`, so going to another page and back, or a reload, finds them as they were. The
// runtime runs a task that names no harness on the same pick (runtime.rs `last_pick`).
//
// Auto's one switch (owner #37, 2026-11-17, docs/v2-plan.md §3): the setting `runtime.auto`, default off. Off, nothing
// offers or starts on Auto (New task, its harness picker, Settings → Auto, crew); on, all of it is back.

import { entityKey, type PluginClient } from '@wisp/client';

export const AUTO_SETTING = 'runtime.auto';
export const LAUNCH_SETTING = 'runtime.launch';

export interface LaunchPick {
  readonly agent?: string;
  readonly models?: Readonly<Record<string, string>>;
  readonly labels?: Readonly<Record<string, string>>;
  readonly efforts?: Readonly<Record<string, string>>;
}

/** Auto is offered only while its switch is on (a bool `true`, nothing else). */
export const autoEnabled = (v: unknown): boolean => v === true;

/** The switch as `client`'s wisp has it, read where it's called (reactive inside a derivation): for a plugin that
 *  shows Auto (Settings → Auto, a harness menu). */
export const autoOn = (client: Pick<PluginClient, 'home' | 'entities'>): boolean =>
  autoEnabled(client.entities.row<{ value?: unknown }>('setting', entityKey(client.home, AUTO_SETTING))()?.value);

const strings = (v: unknown): Record<string, string> | undefined => {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return undefined;
  const out = Object.fromEntries(Object.entries(v as Record<string, unknown>).filter((e): e is [string, string] => typeof e[1] === 'string'));
  return Object.keys(out).length ? out : undefined;
};

/** The setting's value as a pick (anything malformed dropped). */
export function readLaunch(v: unknown): LaunchPick {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return {};
  const o = v as Record<string, unknown>;
  const models = strings(o['models']);
  const labels = strings(o['labels']);
  const efforts = strings(o['efforts']);
  return {
    ...(typeof o['agent'] === 'string' && o['agent'] && { agent: o['agent'] }),
    ...(models && { models }),
    ...(labels && { labels }),
    ...(efforts && { efforts }),
  };
}

/** A pick made: this harness, and its model, the model's name and level (each harness keeps its own). */
export function withPick(prev: LaunchPick, agent: string, model: string, label: string, effort: string): LaunchPick {
  return {
    agent,
    models: { ...prev.models, [agent]: model },
    labels: { ...prev.labels, [agent]: label },
    efforts: { ...prev.efforts, [agent]: effort },
  };
}

/** What New task starts on: the last pick while it's there (Auto only while it's on), else Auto when on, else the
 *  first ready harness (`ready`: in their order). */
export function startRunner(pick: LaunchPick, ready: readonly string[], auto: boolean): { agent: string; model: string; modelLabel: string; effort: string } {
  const kept = pick.agent && (pick.agent === 'auto' ? auto : ready.includes(pick.agent)) ? pick.agent : undefined;
  const agent = kept ?? (auto ? 'auto' : (ready[0] ?? ''));
  return { agent, model: pick.models?.[agent] ?? '', modelLabel: pick.labels?.[agent] ?? '', effort: pick.efforts?.[agent] ?? '' };
}
