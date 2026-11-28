// The open task's page state (TaskView.tsx): its row, turns and events, the reader's place, Find, the reply, the
// head's actions and the questions they ask. The markup reads it as one object (TaskPage), each part of the page from
// its own component (TaskDock.tsx, TaskSide.tsx, TaskTop.tsx, TaskTurns.tsx).
import '../i18n/lazy.ts';
import '../i18n/home.ts';
import { defineCommand, entityKey, ownerOf, type EntityKey, type Json } from '@wisp/client';
import { createMemo, createResource, createSignal, onCleanup, untrack } from 'solid-js';
import { changeWhere as changeWhereOf, whereOf } from './change-where.ts';
import { useSignal, type LedgerString, type TaskPlaceChip, type TaskScroll } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { blockStrings } from '../block-strings.ts';
import { newestSide } from './side-newest.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { useClients, usePluginClient } from '../slot.tsx';
import { afterNextPaint, buildsAtOnce } from './task-paint.ts';
import { NO_EVENTS, eventsByTurn } from './turn-events.ts';
import { useProjects } from './projects.ts';
import { taskPlaces } from './project-pick.ts';
import { doneRefused, nextNeedsYou, overlapsOf, undoneState } from './rows-view.ts';
import { coldHint, compactItem } from './cold.ts';
import { useNow } from '../clock.ts';
import { turnClock } from './turn-clock.ts';
import { useChatSettings } from './chat-settings.ts';
import { asZenList, withZen, zenSet, zenStore } from './task-zen.ts';
import { displayCombo } from '../keys.ts';
import { dismiss, dismissedStore, type Dismissed } from './overlap-dismiss.ts';
import { rateTurn, coldOf, isSaved, setState } from './commands.ts';
import { uploadPath, uploadUrl } from './answer-of.ts';
import { peerMedia } from './media-src.ts';
import { harnessOf } from './harness.ts';
import { placeIn, taskSteps, type TaskSteps } from './task-step.ts';
import { isStub, turnOpener } from './turn-window.ts';
import { NOTICE_KINDS, ledgerTurn } from './ledger-turns.ts';
import type { LedgerTurn } from '@wisp/ui/pure';
import { TELEMETRY_TOPIC, feedbackOn } from './feedback-on.ts';
import type { EventRow, TurnBody, TurnRow } from './task-rows.ts';
import { useTaskActions } from './task-actions.ts';
import { useTaskPlace } from './task-place.ts';
import { useTaskFind } from './task-find-view.ts';
import { useTaskAsks } from './task-asks.tsx';

import { useTaskReply } from './task-reply.ts';

/** The ledger's words: `shell.ledger.<key>`, a count of one in its own words (`.one`). */
const ONE: Readonly<Partial<Record<LedgerString, string>>> = { turns: 'n', matches: 'n', count: 'd', sum: 'files' };

/** git's read of the task's folder (wisp.git's `git.changes`): stats only, for the head's Changes figures */
const changesNow = defineCommand<{ key: string; path: string; base: string }, { readonly stats?: readonly { readonly add: number; readonly del: number }[] } | null>({
  id: 'git.changes',
  owner: (a) => a.key as EntityKey,
  wire: (a) => ({ path: a.path, base: a.base, stageState: false, statsOnly: true }),
  optimistic: 'none',
  quiet: true,
  pending: 'what changed',
});

export type TaskPage = ReturnType<typeof useTaskPage>;

export function useTaskPage(props: { readonly id: string; readonly onBack?: () => void }) {
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
  const allEvents = useSignal(shell.client.entities.list<EventRow | readonly EventRow[]>('event'));
  // a turn's steps, oldest first: its tool calls and its messages. Grouped once per change of the events, a turn whose
  // events didn't change keeps its array, so only that turn recomputes (turn-events.ts, perf r2 #5)
  const byTurn = createMemo<Map<string, readonly EventRow[]>>((prev) => eventsByTurn(allEvents(), prev));
  const stepsOf = (turnId: string): readonly EventRow[] => byTurn().get(turnId) ?? NO_EVENTS;
  const sessions = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  // a turn names its session by the bare id the server gave it, or by the key (an optimistic one); shallow equality so
  // unrelated tasks' turn updates never re-run turns / fold / ledger here
  const realTurns = createMemo(
    () =>
      allTurns()
        .filter((x) => x.session_id === key || x.session_id === bare)
        .sort((a, b) => a.idx - b.idx),
    undefined,
    { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) },
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
  // the task's body and its scroll control, held by TaskBody's refs: its place (task-place.ts) and Find read them
  let bodyEl: HTMLElement | undefined;
  let scrollCtrl: TaskScroll | undefined;
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
  // hoisted once per task (not once per rendered turn) and reuses `sessions` above
  const delegating = createMemo(() => sessions().some((x) => (x.parent_id === key || x.parent_id === bare) && x.state === 'running'));
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
  // Settings → Chat: Chat mode (`settings.transcript`: 'on'), Tool calls in chat, Show thinking, Send shortcut,
  // Chat width, Time format — all reactive so changing a setting updates open tasks immediately.
  // per-user preferences, written on this computer (Settings → Chat): read from home, not the task's computer
  // Zen mode (⌃., task-zen.ts): this task alone shows its messages and answer only, over what Settings says
  const zens = zenStore(shell.client);
  const zenList = useSignal(zens.value);
  const zen = createMemo(() => asZenList(zenList()).includes(key));
  const toggleZen = (): void => {
    const on = !untrack(zen);
    zens.set((cur) => zenSet(asZenList(cur), key, on) as unknown as Json);
    shell.toast.show(t(on ? 'shell.zen.on' : 'shell.zen.off', { key: displayCombo('ctrl+.', shell.platform.apple).join('') }));
  };
  const chat = withZen(useChatSettings(shell.client, usePluginClient('shell').home), zen);
  const clock = (at: number): string => turnClock(at, undefined, chat.timeFormat());
  // Transcript view (V): off by default in both Chat mode (messages + answer only) and Cards mode, persisted per task
  // with its scroll place so returning to a task restores the same view and scroll position.
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
  // the reader's place, Transcript on or off, the fold behind the ledger and the end (task-place.ts)
  const { leave, stepsOpen, setTaskTranscript, showStream, stepsShown, fold, toEnd, onBodyScroll } = useTaskPlace({
    shell, key, onBack: props.onBack, chat, built, turns, bodyEl: () => bodyEl, scrollCtrl: () => scrollCtrl,
  });
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
  // Find in task, Copy turn as Markdown, each turn's open steps and its chat steps (task-find-view.ts)
  const { openOf, findEnvs, drawnRows, finding, copyTurn, findTurns, openFind, closeFind, goFind, chatStepsOf } = useTaskFind({
    shell, t, chat, turns, stepsOf, fold, openTurn, openLedgerTurn, opener, showStream, setTaskTranscript, bodyEl: () => bodyEl, scrollCtrl: () => scrollCtrl,
  });
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
  const nav = useSignal(shell.nav);
  const harnesses = useSignal(shell.harnesses());
  // a task on Auto is named Auto until the server's row says which harness it picked (the optimistic row of a New task
  // on Auto: "auto is working", "Open in auto" before; B34)
  const harnessView = (id: string): { name: string; logo?: string } => (id === 'auto' ? { name: t('shell.home.auto') } : harnessOf(harnesses(), id));
  // each prompt source with its own plugin's client, the one its extensions get
  // (a page without the shell's clients, a test's, uses the shell's own)
  const clients = ((): { client(plugin: string): ReturnType<typeof shell.client.plugin> } => {
    try {
      return useClients();
    } catch {
      return { client: (pl) => shell.client.plugin(pl) };
    }
  })();
  const shellClient = usePluginClient('shell');
  const shellHome = shellClient.home;
  // a prompt's attachment: the page's own URL, or (the phone app) one the task's computer's transport gives; none while asked
  const attachmentUrl = (id: string): string | undefined => {
    const rel = uploadUrl(shellHome, key, id);
    if (!rel) return undefined;
    const got = peerMedia(shell.client, shellClient.http(ownerOf(key)), ownerOf(key), uploadPath(id), Date.now());
    return got === 'relative' ? rel : (got ?? undefined);
  };
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
    const was = row();
    // v1 actions.js: a task still working stays where it is and says why; never off to the next task with an
    // error about this one shown there (orch #18). A terminal task isn't: Done closes its CLI.
    if (doneRefused(was)) return void shell.toast.show(t('shell.done.working'));
    void run(setState, { id: key, state: 'done' }).catch(() => undefined);
    shell.navigate({ t: 'markDone', id: key as never });
    next();
    // with Undo, as the row's Done (audit A #13)
    const back = undoneState(was);
    shell.toast.success(t('shell.toast.done'), { action: { label: t('shell.toast.undo'), onClick: () => void run(setState, { id: key, state: back }).catch(() => undefined) } });
  };
  // "Open in Claude Code" (v1's Take over; owner, 2026-11-10)
  const harnessName = (): string => harnessView(row()?.agent ?? '').name;
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
  // the reply: its draft, caret, files, team, goal, time, harness, menus and Send (task-reply.ts)
  const r = useTaskReply({ shell, t, key, bare, peer, shellHome, run, clients, row, turns, built, busyNow, lastTurn, toEnd });
  const canRollback = (): boolean => {
    const l = lastTurn();
    return row()?.agent !== 'wisp' && !busyNow() && !!l && (!!(l.changes ?? l.ext?.['wisp.runtime']?.changes)?.length || !!l.checkpoint?.commit);
  };
  const forLaterNow = (): boolean => isSaved(row()?.summary['forLater']);
  const canSaveForLater = (): boolean => !row()?.archived && (row()?.state === 'review' || row()?.state === 'done');
  // the browser edits the head's title in place; the phone apps' bar asks in a dialog
  // Rename (⋯, M): a dialog on every screen, as the row's (QA round 14 desktop #3: the head's in-place edit lost its
  // caret to the menu giving focus back to ⋯); a click on the title still edits it in place
  const startRename = (): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rename' as never, params: { id: key } } });
  const toggleTranscript = (): void => {
    setTaskTranscript(!stepsOpen());
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
  // Stop's and Restore's questions (task-asks.tsx)
  const { askStop, setStopAsk, stopping, setRestoreAsk, restoring, asks } = useTaskAsks({ shell, t, key, bare, run, row, sessions, harnessView, refetchChanges });
  // the head's bar, its More menu and the task keys (task-actions.ts)
  const { more, onMore, headBar } = useTaskActions({
    shell, t, key, bare, peer, run, clients, row, sessions, nav, phoneBar, busyNow, lastTurn, canRollback, forLaterNow, canSaveForLater,
    compactOffer, hasPr, toShip, diff, harnessName, stepsShown, steps, stepTo, input: r.input, scrollCtrl: () => scrollCtrl, markDone, reopen,
    next, ship, side, sideNewest, takeOver, startRename, openFind, openFork, openRerun, toggleTranscript, setTaskTranscript, setOpenTurn,
    setDetailsOpen, setRestoreAsk, zen, toggleZen,
  });
  return {
    r,
    bodyEl: (): HTMLElement | undefined => bodyEl,
    setBodyEl: (el: HTMLElement): void => void (bodyEl = el),
    scrollCtrl: (): TaskScroll | undefined => scrollCtrl,
    setScrollCtrl: (ctrl: TaskScroll): void => void (scrollCtrl = ctrl),
    shell, t, ledgerT, key, rates, built, row, stepsOf, sessions, turns, titleOf, closed, closedReady, closeNote,
    peer, run, now, tick, delegating, hint, chat, clock, phone, detailsOpen, setDetailsOpen, phoneBar, notesOpen,
    setNotesOpen, whyFor, setWhyFor, rate, leave, showStream, stepsShown, fold, onBodyScroll, opener, openTurn,
    openLedgerTurn, openOf, findEnvs, drawnRows, finding, copyTurn, findTurns, closeFind, goFind, chatStepsOf,
    ledger, nav, harnessView, clients, shellHome, attachmentUrl, projects, placeChips, side, sideNewest, openFork,
    steps, stepTo, harnessName, setPaneWide, lastTurn, toggleTranscript, hasPr, diff, askStop, setStopAsk, stopping,
    setRestoreAsk, restoring, asks, more, onMore, headBar,
  };
}
