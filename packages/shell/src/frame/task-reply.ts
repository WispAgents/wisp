// The open task's reply (TaskView.tsx): what's being written (a draft, kept per task), the caret it takes when the
// page opens, its files, team, goal, time and harness, the @ and / menus, ↑ and Shift+Tab, and Send with what an
// answer's chips send or put in the box.
import '../i18n/lazy.ts';
import '../i18n/home.ts';
import { createEffect, createMemo, createSignal, on, onCleanup, untrack, type Accessor } from 'solid-js';
import { entityKey, newRowId, ownerOf, type EntityKey, type PeerId } from '@wisp/client';
import { useSignal } from '@wisp/ui';
import { goalWire, type DeclaredGoal } from './goal-gate.ts';
import { useGoalKit } from './goal-kit.ts';
import { completionOf, suggestedFollowUps } from './follow-ghost.ts';
import { promptKeys, sentStore } from './prompt-keys.ts';
import { draftValue, replyDraft } from './drafts.ts';
import { followUp, setGoal, setMode } from './commands.ts';
import { usePromptMenu } from './prompt-menu.ts';
import { useAttachments } from './attach.ts';
import { useMic } from './dictate.ts';
import type { PickOptions } from '../platform/types.ts';
import { runPick, useDefaultWords, type Runner } from './runner.ts';
import type { When } from './pickers.ts';
import { teamFromSetting, type Delegate } from './team.ts';
import { useReplySend } from './reply-send.ts';
import { openedByStep } from './stepped.ts';
import { mayTakeCaret } from './reply-focus.ts';
import type { useShell } from '../context.tsx';
import type { usePluginClient } from '../slot.tsx';
import type { SessionRow } from '../rows.ts';
import type { TurnBody, TurnRow } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What the reply reads of the open task (TaskView.tsx, task-page.ts). */
export interface TaskReplyCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly key: EntityKey;
  /** the task's own id, the part of the key after `<peer>:` */
  readonly bare: string;
  readonly peer: PeerId;
  readonly shellHome: ReturnType<typeof usePluginClient>['home'];
  readonly run: Shell['client']['commands']['run'];
  readonly clients: { client(plugin: string): ReturnType<Shell['client']['plugin']> };
  readonly row: () => SessionRow | undefined;
  readonly turns: () => readonly TurnRow[];
  /** the page's body is built (task-paint.ts): the caret comes then */
  readonly built: Accessor<boolean>;
  readonly busyNow: () => boolean;
  readonly lastTurn: () => (TurnBody & { id: string }) | undefined;
  readonly toEnd: () => void;
}

export type TaskReply = ReturnType<typeof useTaskReply>;

export function useTaskReply(c: TaskReplyCtx) {
  const { shell, t, key, bare, peer, shellHome, run, clients, row, turns, built, busyNow, lastTurn, toEnd } = c;
// the reply being written is a draft (kind 5): kept across reloads and updates, per task (v1 F-task-32)
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
const [picking, setPicking] = createSignal(false);
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
// the @ and / menus (plugins' prompt sources: shell-composer's tasks, crew, My Stuff and commands)
// its mentions kept beside the draft, so a reload or coming back to the task still sends them (doc25a)
const menu = usePromptMenu({ shell, where: 'reply', task: key, draft: [rKey, rField], text, setText, input: () => input, id: `reply-suggest-${bare}` });
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
const sent = sentStore(shell.client);
const sentList = useSignal(sent.value);
const replyMode = (): string => String(row()?.summary['mode'] ?? 'auto');
// the reply bar's Goal is lit while the task's goal is going (v1 goal.js: live || open)
const goalGoing = (): boolean => {
  const g = row()?.summary['goal'] as { status?: unknown } | undefined;
  return g?.status === 'working' || g?.status === 'checking';
};
// the going goal's condition: the lit Goal chip says it (it read a bare "Goal" and opened an empty line)
const liveGoalText = (): string => {
  const g = row()?.summary['goal'] as { text?: unknown } | undefined;
  return goalGoing() && typeof g?.text === 'string' ? g.text : '';
};
// the going goal edited where the band edits it (v1 goal.js: the lit Goal opens Edit goal…), its judges kept
const editLiveGoal = (): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'goal.edit' as never, params: { task: key } } });
// Done when set on its own (Enter with nothing typed in the box, or Set): no follow-up needed to start it. `kick`:
// a task whose last turn is done is judged on it now, so the goal starts working at once (v1 Goals#set)
const setReplyGoal = (): void => {
  const g = goal();
  if (g === undefined) return;
  if (!g.trim()) {
    goalInput?.focus();
    shell.toast.show(t('shell.goal.empty'));
    return;
  }
  const named = goalWire(g, pickedReplyGoal());
  const back = { text: g, picked: pickedReplyGoal() };
  setGoalText(undefined);
  setPickedReplyGoal(undefined);
  void run(setGoal, { id: key, text: g.trim(), kick: true, ...(typeof named === 'object' && { name: named.name }) }).catch(() => {
    // refused: the line comes back as it was
    if (goal() === undefined) (setGoalText(back.text), setPickedReplyGoal(back.picked));
  });
  queueMicrotask(() => replyBox.focus());
};
// ↑ brings back what you sent, Shift+Tab cycles the task's mode (v1 prompt-history.js, F-task-31, F-comp-4)
const replyKeys = promptKeys({
  sent: () => sentList() as readonly string[],
  value: text,
  // what was sent is stored (`@[Title](wisp:id)`): back as the box shows it, its mentions kept (doc25a)
  set: (v) => menu.typed(menu.restore(v)),
  mode: { get: replyMode, set: (m) => void run(setMode, { id: key, mode: m }).catch(() => undefined) },
});
// while it works, a follow-up waits behind the run when Settings → Follow-ups says Queue (v1 queued.js; else it steers)
const followSetting = useSignal(shell.client.entities.row<{ value?: unknown }>('setting', entityKey(peer, 'runtime.followUp')));
const queueOn = (): boolean => followSetting()?.value === 'queue';
const working = (): boolean => row()?.state === 'running' || row()?.state === 'waiting';
const queues = (): boolean => working() && queueOn();
// Send: slash commands, the goal gate, the hold near a limit, plugins' sources, then the follow-up (reply-send.ts)
const { send, sendConfirmed, sendWaiting, hold, setHold, checking } = useReplySend({
  shell, t, key, bare, peer, shellHome, run, clients, row, turns, text, setText, input: () => input, menu, sent, att, files,
  goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, setReplyGoal, goalInput: () => goalInput, when, setWhen, picked, teamChanged,
  setTeamChanged, runOf, queues, toEnd,
});
/** an answer's `next` chip, `question` option, `actions` reply: sent as a follow-up */
const reply = (v: string): void => {
  toEnd();
  run(followUp, { id: key, text: v, turnId: entityKey(peer, newRowId()), idx: turns().length, at: Date.now(), queue: queues() }).catch(() => undefined);
};
/** an answer's chip without ⌘: into the reply box, to read first. The view goes to the end, by the box it fills (on a
 *  phone the chip sat far above it, behind the keyboard): from the tail, the hold-view keeps it there as the keyboard
 *  shortens the view */
const draft = (v: string): void => {
  setText(v);
  input?.focus();
  toEnd();
};
// TRY-22: Tab fills one of the newest answer's suggested follow-ups (not while it works: those were for an older one)
const followUps = createMemo((): string[] => (busyNow() ? [] : suggestedFollowUps(lastTurn()?.presentation?.blocks)));
const ghost = (): { rest: string; key: string; full: string } | undefined => {
  const full = completionOf(text(), followUps());
  return full ? { rest: full.slice(text().length), key: t('shell.reply.tab'), full } : undefined;
};
const attach = (o: PickOptions): void => {
  setPicking(true);
  void shell.platform.pickFiles(o).then((picked) => {
    setPicking(false);
    if (picked.length) att.add(picked);
  });
};
  return {
    text, setText, input: (): HTMLTextAreaElement | undefined => input, setInput: (el: HTMLTextAreaElement): void => void (input = el),
    goalInput: (): HTMLInputElement | undefined => goalInput, setGoalInput: (el: HTMLInputElement): void => void (goalInput = el),
    att, files, attach, picking, menu, replyBox, mic, replyKeys, ghost, working, queueOn,
    team, setTeam, replyPreset, setReplyPreset, replyDelegateOpen, setReplyDelegateOpen,
    goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, setReplyGoal, goals, declared, goalGoing, liveGoalText, editLiveGoal,
    when, setWhen, replyRun, setReplyRun, defaultName, runnerOpen, setPickingRunner,
    send, sendConfirmed, sendWaiting, hold, setHold, checking, reply, draft,
  };
}
