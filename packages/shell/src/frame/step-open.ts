// Which steps and folded clusters of one turn are open (R4-5). One store per turn, made where the turn is made
// (TaskView's renderTurn, beside its .env Reveal), and handed to both of its transcripts: the live one while it runs
// and the answer's once it ends, so what was opened stays open as the turn ends. Keyed by the step, not its place:
// a row put before it (Show thinking, a coalesced run, a step hidden) moves it, and its open state goes with it.
import { createSignal } from 'solid-js';
import type { TranscriptStepRow } from './TurnTranscript.tsx';

/** A step's id: its event (`step:<seq>`, the id Reveal uses too), else its place in the rows (`index:<n>`). */
export const stepIdOf = (a: Pick<TranscriptStepRow, 'seq'>, index: number): string => (a.seq !== undefined ? `step:${a.seq}` : `index:${index}`);

export interface StepOpen {
  isOpen(id: string): boolean;
  toggle(id: string): void;
  /** opens each (a cluster and its step), closing none */
  openAll(ids: readonly string[]): void;
  /** opens the step and asks its transcript to open the cluster that holds it (Find's hit) */
  reveal(id: string): void;
  /** the step a transcript is asked to bring into sight, once: it opens the cluster holding it and takes it */
  wanted(): string | undefined;
  took(id: string): void;
  /** a clipped step's whole output, once its Show all fetched it (`runtime.fullOutput`), by the event that carried it:
   *  the turn's, so its finished transcript keeps what its live one fetched, and Find and Copy read what is shown */
  whole(seq: number): string | undefined;
  setWhole(seq: number, text: string): void;
}

export function stepOpen(): StepOpen {
  const [open, setOpen] = createSignal<ReadonlySet<string>>(new Set());
  const [want, setWant] = createSignal<string | undefined>(undefined);
  const [wholes, setWholes] = createSignal<ReadonlyMap<number, string>>(new Map());
  const openAll = (ids: readonly string[]): void => {
    if (ids.every((id) => open().has(id))) return;
    setOpen((prev) => new Set([...prev, ...ids]));
  };
  return {
    isOpen: (id) => open().has(id),
    toggle: (id) =>
      setOpen((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      }),
    openAll,
    reveal: (id) => {
      openAll([id]);
      setWant(id);
    },
    wanted: want,
    took: (id) => {
      if (want() === id) setWant(undefined);
    },
    whole: (seq) => wholes().get(seq),
    setWhole: (seq, text) => setWholes((prev) => new Map(prev).set(seq, text)),
  };
}
