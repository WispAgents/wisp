// One turn of the open task as the page draws it (TaskView.tsx): its prompt card, its working panel while it runs, its
// transcript and answer, its trail and tools. Made once per turn id (B35): `turn` and `b` read the latest row, and
// what's derived from them is a function, read where it's shown.
import { Show, createMemo, getOwner, on, onCleanup, runWithOwner, untrack, type JSX, type Accessor } from 'solid-js';
import { entityKey, type EntityKey, type PeerId } from '@wisp/client';
import { Answer, Button, RateButtons, TurnView, WhyForm, WorkingPanel, type EnvReveal } from '@wisp/ui';
import { Slot } from '../slot.tsx';
import { changeFigures, duration, trailActs } from './rows-view.ts';
import { readComments } from './comment-text.ts';
import { turnLongTime } from './turn-clock.ts';
import { turnSteps } from './chat-steps.ts';
import { noOutputLine } from './no-output.ts';
import { ratingOf, fullOutput } from './commands.ts';
import { answerContext } from './answer-ctx.ts';
import { answerComments } from './answer-comments.ts';
import { blockedOf } from './answer-of.ts';
import { TurnTranscript, type TranscriptStepRow } from './TurnTranscript.tsx';
import { isStub, type turnOpener } from './turn-window.ts';
import { envReveal } from './env-reveal.ts';
import { promptHarness } from './prompt-harness.ts';
import type { useShell } from '../context.tsx';
import type { useChatSettings } from './chat-settings.ts';
import type { Runner } from './runner.ts';
import type { StepOpen } from './step-open.ts';
import type { SessionRow } from '../rows.ts';
import { ACT_ICON, through, type EventRow, type TurnBody, type TurnRow } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What a turn reads of its page: the task, its settings, and the page's handlers (TaskView.tsx). */
export interface TurnCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly key: EntityKey;
  readonly peer: PeerId;
  /** the task's row, as the page's <Show> gives it */
  readonly s: () => SessionRow;
  readonly row: () => SessionRow | undefined;
  readonly turns: () => readonly TurnRow[];
  readonly stepsOf: (turnId: string) => readonly EventRow[];
  readonly chatStepsOf: (turn: TurnRow, b: TurnBody, events: readonly EventRow[]) => TranscriptStepRow[];
  readonly drawnRows: Map<string, () => TranscriptStepRow[]>;
  readonly findEnvs: Map<string, EnvReveal>;
  readonly openOf: (turnId: string) => StepOpen;
  readonly opener: ReturnType<typeof turnOpener>;
  readonly chat: ReturnType<typeof useChatSettings>;
  readonly clients: { client(plugin: string): ReturnType<Shell['client']['plugin']> };
  readonly clock: (at: number) => string;
  readonly now: Accessor<number>;
  readonly tick: Accessor<number>;
  readonly showStream: () => boolean;
  readonly stepsShown: () => boolean;
  readonly toggleTranscript: () => void;
  readonly delegating: () => boolean;
  readonly harnessName: () => string;
  readonly harnessView: (id: string) => { name: string; logo?: string };
  readonly replyRun: () => Runner | undefined;
  readonly attachmentUrl: (id: string) => string | undefined;
  readonly rates: () => boolean;
  readonly rate: (turnId: string, rating: 'up' | 'down' | null, text?: string) => void;
  readonly whyFor: () => string | undefined;
  readonly setWhyFor: (turnId: string | undefined) => void;
  readonly notesOpen: () => boolean;
  readonly setNotesOpen: (open: boolean) => void;
  readonly restoring: () => boolean;
  readonly setRestoreAsk: (turnId: string) => void;
  readonly stopping: () => boolean;
  readonly askStop: () => void;
  readonly openFork: (turn?: string) => void;
  readonly copyTurn: (turn: TurnRow, b: TurnBody, prompt: string, steps: readonly TranscriptStepRow[], env: EnvReveal, here: (s: string) => string) => void;
  readonly side: (kind: 'changes' | 'shell') => void;
  /** an answer's chip, reply or option: sent as a follow-up */
  readonly reply: (v: string) => void;
  /** an answer's chip without ⌘: into the reply box, to read first */
  readonly draft: (v: string) => void;
}

/** One turn, made once for its id (B35). */
export function renderTaskTurn(c: TurnCtx, tr: () => TurnRow): JSX.Element {
  const { shell, t, key, peer, s, row, turns, stepsOf, chatStepsOf, drawnRows, findEnvs, openOf, opener, chat, clients, clock, now, tick } = c;
  const { showStream, stepsShown, toggleTranscript, delegating, harnessName, harnessView, replyRun, attachmentUrl, rates, rate, whyFor, setWhyFor } = c;
  const { notesOpen, setNotesOpen, restoring, setRestoreAsk, stopping, askStop, openFork, copyTurn, side, reply, draft } = c;
  const turn = through(tr);
  const b = through(() => tr().body);
  // the harness this turn ran on, not the reply box's pick (audit B #6)
  const turnHarness = (): { name: string; logo?: string } => harnessView(promptHarness(b as { agent?: unknown }, replyRun()?.agent, row()?.agent ?? ''));
  const hero = (): { title?: string; summary?: string } | undefined => b.presentation?.blocks?.find((x) => x.type === 'hero');
  // the steps come as the turn's events (the task's topic); a turn row that carries its own still reads
  // the turn's events, once per change of them: its steps, the working panel's count, ticker and quiet note all read
  // these (the note read them again, filtered and sorted from every event, on each streamed token: perf r1 #15)
  const turnEvents = createMemo(() => stepsOf(turn.id));
  // its tool calls, each failed one carrying its error (audit F3)
  const steps = createMemo(() => turnSteps(turnEvents(), b.activity ?? []));
  const cwd = String((s() as { body?: { cwd?: string } }).body?.cwd ?? s().summary['cwd'] ?? '');
  // the task's folder out of a step's words: one expression for the turn, not one per line per token
  const inFolder = cwd ? new RegExp(`(/private)?${cwd.replace(/^\/private/, '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/?`, 'g') : undefined;
  const here = (text: string): string => (inFolder ? text.replace(inFolder, '') : text);
  // the quiet note's view of the steps: a memo, not work done on every token (perf r1 #15)
  const quietSteps = createMemo(() => turnEvents().map((e) => ({ kind: e.kind ?? e.body?.kind, t: e.t, outAt: e.body?.outAt, call: e.body?.callId, output: e.body?.output })));
  const count = (k: string): number => steps().filter((a) => a.kind === k).length;
  const blocks = () => b.presentation?.blocks ?? [];
  const streamSteps = createMemo(() => chatStepsOf(turn, b, turnEvents()));
  drawnRows.set(turn.id, streamSteps);
  onCleanup(() => drawnRows.get(turn.id) === streamSteps && drawnRows.delete(turn.id));
  const stubbed = () => isStub(b);
  const changes = () => b.ext?.['wisp.runtime']?.changes ?? b.changes ?? [];
  const figures = () => changeFigures(changes());
  const tokens = () => (b.stats?.inputTokens ?? 0) + (b.stats?.outputTokens ?? 0);
  // a follow-up with comments on lines: the cards over the words typed with them (v1 sentComments)
  const sent = readComments(turn.prompt);
  // comments on this answer (v1 comments.js): words or a whole block, for the next follow-up
  const comment = answerComments({ shell, client: (pl) => clients.client(pl), task: key, turn: turn.id, of: () => (turn.id === turns().at(-1)?.id ? undefined : sent.rest) });
  // what a step printed of a .env file: masked, revealed per task (env-reveal.ts)
  const env = envReveal(shell, key, `${turn.id}:`);
  findEnvs.set(turn.id, env);
  // what of it is open: one store for its live and its finished transcripts (step-open.ts)
  const open = openOf(turn.id);
  // a clipped step's whole output, asked for on Show all (audit F8)
  const whole = (step: number): Promise<string> =>
    shell.client.commands.run(fullOutput, { id: key, turnId: turn.id, step }).then(
      (r) => r.text,
      (e: unknown) => {
        shell.toast.error(e instanceof Error ? e.message : String(e));
        throw e;
      },
    );
  // the working panel, made once for this turn and only while it runs, under the turn's own owner: made inside
  // the props' spread it was made again on every change of the task's row (its live line, ~3 a second while it
  // streams), so the panel and its Stop were taken down and put back each time (B31). Its words update in place.
  const live = createMemo(() => turn.status === 'running' && turn.id === turns().at(-1)?.id && (s().state === 'running' || s().state === 'waiting'));
  const owner = getOwner();
  let made: JSX.Element | undefined;
  // v1 transcript.js renderLiveTranscript: with Transcript on, a running turn shows its timeline so far above the
  // working panel, growing as steps come (owner, 2026-11-17: Transcript did nothing on a working task). Made once.
  let madeTx: JSX.Element | undefined;
  const turnAnswerCtx = Object.assign(
    answerContext({
      shell,
      key,
      turnId: turn.id,
      env,
      blocked: () => blockedOf(b),
      latest: () => turn.id === turns().at(-1)?.id,
      title: () => s().title,
      transcript: showStream,
      meta: () => [
        ...(b.endedAt && b.endedAt > turn.started_at ? [duration(b.endedAt - turn.started_at)] : []),
        ...(b.stats?.costUsd ? [`$${b.stats.costUsd.toFixed(2)}`] : []),
        ...(changes().length ? [t(changes().length === 1 ? 'shell.task.touched.one' : 'shell.task.touched.other', { n: changes().length })] : []),
      ],
      reply,
      draft,
      openChanges: () => side('changes'),
      changes,
      apps: () => ((b as { ext?: Record<string, unknown> }).ext?.['wisp.apps'] ?? {}) as Record<string, unknown>,
      tour: s().agent === 'wisp',
    }),
    { comment },
  );
  const liveTx = (): JSX.Element =>
    (madeTx ??= runWithOwner(owner, () => (
      <Show when={showStream() && (!chat.chatMode() || streamSteps().length > 0)}>
        <TurnTranscript steps={streamSteps()} changes={changes()} env={env} open={open} whole={whole} here={here} status={turn.status ?? ''} chat={chat.chatMode()} quiet={chat.chatMode() && !stepsShown()} answerCtx={turnAnswerCtx} t={t} />
      </Show>
    )));
  const working = (): JSX.Element =>
    (made ??= runWithOwner(owner, () => (
      <WorkingPanel
        part="task.working"
        // with Transcript or Chat mode on, the steps above are the log (owner, 2026-11-21): one live last line
        inline={showStream()}
        waiting={s().state === 'waiting'}
        status={
          (b as { by?: string }).by === 'compact'
            ? t('shell.work.compacting')
            : t(s().state === 'waiting' ? 'shell.work.waiting' : 'shell.work.working', { name: harnessName() })
        }
        // a first turn counts from the send: its worktree and setup are part of the wait
        elapsed={duration(Math.max(0, tick() - (turn.idx === 0 ? Math.min(turn.started_at, s().created_at || turn.started_at) : turn.started_at)))}
        count={t('shell.work.actions', { count: steps().filter((a) => a.kind !== 'text').length })}
        note={noOutputLine(
          {
            running: turn.status === 'running',
            waiting: s().state === 'waiting',
            startedAt: turn.started_at,
            steps: quietSteps(),
            // a delegate of this task still works: its row says so
            delegating: delegating(),
          },
          now(),
          t,
        )}
        ticker={steps()
          .filter((a) => a.kind !== 'text')
          .slice(-5)
          .reverse()
          .map((a) => ({ icon: ACT_ICON[a.kind] ?? 'tool', text: here(a.label ?? a.kind) }))}
        actions={
          <>
            <Show when={Number(s().summary.queued ?? 0) > 0}>
              <span data-kit="working-queued">{t('shell.work.queued', { count: Number(s().summary.queued ?? 0) })}</span>
            </Show>
            <span data-kit="working-grow" />
            <Button size="sm" variant="ghost" tone="bad" icon="stop" part="task.stop" loading={stopping()} onClick={askStop}>
              {stopping() ? t('shell.stop.stopping') : t('shell.stop.ok')}
            </Button>
          </>
        }
      />
    )));
  // the answer, made once while it shows: a 👍 patched the turn's ext, which made its body a new object, and a
  // spread that read blocks() built the answer, its timeline and its bar again (owner, 2026-11-17)
  const answerShown = createMemo(() => !stubbed() && blocks().length > 0);
  const answerView = createMemo(on(answerShown, (shown) => (shown ? untrack(() => (
            <>
              {/* v1 transcript.js renderTranscript & Chat mode: inline stream before the answer */}
              <Show when={showStream() && (!chat.chatMode() || streamSteps().length > 0)}>
                <TurnTranscript
                  steps={streamSteps()}
                  changes={changes()}
                  env={env}
                  open={open}
                  whole={whole}
                  here={here}
                  status={turn.status ?? ''}
                  chat={chat.chatMode()}
                  quiet={chat.chatMode() && !stepsShown()}
                  answerCtx={turnAnswerCtx}
                  t={t}
                />
              </Show>
              <Answer blocks={blocks()} ctx={turnAnswerCtx} />
            </>
  )) : undefined)));
  return (
    <>
      {/* over each turn, what plugins mark before it (shell-task's harness-switch divider): `shell.turn:head` */}
      <Slot input="shell.turn:head" layout="contents" id={entityKey(peer, turn.id)} session={() => s() as never} />
      <TurnView
        id={turn.id}
        latest={turn.id === turns().at(-1)?.id}
        chat={chat.chatMode()}
        {...(rates() && { rate: <RateButtons rating={ratingOf(b)} labels={{ up: t('shell.rate.up'), down: t('shell.rate.down') }} onRate={(next) => rate(turn.id, next)} /> })}
        {...(whyFor() === turn.id && {
          trailBody: (
            <WhyForm
              labels={{ placeholder: t('shell.rate.why'), note: t('shell.rate.whyNote'), skip: t('shell.rate.skip'), send: t('shell.rate.send') }}
              onSend={(text) => rate(turn.id, 'down', text)}
              onSkip={() => {
                setWhyFor(undefined);
                shell.toast.success(t('shell.rate.thanks'));
              }}
              onClose={() => setWhyFor(undefined)}
            />
          ),
        })}
        prompt={sent.rest}
        {...((b.attachments ?? []).length > 0 && {
          attachments: (b.attachments ?? []).map((a) => ({
            name: a.name ?? a.id ?? '',
            ...((u) => (u ? { src: u } : {}))(attachmentUrl(a.id ?? '')),
            ...(a.mime?.startsWith('image/') && { image: true }),
          })),
        })}
        askNotes={sent.notes.map((c) => ({
          n: c.n,
          on: c.kind === 'text' && c.quote ? `“${c.quote.replace(/\s+/g, ' ')}”` : c.label,
          removed: c.removed,
          removedLabel: t('shell.task.commentRemoved'),
          say: c.note,
        }))}
        harness={turnHarness().name}
        {...(turnHarness().logo && { logo: turnHarness().logo })}
        time={clock(turn.started_at)}
        timeTitle={turnLongTime(turn.started_at, chat.timeFormat())}
        {...(b.checkpoint?.commit && {
          sha: { short: b.checkpoint.commit.slice(0, 7), title: t('shell.task.checkpoint', { branch: b.checkpoint.branch ?? '', sha: b.checkpoint.commit }) },
        })}
        {...((h) => (h?.title ? { hero: { title: h.title, summary: h.summary ?? '' } } : {}))(hero())}
        {...(stubbed() && {
          answer: (
            <Button size="sm" variant="ghost" icon="chevron" part="turn.open" loading={opener.opening(turn.id)} onClick={() => opener.open(turn.id)}>
              {t('shell.task.openTurn')}
            </Button>
          ),
        })}
        {...(answerView() && { answer: answerView() })}
        {...(turn.status === 'running' && { live: s().summary.live || t('shell.task.starting') })}
        {...(turn.status === 'starting' && { live: t('shell.task.waitingToStart') })}
        {...(turn.status === 'held' && { untold: true })}
        {...(live() && { working: [liveTx(), working()] })}
        actsTitle={t('shell.task.activity')}
        acts={trailActs(stepsShown() ? 0 : steps().length, { read: count('read'), edit: count('edit'), run: count('run') }, t)}
        stats={[
          // v1 renderTrail: each only when there is something to say; a turn refused before it ran (no stats,
          // no steps) took no time worth a figure ("1ms": B9)
          ...(b.endedAt && b.endedAt > turn.started_at && (b.stats || steps().length) ? [{ text: duration(b.endedAt - turn.started_at) }] : []),
          ...(b.stats?.costUsd ? [{ text: `$${b.stats.costUsd.toFixed(2)}` }] : []),
        ]}
        {...(tokens() > 0 && turn.status !== 'running' && { receipt: { text: t('shell.task.tokens', { n: tokens() }), title: t('shell.task.tokensTitle', { n: tokens() }) } })}
        // after the tokens, on their line: the receipt's cache and plan share (shell-task, ADR-0095; QA r1 #19)
        receiptMore={<Slot input="shell.turn:trail" layout="contents" id={entityKey(peer, turn.id)} session={() => s() as never} />}
        {...(changes().length > 0 && {
          files: { label: t(figures().files === 1 ? 'shell.task.files.one' : 'shell.task.files.other', { count: figures().files }), plus: `+${figures().plus}`, minus: `−${figures().minus}`, act: { onClick: () => side('changes') } },
        })}
        activity={{
          open: false,
          onToggle: toggleTranscript,
          items: steps().map((a) => ({
            icon: ACT_ICON[a.kind] ?? 'tool',
            text: here(a.label ?? a.kind),
            ...(a.detail && { detail: here(a.detail) }),
            ...(a.out && { out: here(a.out) }),
            ...(a.bad && { bad: true }),
          })),
        }}
        {...(notesOpen() && { notes: String((b as { presentation?: { notes?: string } }).presentation?.notes ?? t('shell.task.noNotes')) })}
        tools={[
          { id: 'notes', icon: 'text', label: t('shell.task.notes'), on: notesOpen(), act: { onClick: () => setNotesOpen(!notesOpen()) } },
          {
            id: 'rollback',
            icon: 'replay',
            label: t('shell.task.rollback'),
            act: {
              pending: restoring(),
              onClick: () => setRestoreAsk(turn.id),
            },
          },
          { id: 'fork', icon: 'fork', label: t('shell.task.fork'), part: 'turn.fork', act: { onClick: () => openFork(turn.id) } },
          // on the trail on a computer, in the answer's menu on a phone (R4-6)
          { id: 'copy-md', icon: 'copy', label: t('shell.task.copyMarkdown'), part: 'turn.copy-md', act: { onClick: () => copyTurn(turn, b, sent.rest, streamSteps(), env, here) } },
        ]}
        // a phone's answer menu (its ⋯ and a long-press, UX 2026-11-18 phone #8, QA r9 #2): Copy, Comment… on the
        // block held, the tools above, and Good / Bad result (their buttons are on the trail already)
        phoneMenu={{
          copy: { label: t('shell.answer.copy'), onCopy: (text) => void shell.platform.copy(text).then((ok) => ok || shell.toast.show(t('shell.task.copyFailed'), { tone: 'error' })) },
          comment: t('shell.answer.comment'),
          more: rates()
            ? (['up', 'down'] as const).map((r) => ({
                id: `rate:${r}`,
                icon: r === 'up' ? ('thumb-up' as const) : ('thumb-down' as const),
                label: t(r === 'up' ? 'shell.rate.good' : 'shell.rate.bad'),
                on: ratingOf(b) === r,
                act: { onClick: () => rate(turn.id, ratingOf(b) === r ? null : r) },
              }))
            : [],
        }}
      />
      {/* under each turn, what plugins say of it (shell-task's cards: why it stopped, held, what changed, the receipt): `shell.turn:foot`, the turn's key as its id */}
      <Slot input="shell.turn:foot" layout="stack" id={entityKey(peer, turn.id)} session={() => s() as never} />
    </>
  );
}
