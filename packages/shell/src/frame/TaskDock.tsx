// The open task's dock as v1 draws it (TaskView.tsx; ship.css .overlap-strip, composer.css): the overlap note, what
// waits on you (`shell.task:foot`), the reply box with its mode, harness, `+`, chips, goal line, hold row and hints,
// and the harness picker.
import '../i18n/lazy.ts';
import '../i18n/home.ts';
import { Show, createMemo, on, type JSX } from 'solid-js';
import { OverlapNote, ReplyBox, type ReplyCap, type Act, type LauncherTool } from '@wisp/ui';
import { usePlanRing } from './plan-ring.ts';
import { aheadLabel, startsAt, waitable } from './hold.ts';
import { modelWords } from './run-words.ts';
import { Slot } from '../slot.tsx';
import { sameEntries } from './same-entries.ts';
import { overlapLine, overlapsOf } from './rows-view.ts';
import { replyPlaceholderKey } from './reply-hint.ts';
import { factoryFoot, factoryReply, runFactory } from './run-factory.ts';
import { factoryStopQuestion } from './stop-question.ts';
import { dismiss, shownOverlaps, type Dismissed } from './overlap-dismiss.ts';
import { setSetting, setMode } from './commands.ts';
import { attachAct } from './capture.ts';
import { RunnerPicker } from './RunnerPicker.tsx';
import { runWords, taskRunner, type Runner } from './runner.ts';
import { MODE_ICON, modeMenu, whenAt, whenMenu, type Mode, type When } from './pickers.ts';
import { useReplyTools } from './reply-tools.tsx';
import { capOf } from './cap.ts';
import type { TaskPage } from './task-page.ts';
import type { SessionRow } from '../rows.ts';

/** The dock under the turns, drawn once the page's body is built. */
export function TaskDock(props: { readonly p: TaskPage; readonly s: () => SessionRow }): JSX.Element {
  const {
    shell, t, key, built, row, sessions, titleOf, closed, closedReady, closeNote, peer, run, now, hint, chat, phone,
    harnessView, shellHome, projects, lastTurn, setStopAsk, more,
  } = props.p;
  const {
    text, setText, input, setInput, goalInput, setGoalInput, att, files, attach, picking, menu, replyBox, mic,
    replyKeys, ghost, working, queueOn, team, setTeam, replyPreset, setReplyPreset, replyDelegateOpen,
    setReplyDelegateOpen, goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, setReplyGoal, goals, declared,
    goalGoing, liveGoalText, editLiveGoal, when, setWhen, replyRun, setReplyRun, defaultName, runnerOpen,
    setPickingRunner, send, sendConfirmed, sendWaiting, hold, setHold, checking,
  } = props.p.r;
  const s = props.s;
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
  // the reply box's `+`, its chips and the Delegate panel (reply-tools.tsx)
  const { replyTeamAct, replyPlusNow, replyChips } = useReplyTools({
    shell, t, peer, shellHome, s, row, sessions, projects, now, h, shownRun, replyRun, defaultName, replyBox, attach, picking, goals, declared,
    goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, goalGoing, liveGoalText, editLiveGoal, goalInput, team, setTeam,
    replyPreset, setReplyPreset, replyDelegateOpen, setReplyDelegateOpen, when, setWhen,
  });
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
  return (
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
            onGo: sendConfirmed,
            ...(waitable(hd.until, Date.now()) && { wait: { label: t('shell.hold.wait', { at: aheadLabel(hd.until ?? 0, Date.now()) }), title: t('shell.hold.waitTitle') } }),
            onWait: () => sendWaiting(hd.until),
            onKeep: () => (setHold(undefined), input()?.focus()),
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
        sendShortcut={chat.sendShortcut()}
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
        inputRef={setInput}
        attach={t('shell.home.attach')}
        attachAct={attachAct(shell, attach, picking())}
        mic={mic ? { label: mic.label(), on: mic.on(), act: { onClick: mic.toggle } } : undefined}
        mode={{
          label: t(`shell.mode.${mode()}`),
          title: t('shell.home.fullAutoTitle'),
          icon: MODE_ICON[mode()],
          act: { menu: { title: t('shell.mode.sheet'), items: modeItems(), onSelect: (m) => void run(setMode, { id: key, mode: m }).catch(() => undefined) } },
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
        goalAct={{
          onClick: () =>
            goal() === undefined && goalGoing() ? editLiveGoal() : (setGoalText(goal() === undefined ? '' : undefined), goal() !== undefined && queueMicrotask(() => goalInput()?.focus())),
        }}
        {...(goal() !== undefined && {
          goalLine: {
            label: t('shell.home.doneWhen'),
            placeholder: t('shell.home.goalPlaceholder'),
            // v1 .lg-keys: what Enter does, the message's or the goal's own
            keys: text().trim() ? t('shell.goal.withMessage') : t('shell.home.goalKeys'),
            value: goal() ?? '',
            onInput: setGoalText,
            // Enter: with the message if one is typed (v1 goal.js clicks Send), else the goal alone
            onEnter: () => (text().trim() || files().length > 0 ? send() : setReplyGoal()),
            onEscape: () => (setGoalText(undefined), setPickedReplyGoal(undefined), replyBox.focus()),
            set: { label: t('shell.goal.set'), title: t('shell.goal.setTitle'), onClick: setReplyGoal },
            ref: setGoalInput,
          },
        })}
        later={t('shell.home.later')}
        laterAct={{ menu: { title: t('shell.home.later'), items: whenMenu(t, now()), onSelect: (w) => setWhen(w as When) } }}
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
        back={input}
        peer={peer}
        defaultWord={(a) => defaultName(a, '')}
        value={shownRun()}
        onChange={setReplyRun}
      />
    </Show>
  );
}
