// UX 2026-11-18 desktop #5: the side panel's Changes tab carries its file count, and the strip shows Changes only when
// there is a diff. The Changes pane writes what git last said here as it reads it, so the tab and the pane agree and
// the panel reads git itself only while Changes isn't open.
import { createSignal } from 'solid-js';

export interface ChangeStat {
  readonly files: number;
}

const [stats, setStats] = createSignal<ReadonlyMap<string, ChangeStat>>(new Map());

/** The task's last known changes (`task` its entity key); undefined before anything read them. */
export const changeStat = (task: string): ChangeStat | undefined => stats().get(task);

export function setChangeStat(task: string, s: ChangeStat): void {
  if (stats().get(task)?.files === s.files) return;
  setStats((m) => new Map(m).set(task, s));
}
