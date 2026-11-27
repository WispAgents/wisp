// Find in task (mod+f, R4-3) as the page wires it (TaskView.tsx): every turn's rows as its transcript draws them, a
// hit brought into sight (its turn shown, the transcript on, its step opened); Copy turn as Markdown (R4-6); and each
// turn's open steps, one store per turn for its live and finished transcripts (step-open.ts).
import '../i18n/lazy.ts';
import { createSignal, type Accessor } from 'solid-js';
import type { EnvReveal, TaskScroll } from '@wisp/ui';
import { buildChatSteps } from './chat-steps.ts';
import type { TranscriptStepRow } from './TurnTranscript.tsx';
import { isStub, type turnOpener } from './turn-window.ts';
import { stepOpen, type StepOpen } from './step-open.ts';
import { findEnvOf, type FindHit, type FindTurn } from './task-find.ts';
import type { useShell } from '../context.tsx';
import type { useChatSettings } from './chat-settings.ts';
import { finalMarkdownOf, type EventRow, type TurnBody, type TurnRow } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What Find reads of the page: the turns, the ledger's open turn, Transcript, and the body its ref holds. */
export interface TaskFindCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly chat: ReturnType<typeof useChatSettings>;
  readonly turns: () => readonly TurnRow[];
  readonly stepsOf: (turnId: string) => readonly EventRow[];
  readonly fold: () => { readonly shown: readonly { readonly id: string }[] };
  readonly openTurn: Accessor<string | undefined>;
  readonly openLedgerTurn: (id: string | undefined) => void;
  readonly opener: ReturnType<typeof turnOpener>;
  readonly showStream: () => boolean;
  readonly setTaskTranscript: (open: boolean) => void;
  readonly bodyEl: () => HTMLElement | undefined;
  readonly scrollCtrl: () => TaskScroll | undefined;
}

export function useTaskFind(c: TaskFindCtx) {
  const { shell, t, chat, turns, stepsOf, fold, openTurn, openLedgerTurn, opener, showStream, setTaskTranscript, bodyEl, scrollCtrl } = c;
// each turn's open steps and clusters (step-open.ts), made once per turn for the page's life: its live and finished
// transcripts share it, and Find opens a hit's step through it before the turn is drawn
const opens = new Map<string, StepOpen>();
const openOf = (turnId: string): StepOpen => opens.get(turnId) ?? ((o) => (opens.set(turnId, o), o))(stepOpen());
// what Find reads of a drawn turn: its .env Reveal and the rows its transcript draws (an undrawn one's are built)
const findEnvs = new Map<string, EnvReveal>();
const drawnRows = new Map<string, () => TranscriptStepRow[]>();
const [finding, setFinding] = createSignal(false);
// Copy turn as Markdown (R4-6): its serializer loads on the click; it copies the steps as the transcript shows them
// (.env outputs masked unless revealed) and the answer's markdown source
const copyTurn = (turn: TurnRow, b: TurnBody, prompt: string, steps: readonly TranscriptStepRow[], env: EnvReveal, here: (s: string) => string): void => {
  const blocks = (b.presentation?.blocks ?? []) as readonly { readonly type: string; readonly text?: string }[];
  const answer = turn.answer ?? (b as { text?: string }).text ?? blocks.filter((x) => x.type === 'markdown' && x.text).map((x) => x.text).join('\n\n');
  void import('./turn-markdown.ts')
    .then((m) =>
      shell.platform.copy(
        m.turnMarkdown({ prompt, steps, answer, env: findEnvOf(env, openOf(turn.id).whole), here, words: { prompt: t('shell.md.prompt'), steps: t('shell.md.steps'), answer: t('shell.md.answer') } }),
      ),
    )
    .then((ok) => shell.toast.show(t(ok ? 'shell.task.copiedMarkdown' : 'shell.task.copyFailedMarkdown'), ok ? {} : { tone: 'error' }));
};
// Find in task (R4-3): every turn's rows as its transcript draws them; a stub's are the server's to search
const findTurns = (): FindTurn[] =>
  turns().map((x) => {
    const stub = isStub(x.body);
    const env = findEnvs.get(x.id);
    return { turnId: x.id, stub, steps: stub ? [] : (drawnRows.get(x.id)?.() ?? chatStepsOf(x, x.body, stepsOf(x.id))), ...(env && { env: findEnvOf(env, openOf(x.id).whole) }) };
  });
let findFrom: Element | null = null;
const openFind = (): void => {
  if (finding()) {
    // pressed again: back to its words
    document.querySelector<HTMLInputElement>('[data-part="task.find.input"]')?.select();
    return;
  }
  findFrom = document.activeElement;
  setFinding(true);
};
const closeFind = (): void => {
  setFinding(false);
  if (findFrom instanceof HTMLElement && findFrom.isConnected) findFrom.focus({ preventScroll: true });
  findFrom = null;
};
// a hit brought into sight: its turn shown (the ledger's, asked whole when a stub), the transcript on, its cluster and
// step opened, then scrolled to once drawn (a few frames at most: the ledger's turn mounts, the cluster opens)
const goFind = (hit: FindHit): Promise<Element | undefined> => {
  if (!showStream()) setTaskTranscript(true);
  if (!fold().shown.some((x) => x.id === hit.turnId) && openTurn() !== hit.turnId) openLedgerTurn(hit.turnId);
  else if (turns().some((x) => x.id === hit.turnId && isStub(x.body))) opener.open(hit.turnId);
  const id = hit.stepId;
  if (id === undefined) return Promise.resolve(undefined);
  openOf(hit.turnId).reveal(id);
  return new Promise((done) => {
    let tries = 0;
    const look = (): void => {
      const el = document.querySelector(`[data-kit="turn"][data-turn="${CSS.escape(hit.turnId)}"] [data-anchor="${CSS.escape(id)}"]`) ?? undefined;
      if (el || ++tries > 8) {
        el?.scrollIntoView({ block: 'center' });
        // the reader went to the hit: the view stays there as output streams (hold-view keeps an unasked move at the end)
        const sc = scrollCtrl();
        const at = bodyEl();
        if (el && sc && at) sc.place(at.scrollTop, false);
        return done(el);
      }
      requestAnimationFrame(look);
    };
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(look);
    else look();
  });
};
// a turn's inline steps (its transcript's rows) as Chat mode's Tool calls and Show thinking say; `turn` and `b` may be
// renderTurn's views of the row (through), read in its memo
// (its events given: a drawn turn reads its own memo of them, not every turn's)
const chatStepsOf = (turn: TurnRow, b: TurnBody, events: readonly EventRow[]): TranscriptStepRow[] => {
  const blocks = b.presentation?.blocks ?? [];
  return buildChatSteps(events, b.activity ?? [], {
    chatMode: chat.chatMode(),
    toolCalls: chat.toolCalls(),
    showThinking: chat.showThinking(),
    running: turn.status === 'running' || turn.status === 'starting',
    // the answer as the agent wrote it is what its last message says; the last markdown block alone differs
    // as soon as the answer carries a wisp block (it is cut around it), so raw answer/body.text and last markdown are tried
    finalText: [
      turn.answer ?? undefined,
      (b as { text?: string }).text,
      (b as { answer?: string }).answer,
      finalMarkdownOf(blocks as readonly { readonly type: string; readonly text?: string }[]),
    ],
    notes: (b as { presentation?: { notes?: string } }).presentation?.notes,
  });
};
  return { openOf, findEnvs, drawnRows, finding, copyTurn, findTurns, openFind, closeFind, goFind, chatStepsOf };
}
