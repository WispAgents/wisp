// The reply box's Send (TaskView.tsx): what a reply turns into before it goes. /feedback and /bug to the wisp team,
// /btw a side question, /side a side discussion; then the goal gate, the hold near a usage limit (v1 hold.js,
// ADR-0091), plugins' prompt sources (comments on lines, an @crew member, Send later), a setup card's answer, and
// last the follow-up itself, its files uploaded first.
import { createSignal, type Accessor } from 'solid-js';
import { entityKey, newRowId, uuidv7, type EntityKey, type PeerId } from '@wisp/client';
import type { SendHold } from '../contract.ts';
import { goalGate, goalWire, type DeclaredGoal } from './goal-gate.ts';
import { isFeedback } from './feedback.ts';
import { remember, sentStore } from './prompt-keys.ts';
import { btw, btwQuestion, sideAsk, sideStart, followUp, answerSetup, setupCardOf, type Attachment, restore, setGoal } from './commands.ts';
import { runPick } from './runner.ts';
import { whenAt, type When } from './pickers.ts';
import { teamToSend, type Delegate } from './team.ts';
import type { useShell } from '../context.tsx';
import type { usePluginClient } from '../slot.tsx';
import type { SessionRow } from '../rows.ts';
import type { useAttachments } from './attach.ts';
import type { usePromptMenu } from './prompt-menu.ts';
import type { TurnRow } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What Send reads of the reply box and the task, and what it clears (TaskView.tsx). */
export interface ReplySendCtx {
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
  readonly text: () => string;
  readonly setText: (v: string) => void;
  readonly input: () => HTMLTextAreaElement | undefined;
  readonly menu: ReturnType<typeof usePromptMenu>;
  readonly sent: ReturnType<typeof sentStore>;
  readonly att: ReturnType<typeof useAttachments>;
  readonly files: ReturnType<typeof useAttachments>['files'];
  readonly goal: Accessor<string | undefined>;
  readonly setGoalText: (v: string | undefined) => void;
  readonly pickedReplyGoal: Accessor<DeclaredGoal | undefined>;
  readonly setPickedReplyGoal: (g: DeclaredGoal | undefined) => void;
  readonly setReplyGoal: () => void;
  readonly goalInput: () => HTMLInputElement | undefined;
  readonly when: Accessor<When | undefined>;
  readonly setWhen: (w: When | undefined) => void;
  readonly picked: Accessor<readonly Delegate[] | undefined>;
  readonly teamChanged: Accessor<boolean>;
  readonly setTeamChanged: (changed: boolean) => void;
  readonly runOf: () => ReturnType<typeof runPick>;
  readonly queues: () => boolean;
  readonly toEnd: () => void;
}

/** Send, and the hold row's state: its Send anyway (`sendConfirmed`) and Wait until the reset (`sendWaiting`). */
export function useReplySend(c: ReplySendCtx): {
  readonly send: () => void;
  readonly sendConfirmed: () => void;
  readonly sendWaiting: (until: number | undefined) => void;
  readonly hold: Accessor<SendHold | undefined>;
  readonly setHold: (h: SendHold | undefined) => void;
  readonly checking: Accessor<boolean>;
} {
  const { shell, t, key, bare, peer, shellHome, run, clients, row, turns, text, setText, input, menu, sent, att, files } = c;
  const { goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, setReplyGoal, goalInput, when, setWhen, picked, teamChanged, setTeamChanged, runOf, queues, toEnd } = c;
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
      // taken: its mentions go with it, the next prompt starts with none (doc25a); refused, they come back with it
      const stored = menu.encode(sq);
      setText('');
      menu.reset();
      const sideId = entityKey(peer, newRowId());
      const at = Date.now();
      const open = (attachments: readonly Attachment[]): void => {
        run(sideStart, { id: sideId, main: key, text: stored, at, ...(attachments.length > 0 && { attachments }) }).catch(() => text() === '' && menu.restore(`/side ${stored}`));
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
      goalInput()?.focus();
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
      void holds
        .check({ peer, agent, session: bare })
        .then(
          (h) => {
            setChecking(false);
            if (h) return void (setHold(h), input()?.focus());
            pass = 'checked';
            send();
          },
          () => setChecking(false),
        );
      return;
    }
    // what plugins send with it, at its top (comments on lines): the reply goes even with nothing typed
    const extra = shell
      .promptSources()()
      .filter((p) => (p.where ?? ['reply']).includes('reply') && p.pending)
      .map((p) => p.pending?.(v, { client: clients.client((p as { plugin?: string }).plugin ?? 'shell'), task: key }))
      .find((x) => x !== undefined);
    // a time picked: only Send later's source keeps it, and its follow-up carries no files or comments yet, so with
    // them it's refused with the box, its files, its comments and the clock as they were; sending now would ignore it
    if (sendAt !== undefined && (extra || files().length > 0)) {
      shell.toast.error(t('shell.when.noFilesReply'));
      return;
    }
    menu.reset();
    if (!v && !extra && files().length === 0) {
      // nothing to send but Done when filled: the goal is set on its own (it did nothing before)
      if (goal()?.trim() && sendAt === undefined) setReplyGoal();
      return;
    }
    // a plugin's prompt source may take it (@a crew member: their job, their answer under this turn)
    const pc = shell.client.plugin('shell');
    if (
      !extra &&
      files().length === 0 &&
      shell
        .promptSources()()
        .some((p) => (p.where ?? ['reply']).includes('reply') && p.send?.(v, { client: pc, task: key, ...(sendAt !== undefined && { launch: { at: sendAt } }), restore: (back) => !text().trim() && menu.restore(back) }) === true)
    ) {
      setText('');
      setWhen(undefined);
      return;
    }
    // no source kept the time (Send later's plugin off or still away): never sent now in its place
    if (sendAt !== undefined) {
      shell.toast.error(t('shell.when.noLater'));
      return;
    }
    // under a setup card, words answer it (ADR-0113 rule 4: Allow runs it, else the Deny with them), never a follow-up
    const card = setupCardOf(row());
    if (card && v && !extra && files().length === 0) {
      setText('');
      run(answerSetup, { id: key, text: v, askId: card }).catch(() => menu.restore(v));
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
      toEnd();
      run(followUp, {
        id: key,
        text: extra ? extra.text : v,
        turnId,
        idx,
        at,
        ...(attachments.length > 0 && { attachments }),
        ...(runOf() ? { run: runOf() as NonNullable<ReturnType<typeof runOf>> } : {}),
        queue: queues(),
        ...(through === 'confirmed' && { confirmLimit: true }),
        ...(sentTeam !== undefined && { team: teamToSend(sentTeam) }),
      }).catch((err: unknown) => {
        extra?.back();
        menu.restore(v);
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
  const sendConfirmed = (): void => {
    pass = 'confirmed';
    send();
  };
  const sendWaiting = (until: number | undefined): void => {
    waitUntil = until;
    send();
  };
  return { send, sendConfirmed, sendWaiting, hold, setHold, checking };
}
