// The open task's turns (TaskView.tsx): Find in task over them, the band (wisp.goal's), the earlier turns as a ledger
// (ADR-0068) with the latest whole after it, the /btw side questions and what side discussions proposed. Before the
// page's body is built, their skeleton.
import { Show, createMemo, lazy, type JSX } from 'solid-js';
import { entityKey, ownerOf } from '@wisp/client';
import { BtwList, ForKeyed, Ledger, Skeleton, Stack } from '@wisp/ui';
import { Slot } from '../slot.tsx';
import { dismiss } from './overlap-dismiss.ts';
import { btwDrop, btwOf, proposalApply, proposalDiscard, proposalsOf } from './commands.ts';
import type { TurnRow } from './task-rows.ts';
import type { TaskPage } from './task-page.ts';
import type { SessionRow } from '../rows.ts';

// a side discussion's proposed changes (audit user-side-edits-for-approval): their own chunk, loaded with the first one
const Proposals = lazy(() => import('./Proposals.tsx'));
// Find in task (mod+f, R4-3): its own chunk, loaded when it first opens
const TaskFind = lazy(() => import('./TaskFind.tsx'));

/** An earlier day in the ledger ("Mon 9 Nov"), in the app's locale as the kit's <Time> reads (content/time-format.ts). */
const DAY_NAME = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'short' });
const dayName = (day: number): string => DAY_NAME.format(day);
/** Its label on the ledger's scrubber, shorter ("Mon 9"), in the same locale. */
const DAY_SHORT = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric' });
const dayShort = (day: number): string => DAY_SHORT.format(day);
/** A task with more turns than this gets the ledger's search and filters (ADR-0068). */
const LEDGER_TOOLS = 10;

/** Where the turns will be, the frame before they're built: a prompt's and an answer's lines. */
function TurnsSkeleton(): JSX.Element {
  return (
    <Stack gap="4" part="split.placeholder" aria-hidden="true">
      <Skeleton shape="block" width="2/3" height="bar" />
      <Skeleton width="full" />
      <Skeleton width="3/4" />
      <Skeleton width="1/2" />
    </Stack>
  );
}

/** The turns, from the ledger to the latest; `renderTurn` draws one whole (TaskTurn.tsx). */
export function TaskTurns(props: { readonly p: TaskPage; readonly s: () => SessionRow; readonly renderTurn: (tr: () => TurnRow) => JSX.Element }): JSX.Element {
  const {
    bodyEl, shell, t, ledgerT, key, built, row, turns, run, now, clock, phoneBar, fold, openTurn, openLedgerTurn,
    finding, findTurns, closeFind, goFind, ledger, harnessView, side,
  } = props.p;
  const { input, when } = props.p.r;
  const s = props.s;
  const renderTurn = props.renderTurn;
  return (
    <Show
      when={built()}
      fallback={
        // the turns' place, the frame before they're built (design/feedback.md: the destination's shell at once)
        <TurnsSkeleton />
      }
    >
      {/* Find in task (mod+f, R4-3): held over the turns as they scroll */}
      <Show when={finding()}>
        <TaskFind shell={shell} task={key} turns={findTurns} go={goFind} scope={() => bodyEl()?.querySelector('[data-kit="turns"]') ?? bodyEl()} onClose={closeFind} />
      </Show>
      {/* the band over the turns (wisp.goal's): `shell.task:band`, the task's row as its session */}
      <Slot input="shell.task:band" layout="stack" id={key} session={() => s() as never} />
      {/* a phone has no details column: what's pinned to it goes over the turns instead */}
      <Show when={phoneBar()}>
        <Slot input="shell.task:details" layout="stack" id={key} session={() => s() as never} />
      </Show>
      {/* the earlier turns as a ledger (ADR-0068): outcome rows, day cards, notice folds, search, the scrubber;
          the latest turn whole after them, as it always was (design/ux.md) */}
      <Ledger
        turns={ledger()}
        total={turns().length}
        now={now()}
        open={openTurn()}
        onOpen={openLedgerTurn}
        body={(id) => {
          // the same row object keeps the same view; a stub that lands whole draws again
          const x = createMemo(() => turns().find((y) => y.id === id));
          return <Show when={x()}>{(r) => renderTurn(r)}</Show>;
        }}
        tail={<ForKeyed each={fold().shown}>{(turn) => renderTurn(turn)}</ForKeyed>}
        t={ledgerT}
        dayName={dayName}
        dayShort={dayShort}
        clock={clock}
        tools={turns().length > LEDGER_TOOLS}
      />
      {/* the /btw side questions asked on it (v1 btw.js), after its turns: answered on the side, the run carries on */}
      <BtwList
        items={btwOf(row()).map((b) => ({
          id: b.id,
          question: b.question,
          ...(b.answer !== undefined && { answer: b.answer }),
          ...(b.pending && { pending: true }),
          error: b.error,
          tip: t('shell.btw.tip', { at: clock(b.at) }),
        }))}
        waiting={t('shell.btw.waiting', { name: harnessView(s().agent).name })}
        dismiss={(pending) => t(pending ? 'shell.btw.cancel' : 'shell.btw.dismiss')}
        onDismiss={(btwId) => void run(btwDrop, { id: key, btwId }).catch(() => undefined)}
      />
      {/* what its side discussions proposed for the files it changed: a diff each, Apply or Discard here */}
      <Show when={proposalsOf(row()).length > 0}>
        <Proposals
          items={proposalsOf(row())}
          t={t}
          onApply={(proposal) => void run(proposalApply, { id: key, proposal }).catch(() => undefined)}
          onDiscard={(proposal) => void run(proposalDiscard, { id: key, proposal }).catch(() => undefined)}
          onOpenSide={(side) => shell.navigate({ t: 'openTask', id: entityKey(ownerOf(key), side) as never })}
        />
      </Show>
    </Show>
  );
}
