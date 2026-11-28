// The reply box's tools (TaskView.tsx, UX 2026-11-18 #3): `+` for what's off (attach, Goal…, Delegate…, Send later),
// a chip with ✕ for each that's on (the goal, the team, the time), and the Delegate panel on its chip.
import '../i18n/lazy.ts';
import '../i18n/home.ts';
import { createMemo, lazy, on, type Accessor } from 'solid-js';
import type { MenuEntry, Act, LauncherTool } from '@wisp/ui';
import { startsAt, whenTip } from './hold.ts';
import { sameEntries } from './same-entries.ts';
import { projectOfRow } from './rows-view.ts';
import { goalWire, type DeclaredGoal } from './goal-gate.ts';
import { attachAct } from './capture.ts';
import { runWords, type Runner, type useDefaultWords } from './runner.ts';
import { whenAt, whenMenu, type When } from './pickers.ts';
import { teamSize, type Delegate } from './team.ts';
import { plusTool } from './composer-plus.ts';
import type { PeerId } from '@wisp/client';
import type { useShell } from '../context.tsx';
import type { usePluginClient } from '../slot.tsx';
import type { SessionRow } from '../rows.ts';
import type { PickOptions } from '../platform/types.ts';
import type { useProjects } from './projects.ts';
import type { useGoalKit } from './goal-kit.ts';
import { useHere } from './use-here.ts';

// Delegate's panel: its own chunk, loaded when it first opens (ADR-0011)
const DelegatePanel = lazy(() => import('./DelegatePanel.tsx'));

type Shell = ReturnType<typeof useShell>;

/** What the reply box's tools read of the task and the box, and what they set (TaskView.tsx). */
export interface ReplyToolsCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly peer: PeerId;
  readonly shellHome: ReturnType<typeof usePluginClient>['home'];
  readonly s: () => SessionRow;
  readonly row: () => SessionRow | undefined;
  readonly sessions: () => readonly SessionRow[];
  readonly projects: ReturnType<typeof useProjects>['list'];
  readonly now: Accessor<number>;
  /** the reply's harness as the box names it, and the run it takes */
  readonly h: () => { name: string; logo?: string };
  readonly shownRun: () => Runner;
  readonly replyRun: () => Runner | undefined;
  readonly defaultName: ReturnType<typeof useDefaultWords>;
  readonly replyBox: { readonly focus: () => void };
  readonly attach: (o: PickOptions) => void;
  readonly picking: Accessor<boolean>;
  readonly goals: ReturnType<typeof useGoalKit>;
  readonly declared: () => readonly DeclaredGoal[];
  readonly goal: Accessor<string | undefined>;
  readonly setGoalText: (v: string | undefined) => void;
  readonly pickedReplyGoal: Accessor<DeclaredGoal | undefined>;
  readonly setPickedReplyGoal: (g: DeclaredGoal | undefined) => void;
  readonly goalGoing: () => boolean;
  readonly liveGoalText: () => string;
  readonly editLiveGoal: () => void;
  readonly goalInput: () => HTMLInputElement | undefined;
  readonly team: () => readonly Delegate[];
  readonly setTeam: (next: readonly Delegate[]) => void;
  readonly replyPreset: Accessor<string>;
  readonly setReplyPreset: (v: string) => void;
  readonly replyDelegateOpen: Accessor<boolean>;
  readonly setReplyDelegateOpen: (open: boolean) => void;
  readonly when: Accessor<When | undefined>;
  readonly setWhen: (w: When | undefined) => void;
}

/** The Delegate panel's act, the `+` (its entries kept while they say the same) and the chips of what's on. */
export function useReplyTools(c: ReplyToolsCtx): { readonly replyTeamAct: () => Act; readonly replyPlusNow: Accessor<LauncherTool>; readonly replyChips: () => LauncherTool[] } {
  const { shell, t, peer, shellHome, s, row, sessions, projects, now, h, shownRun, replyRun, defaultName, replyBox, attach, picking, goals, declared } = c;
  const here = useHere();
  const { goal, setGoalText, pickedReplyGoal, setPickedReplyGoal, goalGoing, liveGoalText, editLiveGoal, goalInput, team, setTeam, replyPreset, setReplyPreset } = c;
  const { replyDelegateOpen, setReplyDelegateOpen, when, setWhen } = c;
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
            place: peer === shellHome ? { icon: here.icon(), label: here.name(), title: t('shell.home.where') } : { icon: 'server', label: shell.client.peers.name(peer), title: t('shell.home.where') },
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
  const newReplyGoal = (): void => (setPickedReplyGoal(undefined), setGoalText(''), queueMicrotask(() => goalInput()?.focus()));
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
        [!when() && { id: 'later', part: 'composer.sendLater', icon: 'clock', label: t('shell.home.later'), act: { menu: { title: t('shell.home.later'), items: whenMenu(t, now()), onSelect: (w) => setWhen(w as When) } } }],
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
              label:
                typeof named === 'object'
                  ? t('shell.home.goalChip', { text: named.name })
                  : text
                    ? t('shell.home.goalChip', { text })
                    : goal() === undefined && liveGoalText()
                      ? t('shell.home.goalChip', { text: liveGoalText() })
                      : t('shell.home.goal'),
              title: goal() === undefined && liveGoalText() ? t('shell.goal.editLive', { text: liveGoalText() }) : t('shell.task.goal'),
              text: true,
              on: true,
              act: { onClick: () => (goal() === undefined && goalGoing() ? editLiveGoal() : (goal() === undefined && setGoalText(''), queueMicrotask(() => goalInput()?.focus()))) },
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
              act: { menu: { title: t('shell.home.later'), items: whenMenu(t, now()), onSelect: (w: string) => setWhen(w as When) } },
              clear: { label: t('shell.when.clear'), act: { onClick: () => setWhen(undefined) } },
            },
          ]
        : []),
    ];
  };
  return { replyTeamAct, replyPlusNow, replyChips };
}
