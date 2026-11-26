// Signal-only grouping for a finished turn's timeline (TurnTranscript):
// While a turn is running, every step streams live without folding. Once finished, high-signal steps
// (agent prose and thoughts, file edits, errors/failed calls) stay directly visible on the rail while contiguous runs
// of 3+ routine exploration steps (reads, searches, commands, generic tools) fold into a single
// quiet expandable cluster with compact read-file chips and coalesced duplicate labels. DOM-free.
import type { TimelineFileChip } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import '../i18n/lazy.ts';
import type { TranscriptStepRow } from './TurnTranscript.tsx';

/** Minimum consecutive routine steps before folding into a quiet cluster. */
export const MIN_CLUSTER = 3;

export interface IndexedStep {
  readonly step: TranscriptStepRow;
  readonly index: number;
}

export interface ClusterCounts {
  readonly reads: number;
  readonly edits?: number;
  readonly searches: number;
  readonly commands: number;
  readonly thoughts: number;
  readonly other: number;
}

export interface StepCluster {
  readonly kind: 'group';
  readonly key: string;
  readonly items: readonly IndexedStep[];
  readonly counts: ClusterCounts;
  readonly files: readonly TimelineFileChip[];
  readonly rest: readonly IndexedStep[];
}

export type TranscriptEntry =
  | { readonly kind: 'step'; readonly step: TranscriptStepRow; readonly index: number }
  | StepCluster;

const LEGACY_READ = /^using read(?: file)?$/i;
const SEARCH_RUN = /^(searching|listing|finding)\b/i;
const READING_PREFIX = /^reading\s+(.+)$/i;

/** High-signal steps stay surfaced directly on a finished turn's rail (when Transcript is on). */
export function isSignalStep(s: TranscriptStepRow): boolean {
  return s.kind === 'text' || s.kind === 'thinking' || s.kind === 'edit' || s.kind === 'agent' || s.kind === 'error' || s.bad === true;
}

/** In quiet Chat mode (Transcript off), agent messages and thoughts, delegations and errors stay surfaced; routine actions
 *  between messages fold into an expandable pill. A thought is what the agent says it is doing: it reads as a message. */
export function isQuietSignalStep(s: TranscriptStepRow): boolean {
  return s.kind === 'text' || s.kind === 'thinking' || s.kind === 'agent' || s.kind === 'error' || s.bad === true;
}

function classify(s: TranscriptStepRow): keyof ClusterCounts {
  const label = (s.label ?? '').trim();
  if (s.kind === 'edit') return 'edits';
  if (s.kind === 'read' || (s.kind === 'tool' && LEGACY_READ.test(label))) return 'reads';
  if (s.kind === 'search' || s.kind === 'web' || (s.kind === 'run' && SEARCH_RUN.test(label))) return 'searches';
  if (s.kind === 'run') return 'commands';
  if (s.kind === 'thinking') return 'thoughts';
  return 'other';
}

function readPathOf(s: TranscriptStepRow, here: (v: string) => string): string | undefined {
  if (s.kind !== 'read') return undefined;
  const raw = s.file?.trim() || READING_PREFIX.exec((s.label ?? '').trim())?.[1]?.trim();
  if (!raw) return undefined;
  const rel = here(raw).trim();
  return rel || undefined;
}

function basenameOf(path: string): string {
  const clean = path.replace(/[\\/]+$/, '');
  const slash = Math.max(clean.lastIndexOf('/'), clean.lastIndexOf('\\'));
  return slash >= 0 ? clean.slice(slash + 1) : clean;
}

/** Coalesce consecutive identical plain steps (e.g. legacy "Using read file" ×12) without hiding output/detail. */
function coalescePlain(items: readonly IndexedStep[]): IndexedStep[] {
  const out: IndexedStep[] = [];
  let i = 0;
  while (i < items.length) {
    const cur = items[i]!;
    const plain = cur.step.out === undefined && cur.step.title === undefined && cur.step.detail === undefined;
    if (!plain) {
      out.push(cur);
      i++;
      continue;
    }
    let j = i + 1;
    while (j < items.length) {
      const next = items[j]!;
      const nextPlain = next.step.out === undefined && next.step.title === undefined && next.step.detail === undefined;
      if (!nextPlain || next.step.kind !== cur.step.kind || (next.step.label ?? '') !== (cur.step.label ?? '')) break;
      j++;
    }
    const n = j - i;
    if (n > 1) {
      out.push({
        index: cur.index,
        step: { ...cur.step, label: `${cur.step.label ?? cur.step.kind} ×${n}` },
      });
    } else {
      out.push(cur);
    }
    i = j;
  }
  return out;
}

function buildCluster(items: readonly IndexedStep[], here: (s: string) => string): StepCluster {
  let reads = 0;
  let edits = 0;
  let searches = 0;
  let commands = 0;
  let thoughts = 0;
  let other = 0;
  const files: TimelineFileChip[] = [];
  const seenFiles = new Set<string>();
  const rawRest: IndexedStep[] = [];

  for (const it of items) {
    const bucket = classify(it.step);
    if (bucket === 'reads') reads++;
    else if (bucket === 'edits') edits++;
    else if (bucket === 'searches') searches++;
    else if (bucket === 'commands') commands++;
    else if (bucket === 'thoughts') thoughts++;
    else other++;

    const path = it.step.out === undefined ? readPathOf(it.step, here) : undefined;
    if (path) {
      if (!seenFiles.has(path)) {
        seenFiles.add(path);
        files.push({ name: basenameOf(path), title: path });
      }
    } else {
      rawRest.push(it);
    }
  }

  const first = items[0]!;
  return {
    kind: 'group',
    key: `group:${first.step.seq ?? first.index}`,
    items,
    counts: { reads, ...(edits > 0 && { edits }), searches, commands, thoughts, other },
    files,
    rest: coalescePlain(rawRest),
  };
}

/**
 * Partition a turn's steps into surfaced signal steps and collapsed routine clusters.
 * Running turns stay flat when Transcript is on so live output streams naturally; finished turns fold 3+ routine steps.
 * When `quiet` is true (Chat mode with Transcript off), every contiguous run of 1+ actions between agent messages
 * (both while running and when finished) folds into an expandable summary pill so the reader sees what is happening
 * between messages and can expand to inspect the exact actions taken.
 */
export function groupFinishedSteps(
  steps: readonly TranscriptStepRow[],
  running: boolean,
  here: (s: string) => string = (s) => s,
  quiet = false,
): TranscriptEntry[] {
  if (running && !quiet) {
    return steps.map((step, index) => ({ kind: 'step', step, index }));
  }
  const minCluster = quiet ? 1 : MIN_CLUSTER;
  const isSignal = quiet ? isQuietSignalStep : isSignalStep;
  const out: TranscriptEntry[] = [];
  let buf: IndexedStep[] = [];
  const flush = (): void => {
    if (buf.length >= minCluster) {
      out.push(buildCluster(buf, here));
    } else {
      for (const it of buf) out.push({ kind: 'step', step: it.step, index: it.index });
    }
    buf = [];
  };

  for (let i = 0; i < steps.length; i++) {
    const step = steps[i]!;
    if (isSignal(step)) {
      flush();
      out.push({ kind: 'step', step, index: i });
    } else {
      buf.push({ step, index: i });
    }
  }
  flush();
  return out;
}

/** Human-readable summary for a collapsed cluster (e.g. "Edited 2 files · Read 12 files · 3 searches"). */
export function clusterLabel(counts: ClusterCounts, total: number, t: ShellContext['t']): string {
  const parts: string[] = [];
  if ((counts.edits ?? 0) > 0) parts.push(t(counts.edits === 1 ? 'shell.task.group.edits.one' : 'shell.task.group.edits.other', { n: counts.edits ?? 0 }));
  if (counts.reads > 0) parts.push(t(counts.reads === 1 ? 'shell.task.group.reads.one' : 'shell.task.group.reads.other', { n: counts.reads }));
  if (counts.searches > 0) parts.push(t(counts.searches === 1 ? 'shell.task.group.searches.one' : 'shell.task.group.searches.other', { n: counts.searches }));
  if (counts.commands > 0) parts.push(t(counts.commands === 1 ? 'shell.task.group.commands.one' : 'shell.task.group.commands.other', { n: counts.commands }));
  if (counts.thoughts > 0) parts.push(t(counts.thoughts === 1 ? 'shell.task.group.thoughts.one' : 'shell.task.group.thoughts.other', { n: counts.thoughts }));
  if (counts.other > 0) parts.push(t(counts.other === 1 ? 'shell.task.group.actions.one' : 'shell.task.group.actions.other', { n: counts.other }));
  return parts.length > 0 ? parts.join(' · ') : t(total === 1 ? 'shell.task.steps.one' : 'shell.task.steps.other', { n: total });
}
