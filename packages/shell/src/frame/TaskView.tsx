// The open task as v1 shows it (web/js/features/split.js, feed.js openBody, transcript.js): its head,
// its turns (the ask, the answer's brief, the trail), the details column with the task's actions,
// the overlap note and the reply box. It renders at once from the list's row (design/feedback.md);
// the turns fill in as the task's own topic arrives. Every control answers at once (design/feedback.md):
// Mark done, a follow-up, Rerun and Fork show their result before the peer answers; Ship it and a
// restore show their control working; the panes and menus open in the same frame.

import { Show, createEffect, createMemo, createResource, createSignal, getOwner, lazy, on, onCleanup, runWithOwner, untrack, type JSX } from 'solid-js';
import { defineCommand, entityKey, newRowId, ownerOf, uuidv7, type EntityKey, type Json } from '@wisp/client';
import { changeWhere as changeWhereOf, whereOf } from './change-where.ts';
import {
  Answer,
  BtwList,
  ForKeyed,
  Button,
  Dialog,
  Inline,
  Ledger,
  OverlapNote,
  RateButtons,
  ReplyBox,
  Skeleton,
  Stack,
  TaskBody,
  TaskDetails,
  TaskHead,
  TurnView,
  WhyForm,
  WorkingPanel,
  useSignal,
  type IconName,
  type LedgerString,
  type MenuEntry,
  type HeadAction,
  type ReplyCap,
  type Act,
  type LauncherTool,
  type TaskPlaceChip,
} from '@wisp/ui';
import { useShell } from '../context.tsx';
import { blockStrings } from '../block-strings.ts';
import { newestSide, prBadge } from './side-newest.ts';
import { usePlanRing } from './plan-ring.ts';
import type { SendHold } from '../contract.ts';
import { aheadLabel, startsAt, waitable, whenTip } from './hold.ts';
import { modelWords } from './run-words.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { Slot, useClients, usePluginClient } from '../slot.tsx';
import { TaskHeadSlot } from './slots.tsx';
import { afterNextPaint, buildsAtOnce } from './task-paint.ts';
import { NO_EVENTS, eventsByTurn } from './turn-events.ts';
import { sameEntries } from './same-entries.ts';
import { useProjects } from './projects.ts';
import { taskPlaces } from './project-pick.ts';
import { phoneTaskMenu } from './phone-menu.ts';
import { changeFigures, duration, foldedFrom, longTime, nextNeedsYou, overlapLine, overlapsOf, projectOfRow, statusOf, trailActs } from './rows-view.ts';
import { coldHint, compactItem } from './cold.ts';
import { replyPlaceholderKey } from './reply-hint.ts';
import { factoryFoot, factoryReply, runFactory } from './run-factory.ts';
import { goalGate, goalWire, type DeclaredGoal } from './goal-gate.ts';
import { useGoalKit } from './goal-kit.ts';
import { isFeedback } from './feedback.ts';
import { readComments } from './comment-text.ts';
import { useNow } from '../clock.ts';
import { turnClock } from './turn-clock.ts';
import { factoryStopQuestion, stopQuestion, type StopQuestion } from './stop-question.ts';
import { noOutputLine } from './no-output.ts';
import { completionOf, suggestedFollowUps } from './follow-ghost.ts';
import { promptKeys, remember, sentStore } from './prompt-keys.ts';
import { draftValue, replyDraft } from './drafts.ts';
import { keep, placeOf, placesStore, scrollFor, type Place } from './place.ts';
import { dismiss, dismissedStore, shownOverlaps, type Dismissed } from './overlap-dismiss.ts';
import { menuEntryOf, taskMenu, type TaskMenuEntry } from '../task-menu.ts';
import {
  rateTurn,
  ratingOf,
  btw,
  btwDrop,
  btwOf,
  btwQuestion,
  sideAsk,
  sideStart,
  setSetting,
  archive,
  stopFactoryRun,
  stopRun,
  coldOf,
  compactNow,
  followUp,
  answerSetup,
  setupCardOf,
  forLater,
  isSaved,
  markRead,
  pin,
  setLean,
  type Attachment,
  remove,
  rename,
  restore,
  setGoal,
  setMode,
  setState,
} from './commands.ts';
import { answerContext } from './answer-ctx.ts';
import { answerComments } from './answer-comments.ts';
import { blockedOf, uploadUrl } from './answer-of.ts';
import { TASK_KEY, takePendingKey, type TaskKey } from './task-keys.ts';
import { usePromptMenu } from './prompt-menu.ts';
import { useAttachments } from './attach.ts';
import { attachAct } from './capture.ts';
import { useMic } from './dictate.ts';
import type { PickOptions } from '../platform/types.ts';
import { harnessOf } from './harness.ts';
import { placeIn, taskSteps, type TaskSteps } from './task-step.ts';
import { RunnerPicker } from './RunnerPicker.tsx';
import { TurnTranscript } from './TurnTranscript.tsx';
import { isStub, turnOpener } from './turn-window.ts';
import { NOTICE_KINDS, ledgerTurn } from './ledger-turns.ts';
import type { LedgerTurn } from '@wisp/ui/pure';
import { envReveal } from './env-reveal.ts';
import { runPick, runWords, taskRunner, useDefaultWords, type Runner } from './runner.ts';
import { rowUsage, taskUsage, type TurnStatsLike } from './task-usage.ts';
import { TELEMETRY_TOPIC, feedbackOn } from './feedback-on.ts';
import { MODE_ICON, agentMenu, modeMenu, whenAt, whenMenu, type Mode, type When } from './pickers.ts';
import { teamFromSetting, teamSize, teamToSend, type Delegate } from './team.ts';
import { plusTool } from './composer-plus.ts';

// Delegate's panel: its own chunk, loaded when it first opens (ADR-0011)
const DelegatePanel = lazy(() => import('./DelegatePanel.tsx'));
import { capOf } from './cap.ts';
import { shipsFromWorktree } from './ship-gate.ts';
import { unreadLeave } from './unread-leave.ts';
import { laterToggle } from './later-toggle.ts';
import { promptHarness } from './prompt-harness.ts';
import '../i18n/lazy.ts';
import { openedByStep } from './stepped.ts';
import { mayTakeCaret } from './reply-focus.ts';


interface TurnBody {
  readonly prompt?: string;
  /** the files sent with the prompt (runtime's Attachment: an upload's id, name and mime) */
  readonly attachments?: readonly { readonly id?: string; readonly name?: string; readonly mime?: string }[];
  readonly startedAt?: number;
  readonly endedAt?: number | null;
  readonly checkpoint?: { readonly commit?: string; readonly branch?: string };
  readonly presentation?: { readonly blocks?: readonly { readonly type: string; readonly title?: string; readonly summary?: string }[] };
  readonly stats?: { readonly costUsd?: number; readonly durationMs?: number; readonly inputTokens?: number; readonly outputTokens?: number };
  readonly activity?: readonly { readonly kind: string; readonly label?: string }[];
  readonly raw?: string;
  readonly presentationNotes?: string;
  readonly changes?: readonly { readonly file: string; readonly edits?: readonly unknown[] }[];
  /** where runtime keeps a turn's changes today */
  readonly ext?: { readonly 'wisp.runtime'?: { readonly changes?: readonly { readonly file: string; readonly edits?: readonly unknown[] }[] } };
}

/** A step of a turn, as the task's topic sends it (runtime's event log: `kind`, and a tool's `tool` and `label`). */
/** A tool's output carried by its own `tool_output` event, as a step's `out` and the event that carried it. */
// the phone apps' task head (ADR-0065 step 13): the platform's app bar with the task's menu, in the phone heads' chunk
// (the list's head loaded it already)
const PhoneTaskBar = lazy(() => import('./PhoneHead.tsx').then((m) => ({ default: m.PhoneTaskBar })));

const outputOf = (o: { readonly text: string; readonly seq: number } | undefined): { out?: string; seq?: number } => (o?.text ? { out: o.text, seq: o.seq } : {});

interface EventRow {
  readonly seq: number;
  readonly turn_id: string;
  /** when it happened (ms) */
  readonly t?: number;
  readonly kind?: string;
  readonly body?: {
    readonly kind?: string;
    readonly tool?: string;
    readonly label?: string;
    /** its whole title when the label was cut (v1 #780) */
    readonly title?: string;
    readonly text?: string;
    readonly detail?: string;
    readonly file?: string;
    readonly output?: string;
    readonly callId?: string;
    /** when a call's result came in */
    readonly outAt?: number;
    readonly name?: string;
  };
}

interface TurnRow {
  readonly id: string;
  readonly session_id: string;
  readonly idx: number;
  readonly prompt: string;
  readonly started_at: number;
  readonly status?: string;
  readonly body: TurnBody;
}

const ACT_ICON: Readonly<Record<string, IconName>> = { read: 'read', edit: 'pen', run: 'terminal', error: 'warn', text: 'text', tool: 'tool' };

/** A turn's clock time, in the app's locale (v1 turnTime). */
const clock = (at: number): string => turnClock(at);

/** An earlier day in the ledger ("Mon 9 Nov"), in the app's locale as the kit's <Time> reads (content/time-format.ts). */
const DAY_NAME = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'short' });
const dayName = (day: number): string => DAY_NAME.format(day);
/** Its label on the ledger's scrubber, shorter ("Mon 9"), in the same locale. */
const DAY_SHORT = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric' });
const dayShort = (day: number): string => DAY_SHORT.format(day);

/** A task with more turns than this gets the ledger's search and filters (ADR-0068). */
const LEDGER_TOOLS = 10;

/** The ledger's words: `shell.ledger.<key>`, a count of one in its own words (`.one`). */
const ONE: Readonly<Partial<Record<LedgerString, string>>> = { turns: 'n', matches: 'n', count: 'd', sum: 'files' };

/** An object that reads `of()`'s latest value on every property read: in a JSX attribute or a memo that read is
 * tracked, so a turn's view follows its row without being made again (B35). */
function through<T extends object>(of: () => T | undefined): T {
  return new Proxy({} as T, {
    get: (_t, k) => (of() as Record<PropertyKey, unknown> | undefined)?.[k],
    has: (_t, k) => k in ((of() ?? {}) as object),
    ownKeys: () => Reflect.ownKeys((of() ?? {}) as object),
    getOwnPropertyDescriptor: (_t, k) => {
      const v = of() as Record<PropertyKey, unknown> | undefined;
      return v && k in v ? { value: v[k], enumerable: true, configurable: true } : undefined;
    },
  });
}

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

/** git's read of the task's folder (wisp.git's `git.changes`): its files' stats are the head's Changes figures */
const changesNow = defineCommand<{ key: string; path: string; base: string }, { readonly stats?: readonly { readonly add: number; readonly del: number }[] } | null>({
  id: 'git.changes',
  owner: (a) => a.key as EntityKey,
  wire: (a) => ({ path: a.path, base: a.base, stageState: false }),
  optimistic: 'none',
  quiet: true,
  pending: 'what changed',
});

export default function TaskView(props: { readonly id: string; readonly onBack?: () => void }): JSX.Element {
  const shell = useShell();
  // the answer blocks' copy, before the first answer draws (block-strings.ts)
  blockStrings(shell.client);
  const t = shell.t;
  const ledgerT = (k: LedgerString, vars?: Readonly<Record<string, string | number>>): string =>
    t(ONE[k] && vars?.[ONE[k] ?? ''] === 1 ? `shell.ledger.${k}.one` : `shell.ledger.${k}`, vars as Record<string, string | number> | undefined);
  const key = props.id as EntityKey;
  const scope = shell.client.scope.child(`task:${key}`);
  // the click's (or the key's, or the tap's) frame draws the page's head and the turns' skeleton; the body is built in the
  // frame after (task-paint.ts), on a phone and in split alike. On a phone, Back's frame shows the list first: the task's
  // scope (its topic's unwatch is a request sent) goes after it
  const later = !buildsAtOnce(typeof requestAnimationFrame === 'function');
  onCleanup(() => (later ? afterNextPaint(() => scope.dispose()) : scope.dispose()));
  // the topic names the session by its own id, the part after `<peer>:`
  const bare = key.slice(key.indexOf(':') + 1);
  // the task's topic, and whether its wisp takes feedback (a turn's 👍 👎 only then, v1 feedbackOn)
  const watch = (): void => (shell.client.peers.watch(ownerOf(key), `session:${bare}`, scope), shell.client.peers.watch(ownerOf(key), TELEMETRY_TOPIC, scope, { passive: true }));
  const rates = useSignal(feedbackOn(shell.client, ownerOf(key)));
  // on a phone the tap's frame draws the page with its head; the body (turns, reply box, details) comes the frame
  // after, and so does the task's own topic, asked for with it (task-paint.ts, F-phone-8: each request sent is a
  // fetch in the tap)
  const [built, setBuilt] = createSignal(!later);
  if (built()) watch();
  else
    onCleanup(
      afterNextPaint(() => {
        watch();
        setBuilt(true);
      }),
    );
  const row = useSignal(shell.client.entities.row<SessionRow>(SESSION, key));
  const allTurns = useSignal(shell.client.entities.list<TurnRow>('turn'));
  const allEvents = useSignal(shell.client.entities.list<EventRow>('event'));
  // a turn's steps, oldest first: its tool calls and its messages. Grouped once per change of the events, a turn whose
  // events didn't change keeps its array, so only that turn recomputes (turn-events.ts, perf r2 #5)
  const byTurn = createMemo<Map<string, readonly EventRow[]>>((prev) => eventsByTurn(allEvents(), prev));
  const stepsOf = (turnId: string): readonly EventRow[] => byTurn().get(turnId) ?? NO_EVENTS;
  const sessions = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  // a turn names its session by the bare id the server gave it, or by the key (an optimistic one)
  const realTurns = createMemo(() =>
    allTurns()
      .filter((x) => x.session_id === key || x.session_id === bare)
      .sort((a, b) => a.idx - b.idx),
  );
  // a task whose first prompt hasn't started yet (just sent, made held, waiting for its worktree) shows that prompt as
  // its first turn, starting (v1: the prompt card at once), never as a queued follow-up (B16, B17): its first queued item
  const allQueue = useSignal(shell.client.entities.list<{ session_id?: string; pos?: number; body?: { id?: string; text?: string; at?: number; attachments?: TurnBody['attachments'] } }>('queue_item'));
  const firstQueued = createMemo((): { id: string; text: string; at: number; attachments?: TurnBody['attachments'] } | undefined => {
    const own = (row()?.body as { queue?: readonly { id: string; text: string; at?: number; attachments?: TurnBody['attachments'] }[] } | undefined)?.queue;
    const q = own ?? allQueue().filter((r) => r.session_id === bare && r.body).toSorted((a, b) => (a.pos ?? 0) - (b.pos ?? 0)).map((r) => ({ id: r.body?.id ?? '', text: r.body?.text ?? '', at: r.body?.at ?? 0, ...(r.body?.attachments && { attachments: r.body.attachments }) }));
    const f = q[0];
    return f && f.text ? { id: f.id, text: f.text, at: f.at ?? 0, ...(f.attachments?.length && { attachments: f.attachments }) } : undefined;
  });
  const turns = createMemo((): TurnRow[] => {
    const real = realTurns();
    const f = real.length === 0 ? firstQueued() : undefined;
    if (!f) return real;
    // running: the working panel says so (Starting… while the run gets going); held: it waits to start
    return [{ id: `first:${f.id}`, session_id: bare, idx: 0, prompt: f.text, started_at: f.at || (row()?.created_at ?? Date.now()), status: row()?.state === 'running' ? 'running' : 'starting', body: (f.attachments ? { attachments: f.attachments } : {}) as TurnBody }];
  });
  // the reply being written is a draft (kind 5): kept across reloads and updates, per task (v1 F-task-32)
  // each task keeps its place (v1 split.js keepPlace/backToPlace, F-task-17): written when it closes or the page hides,
  // read once on opening it, after its turns are there
  let bodyEl: HTMLElement | undefined;
  const places = placesStore(shell.client);
  const turnTops = (el: HTMLElement): { id: string; top: number }[] => {
    const at = el.getBoundingClientRect().top - el.scrollTop;
    return [...el.querySelectorAll<HTMLElement>('[data-turn]')].map((t) => ({ id: t.dataset['turn'] ?? '', top: t.getBoundingClientRect().top - at }));
  };
  // kept by the page's own Back just before it leaves: read while the layout is still the frame's (free), where the
  // close's read came after the stage had flipped and forced a style and layout of the half-changed page (F-phone-8).
  // Back also lets go of the focus first (it goes to the page, as when the button is removed): removing the focused
  // button with the page forced another style recalc of the list coming back
  let kept = false;
  // the place is read as the reader scrolls (once a frame, while the layout is the frame's own), so keeping it on the
  // way out reads nothing: read in the close it forced a layout of the half-changed page, 7.5 ms of a Mark done
  // (the flicker gate's profile). Never scrolled since it opened: the kept place stands as it was.
  let placeSeen: { place: Place | undefined } | undefined;
  let reading = 0;
  const notePlace = (el: HTMLElement): void => {
    if (reading) return;
    reading = requestAnimationFrame(() => {
      reading = 0;
      // a page already hidden (a phone's Back through history hides the main pane first) has no place to read: every
      // turn would measure 0 and the kept place would become the top
      if (!el.isConnected || el.checkVisibility?.() === false || !el.querySelector('[data-turn]')) return;
      placeSeen = { place: placeOf(el, turnTops(el)) };
    });
  };
  onCleanup(() => cancelAnimationFrame(reading));
  const keepPlace = (): void => {
    if (kept || !placeSeen) return;
    const p = placeSeen.place;
    places.set((cur) => keep(cur as unknown as Record<string, Place>, key, p) as unknown as Json);
  };
  onCleanup(keepPlace);
  const back = props.onBack;
  const leave =
    back &&
    ((): void => {
      keepPlace();
      kept = true;
      const focused = document.activeElement;
      if (focused instanceof HTMLElement && bodyEl?.parentElement?.contains(focused)) focused.blur();
      back();
    });
  const onHide = (): void => {
    if (document.visibilityState === 'hidden') keepPlace();
  };
  addEventListener('pagehide', keepPlace);
  document.addEventListener('visibilitychange', onHide);
  onCleanup(() => {
    removeEventListener('pagehide', keepPlace);
    document.removeEventListener('visibilitychange', onHide);
  });
  let placed = false;
  createEffect(() => {
    if (placed || !built() || turns().length === 0) return;
    placed = true;
    // after a reload the kept places are still loading from disk
    void places.ready.then(() => {
      const p = (places.value() as unknown as Readonly<Record<string, Place>> | undefined)?.[key];
      requestAnimationFrame(() => {
        const el = bodyEl;
        if (el && p) el.scrollTop = scrollFor(p, el, turnTops(el));
        // placed away from the end (its kept place, or its top): what shows stays as turns land (F-page-44)
        if (el) keepShown(el);
      });
    });
  });
  const [rKey, rField] = replyDraft(key);
  const reply$ = useSignal(shell.client.drafts.get<string>(rKey, rField));
  const text = (): string => reply$() ?? '';
  const setText = (v: string): void => shell.client.drafts.set(rKey, rField, draftValue(v));
  let input: HTMLTextAreaElement | undefined;
  // opened by j or k: the keys stay on the list (stepped.ts, v1)
  const stepped = openedByStep();
  // in split the reply box takes the caret, once the body is there and in a frame of its own: focusing in the render forced
  // a style and layout of the half-built page in the click's frame (46 ms of 399 at 4×, perf r1 #6). Unless you clicked
  // meanwhile: a click on the head (its ⋯, the title) keeps its own focus. Focus an overlay puts back as it closes (the
  // palette's Open) is not a click, so the caret still comes here
  let clicked = false;
  let unwatch = (): void => undefined;
  if (typeof document !== 'undefined') {
    const down = (): void => {
      clicked = true;
    };
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('click', down, true);
    unwatch = () => {
      document.removeEventListener('pointerdown', down, true);
      document.removeEventListener('click', down, true);
    };
    onCleanup(unwatch);
  }
  const focusReply = (): void => {
    unwatch();
    // never taken from the side panel or a field the keys already went to (reply-focus.ts, audit H tasks F #1)
    if (!clicked && mayTakeCaret(document.activeElement, document, untrack(() => shell.nav().side.kind))) input?.focus();
  };
  createEffect(
    on(built, (ready) => {
      if (!ready) return;
      if (stepped || untrack(() => shell.nav().layout) !== 'split') return unwatch();
      if (typeof requestAnimationFrame !== 'function') return focusReply(); // nothing paints: no frame to wait for
      const id = requestAnimationFrame(focusReply);
      onCleanup(() => cancelAnimationFrame(id));
    }),
  );
  const titleOf = (id: string): string => sessions().find((s) => s.id === id)?.title ?? '';
  // the overlap note's × (v1 #787): closed for this task until what it says changes; kept in this browser as v1 did
  const closedNotes = dismissedStore(shell.client);
  const closed = useSignal(closedNotes.value);
  // until the kept ones load (a reload), a closed note would flash up and go: it waits for them
  const [closedReady, setClosedReady] = createSignal(false);
  void closedNotes.ready.catch(() => undefined).finally(() => setClosedReady(true)); // a failed load shows them all
  const closeNote = (id: string, all: ReturnType<typeof overlapsOf>): void => {
    closedNotes.set((cur) => dismiss(cur as unknown as Dismissed, id, all));
  };
  const peer = ownerOf(key);
  const run = shell.client.commands.run;
  // the cold-cache line and Compact now (v1 cold-cache.js): asked again whenever the task settles or changes
  const now = useNow(shell.client);
  // the running turn's elapsed time counts each second (v1); the rest of the page reads `now` (every 30 s) (B21)
  const tick = useNow(shell.client, 1_000);
  // Stop asks first (v1 #781): what else stops (its running subagents, a subagent's report), and that queued follow-ups stay
  const [stopAsk, setStopAsk] = createSignal<(StopQuestion & { readonly factory?: true }) | undefined>();
  const [stopping, setStopping] = createSignal(false);
  const allRows = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  const askStop = (): void => {
    const r = row();
    if (!r) return;
    const live = allRows().filter((x) => x.parent_id === key && x.state === 'running').length;
    setStopAsk(stopQuestion({ sub: !!r.parent_id, name: harnessView(r.agent).name, live, queued: Number(r.summary.queued ?? 0) }, t));
  };
  // Restore to before a turn asks first (orch #17): it undoes every change made in the folder since that turn
  const [restoreAsk, setRestoreAsk] = createSignal<string | undefined>();
  const doRestore = (): void => {
    const turnId = restoreAsk();
    setRestoreAsk(undefined);
    if (!turnId) return;
    setRestoring(true);
    void run(restore, { id: key, turn: turnId })
      .then(
        // the head's Changes figure shows it; one that restored nothing says so. What the folder held just before
        // is kept: Undo puts it back (audit A tasks #1: a restore lost work with no way back)
        (r) => {
          if (r?.restored === false) return void shell.toast.show(t('shell.restore.nothing'), { tone: 'error' });
          const undo = r?.undo;
          if (!undo) return;
          shell.toast.show(t('shell.restore.done'), {
            action: { label: t('shell.restore.undo'), onClick: () => void run(restore, { id: key, turn: turnId, undo }).then(() => refetchChanges(), () => undefined) },
          });
        },
        () => undefined,
      )
      .finally(() => {
        setRestoring(false);
        // what git shows now in the folder, read again: the figure isn't left at what it was before (orch #17)
        void refetchChanges();
      });
  };
  const doStop = (): void => {
    const factory = stopAsk()?.factory === true;
    setStopAsk(undefined);
    // checked again after the answer: a run that ended while the question was open isn't stopped by an old Yes
    const st = row()?.state;
    if (st !== 'running' && st !== 'waiting') return;
    setStopping(true);
    void run(factory ? stopFactoryRun : stopRun, { id: key })
      .catch(() => undefined)
      .finally(() => setStopping(false));
  };
  const [cold] = createResource(
    () => {
      // read for the reply box's hints, which come with the body
      if (!built()) return undefined;
      const r = row();
      return r && r.state !== 'running' ? `${r.state}:${r.updated_at}` : undefined;
    },
    () => run(coldOf, { id: key }).catch(() => undefined),
  );
  const hint = (): ReturnType<typeof coldHint> => (row()?.state === 'running' ? undefined : coldHint(cold(), now(), t));
  const compactOffer = (): ReturnType<typeof compactItem> => (row()?.state === 'running' ? undefined : compactItem(cold(), now(), t));
  // what's open in the page: the steps, the notes, a command in flight, the reply's extras
  // Settings → Full transcript (`settings.transcript`: 'on'): a task opens on its transcript (v1 transcriptByDefault, B54)
  const txDefault = shell.client.entities.peek<{ value?: unknown }>('setting', entityKey(ownerOf(key), 'settings.transcript'))?.value;
  const [stepsOpen, setStepsOpen] = createSignal(txDefault === 'on' || txDefault === true);
  // the phone apps: the details column as a sheet from the menu, and Rename as a dialog (the bar's title isn't edited)
  const phone = shell.platform.phone;
  const [detailsOpen, setDetailsOpen] = createSignal(false);
  // a phone's page (stack): the app bar and its menu; a tablet's split keeps the browser's head and details column
  const phoneBar = (): boolean => phone && nav().layout === 'stack';
  const [notesOpen, setNotesOpen] = createSignal(false);
  // 👍 / 👎 on a turn (v1 feedback.js): after 👎, one optional line under that turn's trail
  const [whyFor, setWhyFor] = createSignal<string | undefined>();
  const rate = (turnId: string, rating: 'up' | 'down' | null, text?: string): void => {
    setWhyFor(rating === 'down' && !text ? turnId : undefined);
    if (text) shell.toast.success(t('shell.rate.thanks'));
    void run(rateTurn, { id: key, turnKey: entityKey(peer, turnId), rating, at: Date.now(), ...(text && { text }) }).catch(() => undefined);
  };
  const [restoring, setRestoring] = createSignal(false);
  const [picking, setPicking] = createSignal(false);
  // the latest turn shows whole; the earlier ones are the ledger's (ADR-0068). Once the reader has left the end, the
  // first turn shown whole stays whole as turns land (F-page-44): what they read doesn't fold into a row under them
  const [keepFrom, setKeepFrom] = createSignal<string | undefined>(undefined);
  const fold = createMemo(() => foldedFrom(turns(), false, keepFrom(), 1));
  const keepShown = (el: HTMLElement): void => {
    if (keepFrom() === undefined && el.scrollHeight - el.scrollTop - el.clientHeight >= 2) setKeepFrom(fold().shown[0]?.id);
  };
  const onBodyScroll = (e: Event): void => {
    kept = false;
    keepShown(e.currentTarget as HTMLElement);
    notePlace(e.currentTarget as HTMLElement);
  };
  // the older turns came as stubs (v1 #788): opening one asks for only that turn
  const opener = turnOpener({
    scope,
    bare,
    watch: (topic, s) => shell.client.peers.watch(ownerOf(key), topic, s),
    stubOf: (id) => ((x) => (x ? isStub(x.body) : undefined))(turns().find((x) => x.id === id)),
    failed: () => shell.toast.show(t('shell.task.openTurnFailed')),
  });
  // the ledger (ADR-0068): one earlier turn open in place at a time; a stub is asked for whole as it opens
  const [openTurn, setOpenTurn] = createSignal<string | undefined>(undefined);
  const openLedgerTurn = (id: string | undefined): void => {
    setOpenTurn(id);
    if (id !== undefined && turns().find((x) => x.id === id && isStub(x.body))) opener.open(id);
  };
  const noticeKind = (by: string): string => t((NOTICE_KINDS as readonly string[]).includes(by) || by === 'wisp' ? `shell.ledger.by.${by}` : 'shell.ledger.by.other', { by });
  // each row read once per row object: a patch to one turn re-reads that one (no full-list work per event, ADR-0011)
  const seen = new WeakMap<TurnRow, LedgerTurn>();
  const ledger = createMemo(
    () =>
      turns()
        .slice(0, fold().hidden)
        .map((x) => seen.get(x) ?? ((v) => (seen.set(x, v), v))(ledgerTurn(x, noticeKind))),
    undefined,
    { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) },
  );
  // files with the reply: picked, pasted or dropped, each uploading at once to the task's computer
  const att = useAttachments(shell, () => ownerOf(key));
  const files = att.files;
  // the reply box's Delegate (v1 composer.js replyTeam): the task's own team until you change it; a change goes with the
  // next follow-up (F-comp-33), the lead's team from then on
  const [picked, setPicked] = createSignal<readonly Delegate[] | undefined>(undefined);
  // changed since the last follow-up: what goes with the next one (the panel keeps showing what was sent until the
  // task's own team syncs back)
  const [teamChanged, setTeamChanged] = createSignal(false);
  const setTeam = (next: readonly Delegate[]): void => {
    setPicked(next);
    setTeamChanged(true);
  };
  const ownTeam = (): Delegate[] =>
    teamFromSetting((((row()?.body as { ext?: Record<string, unknown> } | undefined)?.ext?.['wisp.runtime'] ?? {}) as { subagents?: unknown }).subagents);
  const team = (): readonly Delegate[] => picked() ?? ownTeam();
  const [replyPreset, setReplyPreset] = createSignal('');
  // Delegate… in the reply's `+` opens the Delegate panel on its chip (UX 2026-11-18 #3)
  const [replyDelegateOpen, setReplyDelegateOpen] = createSignal(false);
  const [goal, setGoalText] = createSignal<string | undefined>(undefined);
  // a goal declared by name (`goals:`, ADR-0110), picked from Goal…: set by name while Done when still reads its text
  const [pickedReplyGoal, setPickedReplyGoal] = createSignal<DeclaredGoal>();
  // the task's peer's declared goals and Goal…'s code: fetched as the reply's `+` is warmed (./goal-kit.ts)
  const goals = useGoalKit(shell, () => ownerOf(key));
  const declared = (): readonly DeclaredGoal[] => goals.declared();
  const [when, setWhen] = createSignal<When | undefined>(undefined);
  const nav = useSignal(shell.nav);
  const harnesses = useSignal(shell.harnesses());
  // a task on Auto is named Auto until the server's row says which harness it picked (the optimistic row of a New task
  // on Auto: "auto is working", "Open in auto" before; B34)
  const harnessView = (id: string): { name: string; logo?: string } => (id === 'auto' ? { name: t('shell.home.auto') } : harnessOf(harnesses(), id));
  // the @ and / menus (plugins' prompt sources: shell-composer's tasks, crew, My Stuff and commands)
  const menu = usePromptMenu({ shell, where: 'reply', task: key, text, setText, input: () => input, id: `reply-suggest-${bare}` });
  // a plugin may fill this reply box (the file view's ✦: "@src/a.ts:12-20 "): set it, the caret at its end
  const replyBox = { text: () => text(), set: (v: string) => (setText(v), menu.typed(v)), focus: () => (input?.focus(), input?.setSelectionRange(input.value.length, input.value.length)) };
  if (shell.prompts) onCleanup(shell.prompts.provide('reply', replyBox));
  // the phone app's mic (v1 dictate.js): none elsewhere
  const mic = useMic(shell, { text, set: replyBox.set, el: () => input });
  // the reply box's harness · model (v1 S.replyRunners): the next follow-ups run on it, the task keeps its own
  const [replyRun, setReplyRun] = createSignal<Runner | undefined>(undefined);
  // what a harness's Default runs, by name, when its config says (UX 2026-11-18 #9)
  // in the task's own folder: what that project pins, else this computer's (owner's delegate 2026-11-18)
  const defaultName = useDefaultWords(shell.client, () => peer, () => String(row()?.summary['cwd'] ?? '') || undefined);
  const [runnerOpen, setPickingRunner] = createSignal(false);
  const runOf = (): ReturnType<typeof runPick> => runPick(replyRun());
  let goalInput: HTMLInputElement | undefined;
  // each prompt source with its own plugin's client, the one its extensions get
  // (a page without the shell's clients, a test's, uses the shell's own)
  const clients = ((): { client(plugin: string): ReturnType<typeof shell.client.plugin> } => {
    try {
      return useClients();
    } catch {
      return { client: (pl) => shell.client.plugin(pl) };
    }
  })();
  const sent = sentStore(shell.client);
  const sentList = useSignal(sent.value);
  const replyMode = (): string => String(row()?.summary['mode'] ?? 'auto');
  // the reply bar's Goal is lit while the task's goal is going (v1 goal.js: live || open)
  const goalGoing = (): boolean => {
    const g = row()?.summary['goal'] as { status?: unknown } | undefined;
    return g?.status === 'working' || g?.status === 'checking';
  };
  // ↑ brings back what you sent, Shift+Tab cycles the task's mode (v1 prompt-history.js, F-task-31, F-comp-4)
  const replyKeys = promptKeys({
    sent: () => sentList() as readonly string[],
    value: text,
    set: (v) => (setText(v), menu.typed(v)),
    mode: { get: replyMode, set: (m) => void run(setMode, { id: key, mode: m }).catch(() => undefined) },
  });
  // while it works, a follow-up waits behind the run when Settings → Follow-ups says Queue (v1 queued.js; else it steers)
  const followSetting = useSignal(shell.client.entities.row<{ value?: unknown }>('setting', entityKey(peer, 'runtime.followUp')));
  const queueOn = (): boolean => followSetting()?.value === 'queue';
  const working = (): boolean => row()?.state === 'running' || row()?.state === 'waiting';
  const queues = (): boolean => working() && queueOn();
  const shellHome = usePluginClient('shell').home;
  // every folder it works in, its own first (F-task-27): a chip opens that project, the list showing only it
  const projects = useProjects(shell, shellHome).list;
  const placeChips = (s: SessionRow): TaskPlaceChip[] =>
    taskPlaces(
      {
        name: String(s.summary['project'] ?? ''),
        color: String(s.summary['color'] ?? ''),
        cwd: String(s.summary['cwd'] ?? ''),
        ...(typeof s.summary['projectId'] === 'string' && { projectId: s.summary['projectId'] }),
      },
      s.body?.['dirs'],
      projects(),
    ).map((p) => {
      const title = t(p.own ? 'shell.task.placeOwn' : 'shell.task.placeAlso', { path: p.path });
      // a chip, as v1's .q-proj: it names the folder, it isn't a control
      return { name: p.name, color: p.color, title, ...(!p.own && { also: true as const }) };
    });
  // ADR-0067: plugins' prompt sources may still be on their way (the page's first moments, or a load tried again):
  // a send waits for them once, then goes whatever they did
  let waited = false;
  // Ask near the limit (v1 hold.js, ADR-0091): asked before the follow-up goes; `pass` lets the next send() through
  const [hold, setHold] = createSignal<SendHold | undefined>(undefined);
  const [checking, setChecking] = createSignal(false);
  let pass: 'checked' | 'confirmed' | undefined;
  // the hold row's Wait until the reset: the next send() goes then, as Send later does
  let waitUntil: number | undefined;
  const send = (): void => {
    const wait = waited ? undefined : shell.laterReady?.();
    if (wait) {
      return void wait.then(() => {
        waited = true;
        try {
          send();
        } finally {
          waited = false;
        }
      });
    }
    // /feedback … or /bug …: to the wisp team about this task's run, never a follow-up (v1 composer.js)
    if (isFeedback(text())) {
      const typed = text();
      setText('');
      // sending it is its own chunk (feedback-send.ts)
      return void import('./feedback-send.ts').then((m) => m.feedbackCommand(shell, typed, { home: shellHome, sessionId: key, restore: () => text() === '' && setText(typed) })).catch(() => text() === '' && setText(typed));
    }
    // /btw: a side question, answered beside the run (v1 btw.js), never a follow-up
    const q = btwQuestion(text());
    if (q) {
      setText('');
      void run(btw, { id: key, question: q, tmp: uuidv7(), at: Date.now() }).catch(() => setText(`/btw ${q}`));
      return;
    }
    // /side: a side discussion of its own beside this task (v1 side.js), never a follow-up into its conversation; its
    // page opens at once, Starting… (orch #5)
    const sq = sideAsk(text());
    if (sq !== undefined && !row()?.summary['sideOf']) {
      if (!sq) return void shell.toast.show(t('shell.side.empty'));
      setText('');
      const sideId = entityKey(peer, newRowId());
      const at = Date.now();
      const open = (attachments: readonly Attachment[]): void => {
        run(sideStart, { id: sideId, main: key, text: menu.encode(sq), at, ...(attachments.length > 0 && { attachments }) }).catch(() => text() === '' && setText(`/side ${sq}`));
        shell.navigate({ t: 'openTask', id: sideId as never });
      };
      if (files().length === 0) open([]);
      else void att.take().then(open);
      return;
    }
    const v = menu.encode(text().trim());
    if (v) sent.set((cur) => remember(cur as readonly string[], v));
    // the Done when line open and empty: nothing goes, the line takes the focus (v1 #769 replyGoalEmpty)
    const gate = goalGate(v, goal());
    if (gate === 'askGoal') {
      goalInput?.focus();
      shell.toast.show(t('shell.goal.empty'), { action: { label: t('shell.goal.sendWithout'), onClick: () => (setGoalText(undefined), send()) } });
      return;
    }
    // when it goes: the box's clock (Send later, B14), or the hold row's Wait until the reset
    const sendAt = waitUntil ?? (when() ? whenAt(when() ?? 'hour', Date.now()) : undefined);
    waitUntil = undefined;
    // asked before anything is taken (the box, the goal, the menu); not an answer to what the task asks (v1 composer.js)
    const through = pass;
    pass = undefined;
    setHold(undefined);
    const holds = shell.sendHolds;
    const agent = runOf()?.agent ?? row()?.agent ?? '';
    if (!through && sendAt === undefined && holds && v && !v.startsWith('/') && row()?.state !== 'waiting') {
      if (checking()) return;
      setChecking(true);
      void holds.check({ peer, agent, session: bare }).then((h) => {
        setChecking(false);
        if (h) return void (setHold(h), input?.focus());
        pass = 'checked';
        send();
      });
      return;
    }
    menu.reset();
    // what plugins send with it, at its top (comments on lines): the reply goes even with nothing typed
    const extra = shell
      .promptSources()()
      .filter((p) => (p.where ?? ['reply']).includes('reply') && p.pending)
      .map((p) => p.pending?.(v, { client: clients.client((p as { plugin?: string }).plugin ?? 'shell'), task: key }))
      .find((x) => x !== undefined);
    if (!v && !extra) return;
    // a plugin's prompt source may take it (@a crew member: their job, their answer under this turn)
    const pc = shell.client.plugin('shell');
    if (
      !extra &&
      files().length === 0 &&
      shell
        .promptSources()()
        .some((p) => (p.where ?? ['reply']).includes('reply') && p.send?.(v, { client: pc, task: key, ...(sendAt !== undefined && { launch: { at: sendAt } }), restore: (back) => !text().trim() && setText(back) }) === true)
    ) {
      setText('');
      setWhen(undefined);
      return;
    }
    // under a setup card, words answer it (ADR-0113 rule 4: Allow runs it, else the Deny with them), never a follow-up
    const card = setupCardOf(row());
    if (card && v && !extra && files().length === 0) {
      setText('');
      run(answerSetup, { id: key, text: v, askId: card }).catch(() => setText(v));
      return;
    }
    setText('');
    const g = goal();
    const named = goalWire(g, pickedReplyGoal());
    if (g) void run(setGoal, { id: key, text: g, ...(typeof named === 'object' && { name: named.name }) }).catch(() => undefined);
    setGoalText(undefined);
    setPickedReplyGoal(undefined);
    const turnId = entityKey(peer, newRowId());
    const at = Date.now();
    const idx = turns().length;
    // the box clears at once; with files, the turn starts once they're up (usually already)
    extra?.taken();
    // the team picked since the last follow-up goes with this one (none picked: the task keeps its own)
    const sentTeam = teamChanged() ? (picked() ?? []) : undefined;
    if (sentTeam !== undefined) setTeamChanged(false);
    const go = (attachments: readonly Attachment[]): void => {
      run(followUp, {
        id: key,
        text: extra ? extra.text : v,
        turnId,
        idx,
        at,
        ...(attachments.length > 0 && { attachments }),
        ...(runOf() ? { run: runOf() as NonNullable<ReturnType<typeof runOf>> } : {}),
        ...(queues() && { queue: true }),
        ...(through === 'confirmed' && { confirmLimit: true }),
        ...(sentTeam !== undefined && { team: teamToSend(sentTeam) }),
      }).catch((err: unknown) => {
        extra?.back();
        setText(v);
        if (sentTeam !== undefined && !teamChanged()) setTeamChanged(true);
        // the peer knew better: the row, the words back in the box (v1: the 409 `hold`)
        const reason = (err as { reason?: string } | null)?.reason;
        const answer = shell.sendHolds?.answers(reason);
        if (answer) void answer({ peer, agent, session: bare }).then((h) => setHold(h ?? { reason: reason ?? '', say: (err as Error).message }));
      });
    };
    if (files().length === 0) go([]);
    else void att.take().then(go);
  };
  /** an answer's `next` chip, `question` option, `actions` reply: sent as a follow-up */
  const reply = (v: string): void => {
    run(followUp, { id: key, text: v, turnId: entityKey(peer, newRowId()), idx: turns().length, at: Date.now() }).catch(() => undefined);
  };
  /** an answer's chip without ⌘: into the reply box, to read first */
  const draft = (v: string): void => {
    setText(v);
    input?.focus();
  };
  const side = (kind: 'changes' | 'shell'): void => shell.navigate({ t: 'toggleSide', side: { kind, id: key as never } });
  // the panel button and \ open it on the newest thing (UX 2026-11-18 #5): the PR, else Changes with a diff, else Shell
  const sideNewest = (): void => {
    if (nav().side.kind !== 'none') return shell.navigate({ t: 'closeSide' });
    const touched = turns().reduce((n, x) => n + (x.body?.ext?.['wisp.runtime']?.changes ?? x.body?.changes ?? []).length, 0);
    shell.navigate({ t: 'openSide', side: { kind: newestSide(row() as never, touched), id: key as never } });
  };
  // Take over (T, Open in <harness>): the task's conversation in its harness's own CLI, in shell-task's Terminal tab
  const takeOver = (): void => shell.navigate({ t: 'openSide', side: { kind: 'view', id: 'shell-task.term' as never } });
  // Rerun… (v1 rerunDialog): the task's prompt, mode, harness and computer, editable, then a new task (F-task-43/44)
  // Fork… (v1 forkTask): its dialog first, from a turn's Fork too (that turn); nothing is made until its Fork
  const openFork = (turn?: string): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.fork' as never, params: { id: key, ...(turn && { turn }) } } });
  const openRerun = (agent?: string): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rerun' as never, params: { id: key, ...(agent && { agent }) } } });
  // Ship it is wisp.ship's dialog (an overlay view, `ship.dialog`); without that plugin, navigation drops it
  const ship = (): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'ship.dialog' as never, params: { key } } });
  // v1: leaving a task (done, archived, deleted) opens the next that needs you, else New task
  const next = (): void => {
    const id = nextNeedsYou(untrack(sessions), key);
    if (id) shell.navigate({ t: 'openTask', id: (id.includes(':') ? id : entityKey(peer, id)) as never });
  };
  // a phone's page (ADR-0065 step 13): the next / previous task in the list's order, the order the list shows now (a
  // filter changing it moves the neighbours with it); a task that left the list meanwhile keeps its last place
  const order = useSignal(shell.listOrder?.keys ?? (() => [] as readonly string[]));
  const stepsAt = createMemo((was: { readonly steps: TaskSteps; readonly at: number | undefined }) => {
    const o = order();
    return { steps: taskSteps(o, key, was.at), at: placeIn(o, key, was.at) };
  }, { steps: {}, at: undefined });
  const steps = (): TaskSteps => stepsAt().steps;
  const stepTo = (to: string | undefined): void => {
    if (to === undefined) return;
    shell.selection.set(to);
    // a pager: the step takes this task's place in history, so Back returns to the list, not through every step
    shell.navigate({ t: 'openTask', id: to as never, replace: true });
  };
  const markDone = (): void => {
    const was = row()?.state;
    // v1 actions.js: a task still working stays where it is and says why; never off to the next task with an
    // error about this one shown there (orch #18)
    if (was === 'running' || was === 'waiting') return void shell.toast.show(t('shell.done.working'));
    void run(setState, { id: key, state: 'done' }).catch(() => undefined);
    shell.navigate({ t: 'markDone', id: key as never });
    next();
    // with Undo, as the row's Done (audit A #13)
    shell.toast.success(t('shell.toast.done'), { action: { label: t('shell.toast.undo'), onClick: () => void run(setState, { id: key, state: was && was !== 'done' ? was : 'review' }).catch(() => undefined) } });
  };
  // "Open in Claude Code" (v1's Take over; owner, 2026-11-10)
  const harnessName = (): string => harnessView(row()?.agent ?? '').name;
  // v1 moreMenu: what a task's right-click menu offers is here too: the entries other plugins attach to
  // `shell.row:menu` (Side discussion, Share, Ship it, Run app, Save as recipe, Add to crew, Set goal, Keep worktree);
  // the shell's own (Rerun, Fork, Changes, Shell) are on this page already
  const rowMenu = useSignal(shell.extensions('shell.row:menu'));
  const camel = (k: string): string => k.replace(/_([a-z])/g, (_m, c: string) => c.toUpperCase());
  const menuRow = (): Record<string, unknown> => {
    const r = (row() ?? {}) as unknown as Record<string, unknown>;
    const top = Object.fromEntries(
      Object.entries(r)
        .filter(([k]) => k !== 'body' && k !== 'summary')
        .map(([k, v]) => [camel(k), v]),
    );
    return { ...((r['body'] as object | undefined) ?? {}), ...top, ...((r['summary'] as object | undefined) ?? {}), id: key };
  };
  const extProps = (x: typeof rowMenu extends () => readonly (infer E)[] ? E : never) => ({
    client: clients.client(x.plugin),
    config: x.config,
    session: () => menuRow() as { id: string; agent: string },
  });
  // v1 actions.js moreMenu, in its order: on a phone (its bar folded) Transcript, Rerun, Open in first; then Rename,
  // Copy task ID, Copy resume command, Open project in editor, Fork…, Compact, Lean runs, what plugins attach to a
  // task's menu (Side discussion, Share, Changes and Shell where the bar doesn't have them, Run app, Save as recipe,
  // Add to crew, Set goal), Save for later, Keep worktree, Restore to before latest turn, Archive, Delete task
  // the bar folds into More where its desk-only buttons don't show (v1 actions.js `folded`: split, not pane-wide): a
  // phone's stack, or a pane under 860 × 520 (the kit's `pane` container query); at 900 px the bar hid Transcript,
  // Rerun… and Open in and More didn't have them (the parity gate's task.transcript and task.rerun found nothing)
  const [paneWide, setPaneWide] = createSignal(true);
  const barFolded = (): boolean => nav().layout === 'stack' || !paneWide();
  const busyNow = (): boolean => row()?.state === 'running' || row()?.state === 'waiting';
  const lastTurn = (): (TurnBody & { id: string }) | undefined => {
    const x = turns().at(-1);
    return x ? ({ ...(x.body as TurnBody), id: x.id } as TurnBody & { id: string }) : undefined;
  };
  // TRY-22: Tab fills one of the newest answer's suggested follow-ups (not while it works: those were for an older one)
  const followUps = createMemo((): string[] => (busyNow() ? [] : suggestedFollowUps(lastTurn()?.presentation?.blocks)));
  const ghost = (): { rest: string; key: string; full: string } | undefined => {
    const full = completionOf(text(), followUps());
    return full ? { rest: full.slice(text().length), key: t('shell.reply.tab'), full } : undefined;
  };
  const canRollback = (): boolean => {
    const l = lastTurn();
    return row()?.agent !== 'wisp' && !busyNow() && !!l && (!!(l.changes ?? l.ext?.['wisp.runtime']?.changes)?.length || !!l.checkpoint?.commit);
  };
  const forLaterNow = (): boolean => isSaved(row()?.summary['forLater']);
  const canSaveForLater = (): boolean => !row()?.archived && (row()?.state === 'review' || row()?.state === 'done');
  // the shell's Rerun is More's own (above); Changes is on the head's bar (docs/ux/2026-11-18-desktop.md #4), and on a
  // desktop Merge is under its Ship ▾; Shell is the row's entry here, but More's own on a phone
  const canShip = (): boolean => shipsFromWorktree(row()?.summary);
  const moreExts = (): ReturnType<typeof rowMenu> =>
    rowMenu().filter(
      (x) =>
        x.def.bar && !x.def.bar.rowOnly && x.id !== 'shell.row.rerun' && x.id !== 'shell.row.changes' && !(phoneBar() && x.id === 'shell.row.shell') && !(!phoneBar() && x.id === 'wisp.ship.rowMerge') && (x.def.if?.(extProps(x) as never) ?? true),
    );
  // v1 #786 taskMenuHtml: the row's menu's sections and submenus (task-menu.ts), with what only More has: on a phone
  // the bar's folded Transcript, Rerun and Open in first; Compact and Lean runs with the task itself; Copy resume
  // command under Copy ›, Open project in editor and Restore under Files ›
  const moreEntries = (): MenuEntry[] => {
    const own: TaskMenuEntry[] = [];
    // the head's bar keeps Changes, Ship, Done (docs/ux/2026-11-18-desktop.md #4): Transcript, Rerun and Open in are
    // here, with their keys, on every width
    own.push({ section: 'open', order: -30, entry: { value: 'transcript', label: t('shell.task.transcript'), icon: 'transcript', keys: 'V', checked: stepsOpen() } });
    if (!row()?.summary['sideOf'])
      own.push(
        { section: 'open', order: -20, entry: { value: 'rerun', label: t('shell.task.rerun'), icon: 'rerun', keys: '⇧R' } },
        { section: 'open', order: -10, entry: { value: 'openIn', label: t('shell.task.openIn', { harness: harnessName() }), icon: 'terminal', keys: 'T' } },
      );
    // the phone apps have no details bar: what it holds is here, after Open in (v1's order), and Details first
    if (phoneBar())
      own.push(
        { section: 'open', order: -40, entry: { value: 'details', label: t('shell.phone.details'), icon: 'info' } },
        // the side panes are views (Show › View) and the head's swipe twins go with them, so the first section stays
        // short (QA r2 #5)
        { sub: 'view', order: 10, entry: { value: 'changes', label: t('shell.task.changes'), icon: 'file', checked: nav().side.kind === 'changes' } },
        { sub: 'view', order: 20, entry: { value: 'shell', label: t('shell.task.shell'), icon: 'shell', checked: nav().side.kind === 'shell' } },
        ...(canShip() ? [{ section: 'open' as const, order: 30, entry: { value: 'ship', label: t('shell.task.ship'), icon: 'send' as const } }] : []),
        { section: 'open', order: 40, entry: { value: 'done', label: t('shell.task.done'), icon: 'done' } },
        // the head's swipe's twins: at the list's first / last task (or none) they are there, disabled
        { section: 'look', order: 200, entry: { value: 'next-task', label: t('shell.phone.nextTask'), icon: 'chevron', ...(steps().next === undefined && { disabled: true }) } },
        { section: 'look', order: 201, entry: { value: 'previous-task', label: t('shell.phone.previousTask'), icon: 'back', ...(steps().previous === undefined && { disabled: true }) } },
      );
    own.push({ section: 'task', order: -10, entry: { value: 'rename', label: t('shell.more.rename'), icon: 'pen', keys: 'M' } });
    // Pin and Mark unread, as the row's menu has them (one menu: task-menu.ts group 2)
    const pinned = row()?.summary['pinned'] === true;
    // (desktop: the phone's menu keeps the sections its QA rounds settled, task-menu.ts phoneMenu)
    if (!phoneBar()) own.push({ section: 'place', order: -40, entry: { value: 'pin', label: t(pinned ? 'shell.more.unpin' : 'shell.more.pin'), icon: 'pin' } });
    if (!phoneBar() && !busyNow() && !row()?.archived) own.push({ section: 'place', order: -35, entry: { value: 'read', label: t('shell.more.unread'), icon: 'eye' } });
    if (compactOffer()) own.push({ section: 'task', order: 45, entry: { value: 'compact', label: t('shell.cold.compact'), icon: 'minus', hint: compactOffer()?.note ?? '' } });
    // v1's task menu: Lean runs for this task, or as Settings says (Claude Code only)
    if (row()?.agent === 'claude')
      own.push({
        section: 'task',
        order: 46,
        entry: {
          kind: 'group',
          id: 'lean',
          label: t('shell.task.lean'),
          items: (['follow', 'on', 'off'] as const).map((v) => ({
            value: `lean:${v}`,
            label: t(`shell.task.lean.${v}`),
            checked: ((row()?.body as { lean?: string } | undefined)?.lean ?? 'follow') === v,
          })),
        },
      });
    own.push({ sub: 'copy', order: -20, entry: { value: 'copy-id', label: t('shell.more.copyId'), icon: 'copy' } });
    // Copy resume command and Open project in editor are the shell's row entries (row-menu.ts), here as on the row
    if (canRollback()) own.push({ sub: 'files', order: 130, entry: { value: 'rollback', label: t('shell.more.rollback'), icon: 'replay' } });
    if (forLaterNow()) own.push({ section: 'place', order: -20, entry: { value: 'later', label: t('shell.more.unlater'), icon: 'bookmark', keys: 'L' } });
    else if (canSaveForLater()) own.push({ section: 'place', order: -20, entry: { value: 'later', label: t('shell.more.later'), icon: 'bookmark', keys: 'L' } });
    if (row()?.archived) own.push({ section: 'place', order: -10, entry: { value: 'unarchive', label: t('shell.more.unarchive'), icon: 'archive', keys: 'A' } });
    else if (!busyNow()) own.push({ section: 'place', order: -10, entry: { value: 'archive', label: t('shell.task.archive'), icon: 'archive', keys: 'A' } });
    return taskMenu({
      entries: [...own, ...moreExts().map((x) => menuEntryOf(x, extProps(x) as never, t))],
      danger: [{ value: 'delete', label: t('shell.more.delete'), icon: 'trash', danger: true, keys: '⌫' }],
      t,
      // the phone apps' ⋯: short, its longer sections behind one row each (QA r2 #5)
      ...(phoneBar() && { compact: true }),
    });
  };
  // the same array while the menu says the same: every patch to the open task (a streamed token) built new entries, and
  // the kit, keying rows by object, rebuilt the closed menu each time (perf r1 #12, #15)
  const more = createMemo(moreEntries, [], { equals: sameEntries });
  // the browser edits the head's title in place; the phone apps' bar asks in a dialog
  // Rename (⋯, M): a dialog on every screen, as the row's (QA round 14 desktop #3: the head's in-place edit lost its
  // caret to the menu giving focus back to ⋯); a click on the title still edits it in place
  const startRename = (): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rename' as never, params: { id: key } } });
  const onMore = (v: string): void => {
    if (v === 'next-task') stepTo(steps().next);
    else if (v === 'previous-task') stepTo(steps().previous);
    else if (v === 'transcript') setStepsOpen(!stepsOpen());
    else if (v === 'rerun') openRerun();
    else if (v === 'openIn') takeOver();
    else if (v === 'shell') side('shell');
    else if (v === 'details') setDetailsOpen(true);
    else if (v === 'changes') side('changes');
    else if (v === 'ship') ship();
    else if (v === 'done') markDone();
    else if (v === 'fork') openFork();
    else if (v === 'compact') void run(compactNow, { id: key }).catch(() => undefined);
    else if (v.startsWith('lean:')) {
      const lean = v.slice(5);
      void run(setLean, { id: key, ...(lean !== 'follow' && { lean: lean as 'on' | 'off' }) }).catch(() => undefined);
    } else if (v === 'copy') void shell.platform.copy(location.href).then((ok) => shell.toast.show(t(ok ? 'shell.task.copied' : 'shell.task.copyFailed')));
    else if (v === 'archive') {
      void run(archive, { id: key, archived: true }).catch(() => undefined);
      shell.navigate({ t: 'archive', id: key as never });
      next();
      // with Undo, as the row's Archive (audit A #13)
      shell.toast.success(t('shell.toast.archived'), { action: { label: t('shell.toast.undo'), onClick: () => void run(archive, { id: key, archived: false }).catch(() => undefined) } });
    } else if (v === 'delete')
      // asked first, as the row's Delete (audit A #4): the shell's one Delete task dialog
      shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.delete' as never, params: { id: key } } }); else if (v === 'rename') startRename();
    else if (v === 'copy-id') void shell.platform.copy(bare).then((ok) => ok && shell.toast.show(t('shell.more.copiedId')));
    else if (v === 'later') {
      // with Undo, as the row menu and the hover button say it (audit B #7)
      const l = laterToggle(row()?.summary['forLater']);
      void run(forLater, { id: key, ...l.next }).catch(() => undefined);
      shell.toast.show(t(l.toast), { action: { label: t('shell.toast.undo'), onClick: () => void run(forLater, { id: key, ...l.undo }).catch(() => undefined) } });
    }
    else if (v === 'pin') void run(pin, { id: key, pinned: row()?.summary['pinned'] !== true }).catch(() => undefined);
    else if (v === 'read') {
      // Mark unread: it stays bold in the list, so the page leaves it (staying would read it again): the next task that
      // needs you, else home, saying so with Undo (audit B #5: a silent jump read as the task changing under you)
      void run(markRead, { id: key, unread: true }).catch(() => undefined);
      const out = unreadLeave(untrack(sessions), key, row()?.title ?? '', peer);
      shell.navigate(out.to.t === 'openTask' ? { t: 'openTask', id: out.to.id as never } : { t: 'home' });
      shell.toast.success(t(out.toast.key, out.toast.vars), {
        action: {
          label: t('shell.toast.undo'),
          onClick: () => {
            void run(markRead, { id: key, unread: false }).catch(() => undefined);
            shell.navigate({ t: 'openTask', id: key as never });
          },
        },
      });
    }
    else if (v === 'unarchive') void run(archive, { id: key, archived: false }).catch(() => undefined);
    else if (v === 'rollback') {
      const l = lastTurn();
      if (l) setRestoreAsk(l.id);
    } else if (v.startsWith('x:')) {
      const x = rowMenu().find((e) => `x:${e.id}` === v);
      if (x) x.def.bar?.select?.(extProps(x) as never);
    }
  };
  const reopen = (): void => void run(setState, { id: key, state: 'review' }).catch(() => undefined);
  const hasPr = (): boolean => newestSide(row() as never, 0) === 'pr';
  const toShip = (): boolean => {
    const w = row()?.summary['toShip'] as { dirty?: number; ahead?: number } | null | undefined;
    return (w?.dirty ?? 0) > 0 || (w?.ahead ?? 0) > 0;
  };
  // what's changed now in the task's folder, as git says it (the head's Changes +n −n, as the Changes pane shows): not
  // every turn's edits added up (audit A #9: +6 −6 for a one-line fix after two follow-ups, still there after Revert
  // or after the merged PR's worktree went). Read again when a turn ends or what the worktree holds changes; none when
  // git can't read the folder (it's gone) or shows nothing, or the task has no worktree.
  const changeWhere = createMemo(() => changeWhereOf(row(), turns().at(-1)));
  const [changedNow, { refetch: refetchChanges }] = createResource(changeWhere, (w) => {
    const at = whereOf(w);
    return at ? run(changesNow, { key, ...at }).catch(() => null) : null;
  });
  const diff = createMemo(() => {
    const stats = changeWhere() ? (changedNow.latest?.stats ?? []) : [];
    return { files: stats.length, plus: stats.reduce((n, f) => n + f.add, 0), minus: stats.reduce((n, f) => n + f.del, 0) };
  });
  // Ship ▾: Ship it…, the PR's pane, Merge when ready (wisp.ship's row entry, when it offers it); Ship it… alone is a button
  const shipMenu = (): MenuEntry[] => {
    const merge = rowMenu().find((x) => x.id === 'wisp.ship.rowMerge' && (x.def.if?.(extProps(x) as never) ?? true));
    const mergeLabel = merge?.def.bar?.text?.(extProps(merge) as never) ?? (merge?.def.bar?.label ? t(merge.def.bar.label) : undefined);
    return [
      { value: 'ship', label: t('shell.task.ship'), icon: 'send', keys: 'S' },
      ...(hasPr() ? [{ value: 'pr', label: t('shell.task.openPr'), icon: 'branch' as const, keys: 'P' }] : []),
      ...(mergeLabel ? [{ value: 'x:wisp.ship.rowMerge', label: mergeLabel, icon: 'merge' as const }] : []),
    ];
  };
  const onShip = (v: string): void => {
    if (v === 'pr') shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } });
    else onMore(v);
  };
  // the head's one bar (docs/ux/2026-11-18-desktop.md #4), the same with and without the panel: Changes +n −n, Ship ▾,
  // Done (Reopen when done), ⋯; Transcript, Rerun, Open in and Shell are in ⋯ with their keys
  const headBar = (s: SessionRow): HeadAction[] => {
    const d = diff();
    const ship = shipMenu();
    const done = s.state === 'done';
    return [
      // a diff the turns made (with its figures), or work the worktree holds that they don't show (dirty, ahead)
      ...(d.files > 0 || toShip()
        ? [{ id: 'changes', key: 'D', icon: 'file' as const, label: t('shell.task.changes'), title: t('shell.task.changesTitle'), part: 'split.changes', ...(d.files > 0 && { figures: { plus: `+${d.plus}`, minus: `−${d.minus}` } }), act: { onClick: () => side('changes') } }]
        : []),
      // only a worktree task ships (v1 shippable; audit B #2: one in its project's folder failed "no such session")
      ...(canShip()
        ? [
            {
              id: 'ship',
              key: 'S',
              icon: 'send' as const,
              label: t('shell.task.shipShort'),
              title: t('shell.task.shipTitle'),
              part: 'split.ship',
              act: ship.length > 1 ? { menu: { items: ship, onSelect: onShip } } : { onClick: () => onMore('ship') },
            },
          ]
        : []),
      done
        ? { id: 'done', key: 'E', icon: 'replay', label: t('shell.task.reopen'), title: t('shell.task.reopenTitle'), part: 'split.done', act: { onClick: reopen } }
        : { id: 'done', key: 'E', icon: 'done', label: t('shell.task.doneShort'), title: t('shell.card.done'), tone: 'good', part: 'split.done', act: { onClick: markDone } },
      { id: 'more', icon: 'dots', label: t('shell.task.more'), title: t('shell.task.more'), part: 'split.more', bare: true, act: { menu: { items: more(), onSelect: onMore } } },
    ];
  };
  // the shell's task keys (v1 kbd.js, builtins.tsx): what this page's buttons and More menu do
  const onKey = (e: Event): void => {
    const act = (e as CustomEvent<TaskKey>).detail;
    const did = ((): boolean => {
      if (act === 'transcript') setStepsOpen(!stepsOpen());
      else if (act === 'rerun' || act === 'fork' || act === 'archive' || act === 'delete') onMore(act);
      else if (act === 'openIn') takeOver();
      else if (act === 'shell') side('shell');
      else if (act === 'changes' || act === 'panel') {
        if (act === 'panel' && nav().side.kind !== 'none') shell.navigate({ t: 'closeSide' });
        else if (act === 'panel') sideNewest();
        else side('changes');
      } else if (act === 'pr') shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } });
      else if (act === 'ship') {
        if (!canShip()) return false;
        ship();
      }
      else if (act === 'done') {
        if (row()?.state === 'done') reopen();
        else markDone();
      } else if (act === 'later') void run(forLater, { id: key, forLater: !isSaved(row()?.summary['forLater']) }).catch(() => undefined);
      else if (act === 'reply') input?.focus();
      else if (act === 'rename') startRename();
      else if (act === 'collapse') {
        setStepsOpen(false);
        setOpenTurn(undefined);
      } else if (act === 'top') document.querySelector('[data-kit="turns"]')?.firstElementChild?.scrollIntoView({ block: 'start' });
      else if (act.startsWith('answer:')) {
        // the question waiting over the reply box, else the latest answer's question block: its Nth choice
        const n = Number(act.slice(7)) - 1;
        const opts = document.querySelectorAll<HTMLButtonElement>('[data-part="task.ask"] button, [data-part="block.question.option"]:not(:disabled)');
        const b = opts[n];
        if (!b) return false;
        b.click();
      } else return false;
      return true;
    })();
    if (did) e.preventDefault();
  };
  document.addEventListener(TASK_KEY, onKey);
  onCleanup(() => document.removeEventListener(TASK_KEY, onKey));
  // a key pressed on this task's row in the list (V, T, P, D, S, ⌃`): the list opened it, the page does it (audit A #12)
  const pendingAct = takePendingKey(key, Date.now());
  if (pendingAct) queueMicrotask(() => onKey(new CustomEvent(TASK_KEY, { detail: pendingAct, cancelable: true })));
  const attach = (o: PickOptions): void => {
    setPicking(true);
    void shell.platform.pickFiles(o).then((picked) => {
      setPicking(false);
      if (picked.length) att.add(picked);
    });
  };
  return (
    <Show when={row()}>
      {(s) => {
        // inside the reply box, over the text (the queued follow-ups, then wisp.later's, v1 .c-queued): `shell.task:composer-top`. Made once:
        // a JSX prop beside the box's reactive spread would be rebuilt (rows, an edit in progress) each time the task starts or stops
        const composerTop = <Slot input="shell.task:composer-top" layout="contents" id={key} session={() => s() as never} />;
        // a factory run names its factory where another task names its harness (ADR-0114): it runs no agent of its own
        const factory = (): string | undefined => runFactory(s().summary as { factory?: unknown });
        const factoryWords = (): string => t('shell.task.factory', { name: factory() ?? '' });
        // its reply box: the stations do the work, and Stop while the run goes (its follow-ups are refused)
        const factoryRun = (): ReturnType<typeof factoryReply> => factoryReply(s().summary as { factory?: unknown });
        const factoryHint = (): { long: string; short: string; link?: { label: string; title: string; act: Act } } | undefined => {
          const f = factoryRun();
          if (!f) return undefined;
          const w = factoryFoot(f, phone, t);
          if (!w.stop) return { long: w.hint, short: w.hint };
          // asked first, as a task's Stop is (audit G13)
          const stop: Act = { onClick: () => setStopAsk(factoryStopQuestion(t)) };
          return { long: w.hint, short: w.hint, link: { label: t('shell.task.factoryStop'), title: t('shell.task.factoryStopTitle'), act: stop } };
        };
        const h = (): { name: string; logo?: string } => (factory() ? { name: factoryWords() } : harnessView(replyRun()?.agent ?? s().agent));
        // the run a follow-up takes: the reply box's pick, else the task's own harness, model and thinking (QA desktop r3 #1)
        const shownRun = (): Runner => replyRun() ?? taskRunner(s());
        // the reply's harness past 80% of a plan window: Home's ring on the reply box's chip (UX 2026-11-18 #8)
        // the account the follow-up runs on: the task's own (its row's `account`; the harness's own login is budget's
        // 'default'), or every account of another harness picked for the reply (it runs on that one's saved account)
        const ringAccount = (): string =>
          shownRun().agent === s().agent ? String((row()?.body as { account?: unknown } | undefined)?.account || 'default') : '';
        const ring = usePlanRing(shell, { peer: () => peer, agent: () => shownRun().agent, account: ringAccount });
        // the task's own, for its details (not the reply box's pick)
        const own = (): { name: string; logo?: string } => (factory() ? { name: factoryWords() } : harnessView(s().agent));
        // on its harness's Default: the model that runs, by the same answer as the reply chip (QA r10 #2), else "Default model"
        const ownWords = (): string => factory() ? factoryWords() : t('shell.task.harness', { name: own().name, model: modelWords(String(s().summary.model || '')) || defaultName(s().agent, t('shell.task.defaultModel')) });
        // the reply box's Delegate panel: on its button in the old footer, on its chip in the desktop's one footer
        const replyTeamAct = (): Act => ({
          // v1 composer.js: the same Delegate panel as New task's, the lead this task's harness where it runs
          panel: {
            label: t('shell.home.delegate'),
            part: 'composer.team.panel',
            cardPart: 'delegate.panel',
            placement: 'top-start',
            content: () => (
              <DelegatePanel
                lead={{
                  agent: replyRun()?.agent ?? s().agent,
                  run: runWords(h().name, shownRun(), defaultName(shownRun().agent, t('shell.runner.default'))),
                  logo: h().logo,
                  place: peer === shellHome ? { icon: 'laptop', label: t('shell.card.thisComputer'), title: t('shell.home.where') } : { icon: 'server', label: shell.client.peers.name(peer), title: t('shell.home.where') },
                }}
                team={team()}
                onChange={setTeam}
                peer={peer}
                places={[]}
                preset={replyPreset()}
                onPreset={setReplyPreset}
              />
            ),
          },
        });
        // UX 2026-11-18 #3 (docs/ux/2026-11-18-desktop.md): the desktop reply box takes New task's one footer, `+` for what's
        // off (attach, Goal, Delegate, send later), a chip with ✕ for what's on, the harness with its model
        const replyGoalOn = (): boolean => goal() !== undefined || goalGoing();
        // the reply box's menus keep their entries while they say the same: their props are made again on every patch to
        // the task (the cap reads the row), and new entries rebuilt the closed mode and + menus on each streamed token
        // (perf r1 #15). The mode's memo is made after `mode` below
        // Goal…: the declared goals by name, then a new one to write; with none declared, straight to Done when
        const newReplyGoal = (): void => (setPickedReplyGoal(undefined), setGoalText(''), queueMicrotask(() => goalInput?.focus()));
        const takeReplyGoal = (g: DeclaredGoal): void => {
          setPickedReplyGoal(g);
          setGoalText(g.text);
          queueMicrotask(() => replyBox.focus());
        };
        // recent in this task's project first (the goals its tasks were given), then A–Z
        const replyGoalAct = (): Act =>
          goals.kit()?.goalAct({
            shell,
            declared: declared(),
            recent: () => {
              const own = row();
              return goals.kit()?.recentGoals(sessions(), projects(), own ? projectOfRow(own, projects())?.id : undefined) ?? [];
            },
            t,
            take: takeReplyGoal,
            fresh: newReplyGoal,
          }) ?? { onClick: newReplyGoal };
        const replyPlus = (): LauncherTool =>
          plusTool({
            part: 'composer.plus',
            label: t('shell.home.plus'),
            title: t('shell.reply.plusTitle'),
            refocus: replyBox.focus,
            onWarm: goals.warm,
            sections: [
              [{ id: 'attach', part: 'composer.attach', icon: 'clip', label: t('shell.home.attach'), act: attachAct(shell, attach, picking()) }],
              [
                !replyGoalOn() && { id: 'goal', part: 'composer.goal', icon: 'target', label: t('shell.home.goalRow'), act: replyGoalAct() },
                team().length === 0 && !replyDelegateOpen() && { id: 'team', part: 'composer.team', icon: 'agent', label: t('shell.home.delegateRow'), act: { onClick: () => setReplyDelegateOpen(true) } },
              ],
              [!when() && { id: 'later', part: 'composer.sendLater', icon: 'clock', label: t('shell.home.later'), act: { menu: { items: whenMenu(t, now()), onSelect: (w) => setWhen(w as When) } } }],
            ],
          });
        const plusEntries = (x: LauncherTool): readonly MenuEntry[] => x.act?.menu?.items ?? [];
        const replyPlusNow = createMemo(replyPlus, undefined, { equals: (a, b) => a !== undefined && a.label === b.label && a.title === b.title && sameEntries(plusEntries(a), plusEntries(b)) });
        const replyChips = (): LauncherTool[] => {
          const panel = replyTeamAct().panel;
          const text = (goal() ?? '').trim();
          const named = goalWire(goal(), pickedReplyGoal());
          return [
            ...(replyGoalOn()
              ? [
                  {
                    id: 'goal',
                    part: 'composer.goal',
                    icon: 'target' as const,
                    label: typeof named === 'object' ? t('shell.home.goalChip', { text: named.name }) : text ? t('shell.home.goalChip', { text }) : t('shell.home.goal'),
                    title: t('shell.task.goal'),
                    text: true,
                    on: true,
                    act: { onClick: () => (goal() === undefined ? setGoalText('') : undefined, queueMicrotask(() => goalInput?.focus())) },
                    ...(goal() !== undefined && { clear: { label: t('shell.home.goalOff'), act: { onClick: () => setGoalText(undefined) } } }),
                  },
                ]
              : []),
            ...(team().length > 0 || replyDelegateOpen()
              ? [
                  {
                    id: 'team',
                    part: 'composer.team',
                    icon: 'agent' as const,
                    label: team().length ? t(teamSize(team()) === 1 ? 'shell.home.delegates.one' : 'shell.home.delegates.other', { count: teamSize(team()) }) : t('shell.home.delegate'),
                    title: t('shell.home.delegateTitle'),
                    text: true,
                    on: true,
                    ...(panel && { act: { panel: { ...panel, open: replyDelegateOpen(), onOpenChange: setReplyDelegateOpen } } }),
                    // its ✕ from the start, so the open panel is never drawn again on the first pick
                    clear: team().length > 0 ? { label: t('shell.home.delegateOff'), act: { onClick: () => setTeam([]) } } : { label: t('shell.home.delegateClose'), act: { onClick: () => setReplyDelegateOpen(false) } },
                  },
                ]
              : []),
            ...(when()
              ? [
                  {
                    id: 'later',
                    part: 'composer.sendLater',
                    icon: 'clock' as const,
                    label: t('shell.when.chip', { when: startsAt(whenAt(when() ?? 'hour', now()), now()) }),
                    title: whenTip(whenAt(when() ?? 'hour', now()), now()),
                    text: true,
                    on: true,
                    act: { menu: { items: whenMenu(t, now()), onSelect: (w: string) => setWhen(w as When) } },
                    clear: { label: t('shell.when.clear'), act: { onClick: () => setWhen(undefined) } },
                  },
                ]
              : []),
          ];
        };
        const branch = (): string => String(s().summary['branch'] ?? '');
        const color = (): string => String(s().summary['color'] ?? 'slate');
        const ov = createMemo(() => (!closedReady() ? [] : shownOverlaps(overlapsOf(s()), closed() as unknown as Dismissed | undefined, s().id)));
        const mode = (): Mode => (['plan', 'edits', 'auto'].includes(String(s().summary['mode'])) ? (s().summary['mode'] as Mode) : 'auto');
        const modeItems = createMemo(() => modeMenu(t, mode()), [], { equals: sameEntries });
        // a channel's cap (F-chan-10): the pill after the mode; Lift is the user's switch above it, to Full auto
        const capView = (): ReplyCap | undefined => {
          const c = capOf(s().summary);
          if (!c) return undefined;
          const m = t(`shell.mode.${c}`);
          return {
            label: t('shell.cap.label', { mode: m }),
            title: t('shell.cap.title', { mode: m }),
            lift: { label: t('shell.cap.lift'), title: t('shell.cap.liftTitle'), act: { onClick: () => void run(setMode, { id: key, mode: 'auto' }).catch(() => undefined) } },
          };
        };
        // its usage, its subagents' in, and how full its context is (v1 #785): the server sums them on the row
        const usage = createMemo(() =>
          taskUsage(
            rowUsage(s().summary as unknown as Readonly<Record<string, unknown>>),
            () => turns().map((x) => x.body.stats as TurnStatsLike | undefined),
            t,
            (id) => shell.navigate({ t: 'openTask', id: id as never }),
            () => shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'usage' } } }),
          ),
        );
        // one turn whole: the latest (the live one) after the ledger, or an earlier one opened in it
        // one turn, made once for its id (B35): its row changes as it runs and ends (status, stats, the answer), and each
        // change was a new object, so the whole turn (its prompt card, its answer, its buttons) was made again. `turn`
        // and `b` read the latest row; what's derived from them is a function, read where it's shown.
        const renderTurn = (tr: () => TurnRow): JSX.Element => {
          const turn = through(tr);
          const b = through(() => tr().body);
          // the harness this turn ran on, not the reply box's pick (audit B #6)
          const turnHarness = (): { name: string; logo?: string } => harnessView(promptHarness(b as { agent?: unknown }, replyRun()?.agent, row()?.agent ?? ''));
          const hero = (): { title?: string; summary?: string } | undefined => b.presentation?.blocks?.find((x) => x.type === 'hero');
          // the steps come as the turn's events (the task's topic); a turn row that carries its own still reads
          // the turn's events, once per change of them: its steps, the working panel's count, ticker and quiet note all read
          // these (the note read them again, filtered and sorted from every event, on each streamed token: perf r1 #15)
          const turnEvents = createMemo(() => stepsOf(turn.id));
          const steps = createMemo((): { kind: string; label?: string; title?: string; detail?: string; out?: string; bad?: boolean; seq?: number }[] => {
            const all = turnEvents();
            const kindOf = (e: EventRow): string => e.kind ?? e.body?.kind ?? '';
            // a tool's output: on the step itself, or the tool_output that names its call
            const outputs = new Map(all.filter((e) => kindOf(e) === 'tool_output' && e.body?.callId).map((e) => [e.body?.callId ?? '', { text: e.body?.text ?? '', seq: e.seq }]));
            const ev = all.filter((e) => kindOf(e) === 'tool' || kindOf(e) === 'tool_error');
            if (!ev.length) return [...(b.activity ?? [])];
            return ev.map((e) =>
              kindOf(e) === 'tool_error'
                ? { kind: 'error', label: e.body?.text ?? e.body?.name ?? '', bad: true }
                : {
                    kind: e.body?.tool ?? 'tool',
                    label: e.body?.label ?? '',
                    ...(e.body?.title && { title: e.body.title }),
                    ...(e.body?.detail && { detail: e.body.detail }),
                    ...(e.body?.file && { file: e.body.file }),
                    // the event that carried the output names it to Reveal (ADR-0050)
                    ...(e.body?.output ? { out: e.body.output, seq: e.seq } : outputOf(outputs.get(e.body?.callId ?? ''))),
                  },
            );
          });
          const cwd = String((s() as { body?: { cwd?: string } }).body?.cwd ?? s().summary['cwd'] ?? '');
          // the task's folder out of a step's words: one expression for the turn, not one per line per token
          const inFolder = cwd ? new RegExp(`(/private)?${cwd.replace(/^\/private/, '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/?`, 'g') : undefined;
          const here = (text: string): string => (inFolder ? text.replace(inFolder, '') : text);
          // the quiet note's view of the steps and whether a delegate still works: memos, not work done on every token (a
          // scan of every task's row per token, perf r1 #15)
          const quietSteps = createMemo(() => turnEvents().map((e) => ({ kind: e.kind ?? e.body?.kind, t: e.t, outAt: e.body?.outAt, call: e.body?.callId, output: e.body?.output })));
          const delegating = createMemo(() => allRows().some((x) => x.parent_id === key && x.state === 'running'));
          const count = (k: string): number => steps().filter((a) => a.kind === k).length;
          const blocks = () => b.presentation?.blocks ?? [];
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
          // the working panel, made once for this turn and only while it runs, under the turn's own owner: made inside
          // the props' spread it was made again on every change of the task's row (its live line, ~3 a second while it
          // streams), so the panel and its Stop were taken down and put back each time (B31). Its words update in place.
          const live = createMemo(() => turn.status === 'running' && turn.id === turns().at(-1)?.id && (s().state === 'running' || s().state === 'waiting'));
          const owner = getOwner();
          let made: JSX.Element | undefined;
          // v1 transcript.js renderLiveTranscript: with Transcript on, a running turn shows its timeline so far above the
          // working panel, growing as steps come (owner, 2026-11-17: Transcript did nothing on a working task). Made once.
          let madeTx: JSX.Element | undefined;
          const liveTx = (): JSX.Element =>
            (madeTx ??= runWithOwner(owner, () => (
              <Show when={stepsOpen()}>
                <TurnTranscript steps={steps()} changes={changes()} env={env} here={here} status={turn.status ?? ''} t={t} />
              </Show>
            )));
          const working = (): JSX.Element =>
            (made ??= runWithOwner(owner, () => (
              <WorkingPanel
                part="task.working"
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
                      {/* v1 transcript.js renderTranscript: with Transcript on, the turn's timeline comes before its answer */}
                      <Show when={stepsOpen()}>
                        <TurnTranscript
                          steps={[
                            ...steps(),
                            ...((b as { presentation?: { notes?: string } }).presentation?.notes
                              ? [{ kind: 'text', label: String((b as { presentation?: { notes?: string } }).presentation?.notes) }]
                              : []),
                          ]}
                          changes={changes()}
                          env={env}
                          here={here}
                          status={turn.status ?? ''}
                          t={t}
                        />
                      </Show>
                      <Answer
                        blocks={blocks()}
                        ctx={Object.assign(answerContext({
                          shell,
                          key,
                          turnId: turn.id,
                          env,
                          blocked: () => blockedOf(b),
                          latest: () => turn.id === turns().at(-1)?.id,
                          title: () => s().title,
                          transcript: stepsOpen,
                          meta: () => [
                            ...(b.endedAt && b.endedAt > turn.started_at ? [duration(b.endedAt - turn.started_at)] : []),
                            ...(b.stats?.costUsd ? [`$${b.stats.costUsd.toFixed(2)}`] : []),
                            ...(changes().length ? [t(changes().length === 1 ? 'shell.task.touched.one' : 'shell.task.touched.other', { n: changes().length })] : []),
                          ],
                          reply,
                          draft,
                          openChanges: () => side('changes'),
                          apps: () => ((b as { ext?: Record<string, unknown> }).ext?.['wisp.apps'] ?? {}) as Record<string, unknown>,
                          tour: s().agent === 'wisp',
                        }), { comment })}
                      />
                    </>
          )) : undefined)));
          return (
            <>
              <TurnView
                id={turn.id}
                latest={turn.id === turns().at(-1)?.id}
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
                    ...((u) => (u ? { src: u } : {}))(uploadUrl(shellHome, key, a.id ?? '')),
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
                timeTitle={longTime(turn.started_at)}
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
                acts={trailActs(stepsOpen() ? 0 : steps().length, { read: count('read'), edit: count('edit'), run: count('run') }, t)}
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
                  onToggle: () => setStepsOpen(!stepsOpen()),
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
        };
        return (
          <>
            <TaskBody
              bodyRef={(el) => {
                bodyEl = el;
                el.addEventListener('scroll', onBodyScroll, { passive: true });
                onCleanup(() => el.removeEventListener('scroll', onBodyScroll));
                // the pane the kit's `pane` container query reads: wide at 860 × 520 and up, as v1's split.js pane-wide
                // (looked up once the body is in the page: a ref runs before it's inserted)
                if (typeof ResizeObserver !== 'undefined') {
                  const ro = new ResizeObserver(([e]) => {
                    const b = e?.contentBoxSize[0];
                    if (b) setPaneWide(b.inlineSize >= 860 && b.blockSize >= 520);
                  });
                  queueMicrotask(() => {
                    const pane = el.closest('[data-kit="split-main"]');
                    if (pane) ro.observe(pane);
                  });
                  onCleanup(() => ro.disconnect());
                }
              }}
              {...(!phoneBar() && {
                details: (
                <Show when={built()}>
                <TaskDetails
                  harness={ownWords()}
                  {...(own().logo && { logo: own().logo })}
                  project={String(s().summary['project'] ?? '')}
                  color={color()}
                  places={placeChips(s())}
                  status={statusOf(s(), t)}
                  {...(branch() && { branch: { head: branch().replace(/-[0-9a-f]{6}$/, ''), tail: branch().match(/-[0-9a-f]{6}$/)?.[0] ?? '', title: branch() } })}
                  {...((u) => (u ? { usage: u } : {}))(usage())}
                  actions={[]}
                />
                </Show>
                ),
              })}
              turns={
                <Show
                  when={built()}
                  fallback={
                    // the turns' place, the frame before they're built (design/feedback.md: the destination's shell at once)
                    <TurnsSkeleton />
                  }
                >
                  {/* the band over the turns (wisp.goal's): `shell.task:band`, the task's row as its session */}
                  <Slot input="shell.task:band" layout="stack" id={key} session={() => s() as never} />
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
                </Show>
              }
              dock={
                <Show when={built()}>
                  <Show when={ov()[0]}>
                    {(o) => (
                      <OverlapNote
                        head={t('shell.overlap.with', { title: titleOf(o().id) })}
                        files={overlapLine(o(), t)}
                        {...(ov().length > 1 && { more: t('shell.overlap.more', { n: ov().length - 1 }) })}
                        moreTitle={t('shell.overlap.moreTitle')}
                        open={t('shell.overlap.open')}
                        openTitle={t('shell.overlap.openTitle')}
                        onOpen={() => shell.navigate({ t: 'openTask', id: o().id as never })}
                        dismiss={{ label: t('shell.overlap.dismiss'), title: t('shell.overlap.dismissTitle'), onDismiss: () => closeNote(s().id, overlapsOf(s())) }}
                        moreAct={{
                          menu: {
                            items: ov()
                              .slice(1)
                              .map((x) => ({ value: x.id, label: titleOf(x.id), hint: overlapLine(x, t) })),
                            onSelect: (id) => shell.navigate({ t: 'openTask', id: id as never }),
                          },
                        }}
                      />
                    )}
                  </Show>
                  {/* what waits on you, over the reply box (shell-task's question and queue): `shell.task:foot` */}
                  <Slot input="shell.task:foot" layout="stack" id={key} session={() => s() as never} />
                  <ReplyBox
                    hold={((hd) =>
                      hd && {
                        say: hd.say,
                        goLabel: t('shell.hold.send'),
                        goTitle: t('shell.hold.sendTitle'),
                        keepLabel: t('shell.hold.keep'),
                        onGo: () => ((pass = 'confirmed'), send()),
                        ...(waitable(hd.until, Date.now()) && { wait: { label: t('shell.hold.wait', { at: aheadLabel(hd.until ?? 0, Date.now()) }), title: t('shell.hold.waitTitle') } }),
                        onWait: () => ((waitUntil = hd.until), send()),
                        onKeep: () => (setHold(undefined), input?.focus()),
                      })(hold())}
                    placeholder={
                      factoryRun()
                        ? factoryFoot(factoryRun() ?? { name: '', live: false }, phone, t).placeholder
                        : t(replyPlaceholderKey(row(), row()?.state !== 'running' && !!lastTurn()?.presentation?.blocks?.some((x) => x.type === 'question')), { name: h().name })
                    }
                    value={text()}
                    onInput={(v, e) => {
                      setText(v);
                      menu.typed(v, e);
                    }}
                    onSend={send}
                    onKey={replyKeys}
                    top={composerTop}
                    {...(working() && {
                      queue: {
                        label: t('shell.reply.queue'),
                        title: queueOn() ? t('shell.reply.queueOn', { name: h().name }) : t('shell.reply.queueOff', { name: h().name }),
                        on: queueOn(),
                        act: { onClick: () => void run(setSetting, { peer, key: 'runtime.followUp', value: queueOn() ? 'interrupt' : 'queue' }).catch(() => undefined) },
                      },
                    })}
                    suggest={menu.suggest()}
                    // the @ and / menus win while they're open
                    ghost={menu.suggest() ? undefined : ghost()}
                    onPaste={(e) => menu.paste(e) || att.paste(e)}
                    onFiles={att.add}
                    inputRef={(el) => (input = el)}
                    attach={t('shell.home.attach')}
                    attachAct={attachAct(shell, attach, picking())}
                    mic={mic ? { label: mic.label(), on: mic.on(), act: { onClick: mic.toggle } } : undefined}
                    mode={{
                      label: t(`shell.mode.${mode()}`),
                      title: t('shell.home.fullAutoTitle'),
                      icon: MODE_ICON[mode()],
                      act: { menu: { items: modeItems(), onSelect: (m) => void run(setMode, { id: key, mode: m }).catch(() => undefined) } },
                      cap: capView(),
                    }}
                    // a factory run's reply box has no harness or model chip: its stations do the work (ADR-0114 rule 14)
                    {...(!factoryRun() && { runner: {
                      title: t('shell.task.runner', { name: runWords(h().name, shownRun(), defaultName(shownRun().agent, t('shell.runner.default'))) }),
                      ...(h().logo && { logo: h().logo }),
                      act: { onClick: () => setPickingRunner(!runnerOpen()) },
                      open: runnerOpen(),
                      // the model it runs on beside the logo (UX 2026-11-18 #3), the phone keeps the bare logo
                      ...(!phone && { model: (shownRun().model ? modelWords(shownRun().model) : '') || defaultName(shownRun().agent, t('shell.runner.default')) }),
                      ...(ring() && { usage: ring() }),
                    } })}
                    {...(!phone && { plus: ((p: LauncherTool) => ({ label: p.label, title: p.title, act: p.act ?? {} }))(replyPlusNow()), chips: replyChips(), chipsMore: { label: t('shell.home.moreOptions'), title: t('shell.reply.moreChips') } })}
                    team={t('shell.home.delegateTitle')}
                    teamAct={replyTeamAct()}
                    goal={t('shell.task.goal')}
                    goalOn={goal() !== undefined || goalGoing()}
                    goalAct={{ onClick: () => setGoalText(goal() === undefined ? '' : undefined) }}
                    {...(goal() !== undefined && {
                      goalLine: {
                        label: t('shell.home.doneWhen'),
                        placeholder: t('shell.home.goalPlaceholder'),
                        keys: t('shell.home.goalKeys'),
                        value: goal() ?? '',
                        onInput: setGoalText,
                        ref: (el: HTMLInputElement) => (goalInput = el),
                      },
                    })}
                    later={t('shell.home.later')}
                    laterAct={{ menu: { items: whenMenu(t, now()), onSelect: (w) => setWhen(w as When) } }}
                    // the phone apps: the bar's options behind one chip and its sheet (UX 2026-11-18 phone #1)
                    {...(phone && { phoneOptions: { options: t('shell.reply.options'), mode: t('shell.reply.permissions'), agent: t('shell.reply.agent'), goal: t('shell.reply.goalShort') } })}
                    // a reply sends (v1 composer.js), and a scheduled one says when (QA r11 #1), as New task's Start does
                    send={when() ? t('shell.reply.sendAt', { at: aheadLabel(whenAt(when() ?? 'hour', now()), now()) }) : t('shell.reply.send')}
                    {...(checking() && { sendAct: { pending: true } })}
                    hint={factoryHint() ?? hint()}
                    extras={[
                      ...files().map((f, i) => ({
                        id: `file:${i}:${f.name}`,
                        icon: 'clip' as const,
                        label: f.name,
                        removeLabel: t('shell.home.remove', { name: f.name }),
                        onRemove: () => att.remove(f),
                      })),
                      ...(when() && phone
                        ? [
                            {
                              id: 'when',
                              icon: 'clock' as const,
                              label: t('shell.when.chip', { when: startsAt(whenAt(when() ?? 'hour', now()), now()) }),
                              removeLabel: t('shell.when.clear'),
                              onRemove: () => setWhen(undefined),
                            },
                          ]
                        : []),
                    ]}
                  />
                  <RunnerPicker
                    open={runnerOpen()}
                    onOpenChange={setPickingRunner}
                    anchor={() => document.querySelector('[data-part="composer.harness"]')}
                    back={() => input}
                    peer={peer}
                    defaultWord={(a) => defaultName(a, '')}
                    value={shownRun()}
                    onChange={setReplyRun}
                  />
                </Show>
              }
            />
            <Show
              when={phoneBar() && leave}
              fallback={
            <>
            <TaskHead
              actions={<TaskHeadSlot id={key} session={() => s() as never} />}
              title={s().title}
              color={color()}
              renameLabel={t('shell.task.rename')}
              panelLabel={t('shell.task.panel')}
              closeLabel={t('shell.task.close')}
              {...(leave && { onBack: leave, backLabel: t('shell.home.back') })}
              onClose={() => shell.navigate({ t: 'home' })}
              onRename={(title) => void run(rename, { id: key, title }).catch(() => undefined)}
              onPanel={sideNewest}
              panelOn={nav().side.kind !== 'none'}
              bar={headBar(s())}
            />
            </>
              }
            >
              {(back) => (
                <PhoneTaskBar
                  title={s().title}
                  onBack={back()}
                  head={() => <TaskHeadSlot id={key} session={() => s() as never} />}
                  // UX 2026-11-18 phone #6: the same entries in three groups, the rest in This task ›
                  menu={() => phoneTaskMenu(more(), t)}
                  onSelect={onMore}
                  // UX 2026-11-18 phone #3: what the task holds, each a tap to its pane (a sheet over the task)
                  status={() => [
                    ...(diff().files > 0
                      ? [{ id: 'changes', icon: 'file' as const, label: t('shell.phone.files', { count: diff().files, n: diff().files, plus: `+${diff().plus}`, minus: `−${diff().minus}` }), on: nav().side.kind === 'changes', onSelect: () => side('changes') }]
                      : []),
                    ...(hasPr() ? [{ id: 'pr', icon: 'branch' as const, label: ((b) => (b ? t('shell.phone.prNumber', { pr: b }) : t('shell.phone.pr')))(prBadge(row() as never)), on: nav().side.kind === 'pr', onSelect: () => shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } }) }] : []),
                  ]}
                  details={() => (
                    <TaskDetails
                      harness={ownWords()}
                      {...(own().logo && { logo: own().logo })}
                      project={String(s().summary['project'] ?? '')}
                      color={color()}
                      places={placeChips(s())}
                      status={statusOf(s(), t)}
                      {...(branch() && { branch: { head: branch().replace(/-[0-9a-f]{6}$/, ''), tail: branch().match(/-[0-9a-f]{6}$/)?.[0] ?? '', title: branch() } })}
                      {...((u) => (u ? { usage: u } : {}))(usage())}
                      actions={[]}
                    />
                  )}
                  detailsOpen={detailsOpen()}
                  onDetails={setDetailsOpen}
                  swipe={{
                    get onNext() {
                      const to = steps().next;
                      return to === undefined ? undefined : () => stepTo(to);
                    },
                    get onPrevious() {
                      const to = steps().previous;
                      return to === undefined ? undefined : () => stepTo(to);
                    },
                  }}
                />
              )}
            </Show>
            <Show when={stopAsk()}>
              {(q) => (
                <Dialog
                  open
                  role="alertdialog"
                  size="sm"
                  title={q().title}
                  description={q().body}
                  onOpenChange={(open) => !open && setStopAsk(undefined)}
                  part="task.stopAsk"
                  footer={
                    <Inline gap="4" justify="end">
                      <Button variant="ghost" onClick={() => setStopAsk(undefined)}>
                        {t('shell.stop.cancel')}
                      </Button>
                      <Button variant="danger" part="task.stopAsk.ok" onClick={doStop}>
                        {q().ok}
                      </Button>
                    </Inline>
                  }
                />
              )}
            </Show>
            <Show when={restoreAsk()}>
              <Dialog
                open
                role="alertdialog"
                size="sm"
                title={t('shell.restore.title')}
                description={t('shell.restore.body')}
                onOpenChange={(open) => !open && setRestoreAsk(undefined)}
                part="task.restoreAsk"
                footer={
                  <Inline gap="4" justify="end">
                    <Button variant="ghost" onClick={() => setRestoreAsk(undefined)}>
                      {t('shell.stop.cancel')}
                    </Button>
                    <Button variant="danger" part="task.restoreAsk.ok" onClick={doRestore}>
                      {t('shell.restore.ok')}
                    </Button>
                  </Inline>
                }
              />
            </Show>
          </>
        );
      }}
    </Show>
  );
}
