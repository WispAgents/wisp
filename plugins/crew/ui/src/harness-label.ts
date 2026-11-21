// A member's harness as the crew's screens say it (DOM-free; `harness.ts` follows the Auto switch for it). `auto` is
// what a member holds when it names no harness: while Auto is off (owner, 2026-11-17: Auto disabled until it's
// reworked; one switch, `runtime.auto`) its jobs run on New task's pick, so it reads "As New task", never "Auto"
// (audit A #22: "Auto · Full auto" on the crew page, "Every 30 min · auto" in the import review). A harness id reads
// as its name.
import { t } from './i18n.ts';
import './i18n/lazy.ts';

/** a harness as the shell lists it (`shell.harnesses()`) */
export interface Harness {
  readonly id: string;
  readonly name: string;
  readonly logo?: string | undefined;
}

export interface HarnessLabel {
  readonly name: string;
  readonly logo?: string | undefined;
}

/** `id`'s label among `harnesses`; `auto` (or none) by whether Auto is on. */
export function harnessLabel(id: string, harnesses: readonly Harness[], auto: boolean): HarnessLabel {
  if (!id || id === 'auto') return { name: t(auto ? 'wisp.crew.editor.auto' : 'wisp.crew.editor.asNewTask') };
  const h = harnesses.find((x) => x.id === id);
  return { name: h?.name ?? id, logo: h?.logo };
}
