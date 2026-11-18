// Home as v1 shows it (web/js/features/split.js renderHome, launcher.js, tips.js): the greeting, the
// prompt box, what needs you (up to five), the tips. Every control answers at once
// (design/feedback.md): a pick changes its label, a toggle flips, a menu opens, and sending shows the
// new task's row and page straight away (Starting…), the box cleared; a refusal puts the text back
// and the client toasts why.

import { useProjects } from './projects.ts';
import { usePromptMenu } from './prompt-menu.ts';
import { useAttachments } from './attach.ts';
import { attachAct } from './capture.ts';
import { useMic } from './dictate.ts';
import { warm, warmer } from './warm.ts';
import { INSTALLABLE, useSetup } from './setup.ts';
import { useRecipes } from './recipes.ts';
import type { PickOptions } from '../platform/types.ts';
import { Show, createEffect, createMemo, createResource, createSignal, lazy, on, onCleanup, untrack, type JSX } from 'solid-js';
import { githubTip, tools as githubTools } from './github-tip.ts';
import { tipPick } from './home-tips.ts';
import { computed, entityKey, newRowId, ownerOf, type EntityKey, type Json, type PeerId } from '@wisp/client';
import { KeysHint, Launcher, NextList, Reserved, SplitHome, Tips, lazyPart, lazyView, useSignal, warm as warmPart, type Act, type LauncherProps, type LauncherTool, type PlainTip, type SetupTip } from '@wisp/ui';
import { useShell } from '../context.tsx';
import type { LaunchTargetDef, SendHold } from '../contract.ts';
import { aheadLabel, startsAt, waitable, whenTip } from './hold.ts';
import { Slot, useExtensions, usePluginClient } from '../slot.tsx';
import { useNow } from '../clock.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { longTime, needsYou, shortAgo, statusOf, sessionKey } from './rows-view.ts';
import { autoBlocked, autoPreview, autoRules, create, setSetting, trial, type Attachment } from './commands.ts';
import { newRun, runWords, sameSetup, settingModel, trialVary, variantWords, type Variant } from './trial.ts';
import { modelWord, useHarnessRows, type Runner, useDefaultWords } from './runner.ts';
import { usePlanRing } from './plan-ring.ts';
import { AUTO_SETTING, LAUNCH_SETTING, autoEnabled, readLaunch, startRunner, withPick, type LaunchPick } from './launch-pick.ts';
import { goalGate, goalOf, goalSent, isBareGoal, typedInFull, type HomeGoal } from './goal-gate.ts';
import { isFeedback } from './feedback.ts';
import { autoPick, neverRule, runnerParts, runnerPick, overrideOf, PREVIEW_DEBOUNCE, type AutoPreview } from './auto.ts';
// what will Auto run (v1 openPop): its own chunk, drawn only when Auto's pick is pressed
const HomeAutoPop = lazy(() => import('./home-auto-pop.tsx'));
import { defaultHarness, harnessOf } from './harness.ts';
import { MAX_RUNNERS, newGroup, nextRun } from './runs-small.ts';
import { barMode, firstRunTools, launcherFull, type BarMode } from './first-run.ts';
import { sandboxPeers, targetValue, whereItems } from './where.ts';
import { placeOn } from './run-on.ts';
import { dirsOf, launchOf, parsePick, pickLabel, pickedProjects, placePick } from './project-pick.ts';
import { openPlaces, releasePlaces, type PlaceTarget } from './project-switch.ts';
import { promptKeys, remember, sentStore } from './prompt-keys.ts';
import { displayCombo } from '../keys.ts';
import { draftValue, launcherDraft, withShareLink } from './drafts.ts';
import { MODE_ICON, agentName, modeMenu, whenAt, whenMenu, type Mode, type When } from './pickers.ts';
import { newRecipeIntent } from './recipe-new.ts';
import { foldTail } from './phone-bar.ts';
import { plusTool } from './composer-plus.ts';
import { TEAMS_SETTING, presetsOf, teamFromSetting, teamSize, teamToSend, type Delegate } from './team.ts';
import type { DelegatePlace } from './DelegatePanel.tsx';

// Delegate's panel: its own chunk, loaded when it first opens (ADR-0011)
const DelegatePanel = lazy(() => import('./DelegatePanel.tsx'));

// the harness picker opens on a press: it loads apart from the first page (ADR-0011, ADR-0067), as Home first draws,
// with the controls once the page is idle (preloadControls) and on the pill's first hover or focus, so the first press
// finds it there and it shows within the frame
const pickerPart = lazyPart(() => import('./RunnerPicker.tsx'));
// New task's bar in the phone apps (ADR-0065 step 13): in the phone heads' chunk, so the browser never downloads it
const PhoneCompose = lazy(() => import('./PhoneHead.tsx').then((m) => ({ default: m.PhoneCompose })));
const RunnerPicker = lazyView(pickerPart, (m) => m.RunnerPicker);
const warmPicker = warmPart(pickerPart);

interface SettingRow {
  readonly key: string;
  readonly value: unknown;
}

const TIPS = ['goal1', 'goal2', 'vs', 'compare', 'auto', 'limit'] as const;
/** the tips about Auto: shown only while its switch is on (owner #37) */
const AUTO_TIPS: ReadonlySet<string> = new Set(['auto', 'limit']);
const GUIDE = 'https://trywisp.dev/guides';

/** New task's Worktree / In place default (wisp.worktrees declares it). */
const WORKTREE_KEY = 'worktrees.worktree';

export function HomeView(props: { readonly onBack?: () => void }): JSX.Element {
  const shell = useShell();
  // the phone apps (Android, iPhone): New task's own bar; set once, as the platform never changes
  const phone = shell.platform.phone;
  // the phone apps' New task sheet / page holds the prompt box alone (r291): no greeting (its bar says New task), no
  // Needs you, cards, tips, keys or feedback line under it; the browser at a phone's width keeps v1's whole Home
  const sheet = (): boolean => phone && props.onBack !== undefined;
  const t = shell.t;
  const home = usePluginClient('shell').home;
  // gh signed in or not, as Settings → Agents' GitHub row says it: the Sign in to GitHub tip waits for the answer
  const [ghTools] = createResource(() => shell.client.commands.run(githubTools, { peer: home }).catch(() => null));
  // Run on (v1 launch.where): this computer, or another one you added (machines); only one at a time (F-comp-10)
  const peerIds = useSignal(shell.client.peers.list());
  const [runOn, setRunOn] = createSignal<string>(home);
  const target = (): PeerId => (peerIds().includes(runOn() as PeerId) ? runOn() : home) as PeerId;
  // a sandbox is the task's that made it, never a computer to run on (where.ts); plugins' places come after (TRY-19)
  const keptMachines = useSignal(shell.client.entities.list<{ id: unknown; body?: { data?: unknown } }>('machines_peer'));
  const runOnPeers = (): readonly string[] => {
    const boxes = sandboxPeers(keptMachines());
    return peerIds().filter((p) => !boxes.has(p));
  };
  const launchTargets = useSignal(shell.launchTargets());
  const targets = (): readonly LaunchTargetDef[] => launchTargets().filter((d) => d.offered?.(shell.client.plugin('shell')) ?? true);
  const placeOf = (): LaunchTargetDef | undefined => targets().find((d) => targetValue(d.id) === runOn());
  const sessions = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  const { list: projects } = useProjects(shell, usePluginClient('shell').home);
  const settings = useSignal(shell.client.entities.list<SettingRow>('setting'));
  const now = useNow(shell.client);
  const harnesses = useSignal(shell.harnesses());
  // what a harness's Default runs, by name, when its config says (UX 2026-11-18 #9: "Claude Code · Opus")
  // in the picked project: what it pins, else this computer's (owner's delegate 2026-11-18)
  const defaultName = useDefaultWords(shell.client, () => target(), () => project()?.path);
  /** what Auto runs today: the default harness (wisp's router picks per task once it's there) */
  const auto = (): string => defaultHarness(harnesses()) ?? '';
  // New task's text is a draft (kind 5): kept across reloads and updates (v1 F-task-32), under `<home>:launcher` → text,
  // where wisp.stuff's share-in puts what was shared to wisp
  const [draftKey, draftField] = launcherDraft(home);
  const draft = useSignal(shell.client.drafts.get<string>(draftKey, draftField));
  const text = (): string => draft() ?? '';
  const setText = (v: string): void => shell.client.drafts.set(draftKey, draftField, draftValue(v));
  // a share link the page was opened with (the Grid's share page: docs/ux/2026-11-18-grid.md #8): into the box as text,
  // once; the person presses Send, nothing is sent for them
  {
    const link = shell.client.nav.takeShare?.();
    if (link) setText(withShareLink(untrack(draft) ?? '', link));
  }
  const [mode, setMode] = createSignal<Mode>('auto');
  // the first frame starts on what's kept already, never on Auto while it's off (owner #36, #37)
  const first = untrack(() =>
    startRunner(
      readLaunch(settings().find((r) => r.key === LAUNCH_SETTING)?.value),
      harnesses().map((h) => h.id),
      autoEnabled(settings().find((r) => r.key === AUTO_SETTING)?.value),
    ),
  );
  const [agent, setAgent] = createSignal(first.agent);
  // the model and thinking level picked for the harness (Default: '')
  const [model, setModel] = createSignal(first.model);
  // the model's name for the pill (v1 .rb-m shows the label, never the id)
  const [modelLabel, setModelLabel] = createSignal(first.modelLabel);
  const [effort, setEffort] = createSignal(first.effort);
  // the account picked for the harness ('' : Auto, your saved one) and its name for the bar (F-acct-3)
  const [account, setAccount] = createSignal<{ id: string; label: string } | undefined>();
  // UX 2026-11-18 #8: the picked harness's usage ring on its chip (where Home's usage bar was; plan-ring.ts)
  const ring = usePlanRing(shell, { peer: () => target(), agent, account: () => account()?.id ?? '' });
  // Auto's one switch (`runtime.auto`, default off: owner #37) and New task's kept pick (`runtime.launch`, owner #36:
  // the pick was a plain signal, so a page change or a reload put it back on Auto)
  const autoOn = (): boolean => autoEnabled(settings().find((r) => r.key === AUTO_SETTING)?.value);
  const kept = (): LaunchPick => readLaunch(settings().find((r) => r.key === LAUNCH_SETTING)?.value);
  const harnessRows = useHarnessRows(shell.client.entities, () => home);
  const readyIds = (): string[] => {
    const found = harnessRows()
      .filter((r) => r.state === 'installed')
      .sort((a, b) => a.ord - b.ord)
      .map((r) => r.id);
    return found.length ? found : harnesses().map((h) => h.id);
  };
  // until you pick, New task follows what's kept (it lands after the first frame, as do the harnesses)
  let runnerTouched = false;
  createEffect(() => {
    const r = startRunner(kept(), readyIds(), autoOn());
    if (runnerTouched) return;
    setAgent(r.agent);
    setModel(r.model);
    setModelLabel(r.modelLabel);
    setEffort(r.effort);
  });
  const keepPick = (a: string, m: string, label: string, e: string): void => {
    runnerTouched = true;
    void shell.client.commands.run(setSetting, { peer: home, key: LAUNCH_SETTING, value: withPick(kept(), a, m, label, e) as unknown as Json }).catch(() => undefined);
  };
  const pickAgent = (a: string): void => {
    // the harness's own model and level, as it was last picked (v1 savedModel / savedEffort)
    const r = startRunner({ ...kept(), agent: a }, [a], false);
    setAgent(a);
    setModel(r.model);
    setModelLabel(r.modelLabel);
    setEffort(r.effort);
    setAccount(undefined);
    keepPick(a, r.model, r.modelLabel, r.effort);
  };
  const [picker, setPicker] = createSignal(false);
  // What Auto will run (v1 auto-preview.js openPop): from the Auto pick while Auto picks
  const [autoPop, setAutoPop] = createSignal(false);
  // v1 modelLabel: the model alone (what a Never rule acts on), or "<harness>'s default model"
  const neverModel = (pk: { agent: string; model: string; label: string }): string =>
    pk.model ? (pk.label.split(' · ')[1] ?? pk.model) : t('shell.auto.defaultModel', { name: agentName(t, harnesses(), pk.agent) });
  const typeWord = (type: string): string => (type === 'ui' ? 'UI' : type);
  // Never pick this model for this kind of task: said at once with Undo, the toast says why if the peer refuses
  const never = async (pk: { agent: string; model: string; label: string }, type: string): Promise<void> => {
    const rule = neverRule(pk, type);
    const now = await shell.client.commands.run(autoRules, { peer: home }).then(
      (v) => v.settings?.blocked ?? [],
      () => [] as string[],
    );
    if (now.includes(rule)) return;
    const set = (list: readonly string[]): Promise<unknown> => shell.client.commands.run(autoBlocked, { peer: home, blocked: list }).then(() => ask(text()));
    shell.toast.show(t('shell.auto.wont', { model: neverModel(pk), kind: typeWord(type) }), { action: { label: t('shell.auto.undo'), onClick: () => void set(now).catch(() => undefined) } });
    await set([...now, rule]).catch(() => undefined);
  };
  // Until your first task the box is the prompt, how it runs and Send; the rest waits behind More options (v1
  // launcher.js firstRun). Once opened, or once there's a task of yours, it stays full (kept in this browser).
  const fullStore = launcherFull(shell.client);
  const full = useSignal(fullStore.value);
  const [ready, setReady] = createSignal(false);
  void fullStore.ready.then(() => setReady(true));
  // a task of yours on this computer (v1 counted its own server's list, the one the first run waits for): another
  // computer's tasks, synced later, would flip the bar when they land
  const sessionKeys = useSignal(shell.client.entities.keys(SESSION));
  const mine = createMemo(() => (sessions(), sessionKeys().some((k) => ownerOf(k) === home && shell.client.entities.peek<SessionRow>(SESSION, k)?.agent !== 'wisp')));
  createEffect(() => {
    if (mine() && full() !== true) fullStore.set(true);
  });
  // the task list in whole: an empty one is then really empty, not still coming
  const loaded = useSignal(shell.client.entities.loaded(home, 'list'));
  const barNow = createMemo<BarMode>((drawn) => barMode(drawn, { full: full() === true, mine: mine(), ready: ready(), loaded: loaded() }), 'full');
  const firstRun = (): boolean => barNow() === 'first';
  const openFull = (): void => {
    fullStore.set(true);
    input?.focus();
  };
  // vs (v1 launcher.js compare): the same task on more harnesses, side by side; picks here don't touch the defaults
  // each its own harness, model, thinking and account, picked in the same harness picker as the lead (v1 runnerPicker)
  const [runs, setRuns] = createSignal<readonly Runner[]>([]);
  // the run whose harness picker is open (its place in runs), if any
  const [cmpPicker, setCmpPicker] = createSignal<number | undefined>();
  // Try it two ways (v1 #650, #771): the one run added with + on the lead's harness may change one setting
  const [vary, setVary] = createSignal<Variant | undefined>();
  const subSetting = (): string => settingModel(settings().find((r) => r.key === 'runtime.subagentModel')?.value);
  /** What's different: only for a lone copy of the lead (its harness, model and thinking: one setting changes) on a
   * harness that runs variants */
  const varies = (r: Runner, i: number): readonly string[] =>
    // each run needs a worktree of its own: no project, no trial (the launch would be a plain compare)
    i === 0 && runs().length === 1 && sameSetup(r, { agent: agent(), model: model(), effort: effort() }) && project() ? (harnesses().find((h) => h.id === r.agent)?.variants ?? []) : [];
  const [team, setTeam] = createSignal<readonly Delegate[]>([]);
  // the team new tasks start with (Settings, `runtime.subagents`): pre-filled until you pick your own
  let teamTouched = false;
  const subagents = (): unknown => settings().find((r) => r.key === 'runtime.subagents')?.value;
  createEffect(() => {
    const pre = teamFromSetting(subagents());
    if (!teamTouched) setTeam(pre);
  });
  // the preset the team came from (v1 teamPreset): its name is the button's
  const [preset, setPreset] = createSignal('');
  const presetName = (): string | undefined => (preset() ? presetsOf(settings().find((r) => r.key === TEAMS_SETTING)?.value).find((p) => p.id === preset())?.name : undefined);
  const pickTeam = (next: readonly Delegate[]): void => {
    teamTouched = true;
    setTeam(next);
  };
  // Goal: off for every new prompt, Done when's text a draft (v1 launcherGoal: `home-goal`, kept over a reload)
  const [goalOn, setGoalOn] = createSignal(false);
  const goalDraft = useSignal(shell.client.drafts.get<string>(draftKey, 'goal'));
  const homeGoal = (): HomeGoal => ({ on: goalOn(), text: goalDraft() ?? '' });
  const putGoal = (g: HomeGoal): void => {
    setGoalOn(g.on);
    shell.client.drafts.set(draftKey, 'goal', draftValue(g.text));
  };
  const goal = (): string | undefined => goalOf(homeGoal());
  const setGoal = (v: string | undefined): void => {
    if (v === undefined) setGoalOn(false);
    else putGoal({ on: true, text: v });
  };
  const [when, setWhen] = createSignal<When | undefined>(undefined);
  // files with the new task: picked, pasted or dropped, each uploading at once to this computer
  const att = useAttachments(shell, () => home);
  const files = att.files;
  const [picking, setPicking] = createSignal(false);
  const [where, setWhere] = createSignal<string | undefined>(undefined); // a project id, 'none', or the first project
  const [tipAt, setTipAt] = createSignal(0);
  // what Auto would run for the prompt (v1 auto-preview.js): asked after a pause in typing, the newest answer wins;
  // kept when you then pick your own harness, so the task tells Auto what it had shown (autoOverride)
  const [preview, setPreview] = createSignal<AutoPreview | undefined>(undefined);
  let asked = 0;
  const scope = shell.client.scope.child('home');
  onCleanup(() => scope.dispose());
  let cancel: (() => void) | undefined;
  const ask = (v: string, now = false): void => {
    cancel?.();
    if (agent() !== 'auto') return;
    const n = ++asked;
    const go = (): void =>
      void shell.client.commands
        .run(autoPreview, {
          peer: home,
          prompt: v.trim(), // empty too: v1 shows Auto's pick before you type
          ...(files().length > 0 && { attachments: files().length }),
          // Auto's goal (balanced, cheap, best), not Done when's condition: the router reads it as the goal it picks for
          ...(agent() === 'auto' && model() ? { goal: model() } : {}),
          ...(team().length > 0 && { team: true }),
        })
        .then((p) => n === asked && setPreview(p))
        .catch(() => undefined);
    // typing waits for a pause; the first ask goes at once (once setup is done: what it reads is declared below)
    if (now) {
      cancel = undefined;
      queueMicrotask(() => n === asked && go());
    } else cancel = scope.timeout(PREVIEW_DEBOUNCE, go);
  };
  ask('', true); // v1 refresh({ now: true }): Auto's pick shows from the start
  let input: HTMLTextAreaElement | undefined;
  onCleanup(shell.focusTargets.provide('launcher', () => input?.focus()));
  // a plugin may fill this prompt (Start from an issue): set it, the caret at its end
  const box = { text: () => text(), set: (v: string) => (setText(v), menu.typed(v)), focus: () => (input?.focus(), input?.setSelectionRange(input.value.length, input.value.length)) };
  if (shell.prompts) onCleanup(shell.prompts.provide('launcher', box));
  // v1 puts the caret in the prompt a beat after Home shows; never taken from what holds it already (a menu or a
  // dialog opened while Home was still loading kept its focus in v1, whose Home was there from the start)
  queueMicrotask(() => {
    const at = document.activeElement;
    // the phone sheet opens to write in: the caret is in the prompt (the keyboard comes up), whatever was pressed
    if (sheet() || !at || at === document.body) input?.focus();
  });
  // several projects picked over the list (project-pick.ts): New task starts in the first, and also works in the others
  // (runtime.create `dirs`, F-comp-47) until you leave one out with its ×; + on any other chip (or ⌘-click) adds it
  const listFilter = useSignal(shell.projectFilter?.value ?? (() => 'all'));
  const listPick = createMemo(() => launchOf(parsePick(listFilter()), projects()));
  const [ownAlso, setOwnAlso] = createSignal<readonly string[] | undefined>(undefined);
  // the pick over the list changed (its ids, not a new projects list: an added folder or a rename keeps what + added)
  createEffect(on(() => `${listPick().start ?? ''}|${listPick().also.join(',')}`, () => setOwnAlso(undefined), { defer: true }));
  const project = createMemo(() => (where() === 'none' ? undefined : (projects().find((p) => p.id === (where() ?? listPick().start)) ?? projects()[0])));
  const also = createMemo(() => (project() ? (ownAlso() ?? listPick().also).filter((id) => id !== project()?.id) : []));
  // wisp.worktrees' `worktrees.worktree` ('on' | 'off'), the same switch as Settings → General (B54: New task read and
  // wrote `defaults.worktree`, which no plugin declares, so its toggle never stuck and Settings' did nothing)
  const worktree = createMemo(() => !['off', false].includes(settings().find((r) => r.key === WORKTREE_KEY)?.value as never));
  // the usage row's room from last time (Reserved): it loads after the first screen and must not move Needs you
  const usageExt = useExtensions('shell.home:usage');
  // none attached (wisp.budget off): no room held for it
  const usageH = (): number => (usageExt().length ? usageSaved() : 0);
  const usageSaved = useSignal(() => Number(shell.client.settings.get('shell.home.usageHeight')() ?? 0));
  // the cards under Needs you (wisp.budget's Spare allowance) load after the first screen too: their room from last time
  const cardsExt = useExtensions('shell.home:cards');
  const cardsSaved = useSignal(() => Number(shell.client.settings.get('shell.home.cardsHeight')() ?? 0));
  const cardsH = (): number => (cardsExt().length ? cardsSaved() : 0);
  const needs = createMemo(() =>
    sessions()
      // the list's own rules (saved for later, Scheduled, a subagent's run: not here), so Home agrees with it
      .filter(needsYou)
      .sort((a, b) => b.updated_at - a.updated_at),
  );
  // the @ and / menus (plugins' prompt sources), New task's own
  const menu = usePromptMenu({ shell, where: 'home', text, setText, input: () => input, id: 'launcher-suggest', project: () => project()?.path });
  let goalInput: HTMLInputElement | undefined;
  const [promptFocused, setPromptFocused] = createSignal(false);
  // a phone or a touch-only screen has no shortcut to advertise (v1: MOBILE || (hover: none))
  const touchOnly = phone || (typeof matchMedia === 'function' && matchMedia('(hover: none)').matches);
  const promptHint = (): string =>
    phone ? t('shell.home.placeholderPhone') : promptFocused() || touchOnly ? t('shell.home.placeholder') : t('shell.home.placeholderKey', { key: displayCombo('mod+l', shell.platform.apple).join('') });
  // the goal plugin's Judges at the end of Done when (v1 #judgesBtn): made once, shown while the line is
  const goalEnd = <Slot input="shell.home:goalLine" layout="contents" />;
  // ↑ brings back what you sent, Shift+Tab cycles the mode (v1 prompt-history.js, F-task-31, F-comp-4)
  const sent = sentStore(shell.client);
  const sentList = useSignal(sent.value);
  const onKey = promptKeys({ sent: () => sentList() as readonly string[], value: text, set: (v) => (setText(v), menu.typed(v)), mode: { get: mode, set: setMode } });
  // a command typed in full (`/goal`, `/feedback`): the menu's Enter sends it rather than picking what's there
  const suggest = createMemo(() => {
    const s = menu.suggest();
    return s && { ...s, onPick: (v: string) => (typedInFull(v, text()) ? (s.onClose(), send()) : s.onPick(v)) };
  });
  // ADR-0067: plugins' prompt sources may still be on their way (the page's first moments, or a load tried again):
  // a send waits for them once, then goes whatever they did
  let waited = false;
  // Ask near the limit (v1 hold.js, ADR-0091): asked before the send goes; `pass` lets the next send() through once
  // checked ('checked'), after Start anyway ('confirmed': confirmLimit), or to wait for the reset ('wait': at)
  const [hold, setHold] = createSignal<SendHold | undefined>(undefined);
  const [checking, setChecking] = createSignal(false);
  let pass: { readonly kind: 'checked' | 'confirmed' } | { readonly kind: 'wait'; readonly at: number } | undefined;
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
    // /feedback … or /bug …: to the wisp team, never a task (v1 launcher.js)
    if (isFeedback(text())) {
      const typed = text();
      setText('');
      // sending it is its own chunk (feedback-send.ts)
      return void import('./feedback-send.ts').then((m) => m.feedbackCommand(shell, typed, { home, restore: () => text() === '' && setText(typed) })).catch(() => text() === '' && setText(typed));
    }
    // a bare /goal: Done when opens, to type it there (v1 box.goalOpen); `/goal <condition>` goes, the peer reads it
    if (isBareGoal(text())) {
      setText('');
      setGoalOn(true);
      queueMicrotask(() => goalInput?.focus());
      return;
    }
    const prompt = menu.encode(text().trim());
    if (prompt) sent.set((cur) => remember(cur as readonly string[], prompt));
    // Goal on and Done when empty: ask first, so a task meant to keep going doesn't start as a one-shot (v1 #769)
    const gate = goalGate(prompt, goal());
    if (gate === 'askGoal') {
      goalInput?.focus();
      shell.toast.show(t('shell.goal.empty'), { action: { label: t('shell.goal.sendWithout'), onClick: () => (setGoal(undefined), send()) } });
      return;
    }
    if (gate !== 'send') {
      if (gate === 'sayWhat') shell.toast.show(t('shell.goal.sayWhat'));
      input?.focus();
      return;
    }
    // asked before anything is taken (the goal, the menu, the box): a held send leaves New task as it was
    const through = pass;
    pass = undefined;
    setHold(undefined);
    const holds = shell.sendHolds;
    if (!through && holds && !when() && !prompt.startsWith('/')) {
      if (checking()) return;
      setChecking(true);
      // Auto: the harness its preview says it will run (the peer checks its own pick again)
      const lead = agent() === 'auto' ? (autoPick(preview())?.agent ?? '') : agent();
      const asks = [{ agent: lead, account: agent() === 'auto' ? undefined : account()?.id }, ...runs().map((r) => ({ agent: r.agent, account: r.account }))];
      void (async () => {
        for (const r of asks) {
          const h = await holds.check({ peer: target(), agent: r.agent, ...(r.account && { account: r.account }) });
          if (h) return h;
        }
        return undefined;
      })().then((h) => {
        setChecking(false);
        if (h) return void (setHold(h), input?.focus());
        pass = { kind: 'checked' };
        send();
      });
      return;
    }
    menu.reset();
    const p = project();
    const id = entityKey(target(), newRowId());
    const g = goal();
    // sent: Done when empties and Goal goes off for the next prompt; a refused launch puts both back
    const taken = goalSent(homeGoal());
    if (g !== undefined) putGoal(taken.next);
    // unless Done when was opened and typed in again meanwhile
    const goalBack = (): void => void (g !== undefined && !goalOn() && !goalDraft() && putGoal(taken.back));
    // a plugin's prompt source may take it (`/sandbox …`: a sandbox of its own), with what New task picked
    const w = when();
    const launch = {
      ...(p && { cwd: p.path }),
      agent: agent(),
      mode: mode(),
      // Auto's 'model' is its goal, the router's own: never sent as a model
      ...(agent() !== 'auto' && model() && { model: model() }),
      ...(effort() && { effort: effort() }),
      ...(g && { goal: g }),
      ...(w ? { at: whenAt(w, now()) } : through?.kind === 'wait' ? { at: through.at } : {}),
    };
    const pc = shell.client.plugin('shell');
    const place = placeOf();
    if (place) {
      // a plugin's place (a sandbox) takes the prompt with New task's picks; attached files stay for a computer
      if (files().length > 0) {
        shell.toast.error(t('shell.home.placeNoFiles', { place: place.label() }));
        return;
      }
      place.launch(prompt, { client: pc, launch });
      setText('');
      setWhen(undefined);
      return;
    }
    if (
      files().length === 0 &&
      shell
        .promptSources()()
        .some((s) => (s.where ?? ['home']).includes('home') && s.send?.(prompt, { client: pc, task: '', launch }) === true)
    ) {
      setText('');
      setWhen(undefined);
      return;
    }
    // the row and the page at once (optimistic), the box cleared; a refusal puts the text back. With files, the
    // task starts once they're up (usually already: they went when they were added)
    setText('');
    // Auto with a preview: the router picks on the peer (the same pick); the row shows it until the peer's arrives
    const ap = agent() === 'auto' ? autoPick(preview()) : undefined;
    const ov = overrideOf(preview(), { agent: agent(), model: model() });
    const runner = { agent: ap ? 'auto' : agent() === 'auto' ? auto() : agent(), ...(ap && { shows: ap.agent }), ...(ov && { autoOverride: ov }) };
    asked++;
    setPreview(undefined);
    // several runs: one group, so Compare runs shows them side by side (v1: one shared groupId)
    const extra = runs();
    const group = extra.length > 0 ? newGroup(crypto.getRandomValues(new Uint8Array(6))) : undefined;
    // the copy of the lead with one setting changed: one runtime.trial (both runs linked, a worktree each)
    const tried = p && runner.agent !== 'auto' && extra.length === 1 && varies(extra[0] as Runner, 0).length > 0 ? trialVary(agent(), extra.map((r) => r.agent), vary(), subSetting()) : undefined;
    setRuns([]);
    setCmpPicker(undefined);
    setVary(undefined);
    // the other folders it works in: on this computer only (another one has its own checkouts, placed per folder later)
    const dirs = target() === home ? dirsOf(p, also(), projects()) : [];
    setOwnAlso(undefined);
    // Try it two ways: one runtime.trial for both runs (linked, a worktree each), both rows at once
    const startTrial = (attachments: readonly Attachment[], cwd: string): boolean => {
      if (!tried || !p || !group) return false;
      shell.client.commands
        .run(trial, {
          peer: target(),
          ids: [id, entityKey(target(), newRowId())],
          group,
          vary: tried,
          task: {
            prompt,
            agent: agent(),
            mode: mode(),
            at: Date.now(),
            cwd,
            projectId: p.id,
            ...(p.color && { color: p.color }),
            ...(dirs.length > 0 && { dirs }),
            ...(g && { goal: g }),
            ...(attachments.length > 0 && { attachments }),
            ...(model() && { model: model() }),
            ...(effort() && { effort: effort() }),
            ...((a) => (a ? { account: a.id } : {}))(account()),
            ...(team().length > 0 && { team: teamToSend(team()) }),
          },
        })
        .catch(() => (setText(prompt), goalBack()));
      shell.navigate({ t: 'openTask', id: id as EntityKey as never });
      return true;
    };
    const yes = through?.kind === 'confirmed';
    // the peer knew better (a harness wisp picked, another computer's plan): refused before anything was made, so
    // back to Home with the row, the words back in the box (v1 launcher.js: the hold row once the prompt is back)
    const heldBack = (err: unknown): void => {
      const reason = (err as { reason?: string } | null)?.reason;
      const answer = shell.sendHolds?.answers(reason);
      if (!answer) return;
      shell.navigate({ t: 'home' });
      void answer({ peer: target(), agent: runner.agent, ...((a) => (a ? { account: a.id } : {}))(account()) }).then((h) =>
        setHold(h ?? { reason: reason ?? '', say: (err as Error).message }),
      );
    };
    const start = (attachments: readonly Attachment[], cwd: string): void => {
      if (startTrial(attachments, cwd)) return;
      shell.client.commands
        .run(create, {
          peer: target(),
          id,
          prompt,
          ...runner,
          ...(group && { group }),
          mode: mode(),
          at: Date.now(),
          ...(p && { cwd, projectId: p.id, color: p.color, worktree: worktree() }),
          ...(dirs.length > 0 && { dirs }),
          ...(g && { goal: g }),
          ...(attachments.length > 0 && { attachments }),
          ...(agent() !== 'auto' && model() && { model: model() }),
          ...(agent() !== 'auto' && effort() && { effort: effort() }),
          ...((a) => (agent() !== 'auto' && a ? { account: a.id } : {}))(account()),
          ...(team().length > 0 && { team: teamToSend(team()) }),
          ...(yes && { confirmLimit: true }),
        })
        .catch((err: unknown) => (setText(prompt), goalBack(), heldBack(err)));
      // the other runs after the lead, so the lead is run A (Compare orders runs by when they started)
      for (const r of extra) {
        shell.client.commands
          .run(create, {
            peer: target(),
            id: entityKey(target(), newRowId()),
            prompt,
            agent: r.agent,
            ...(r.model && { model: r.model }),
            ...(r.effort && { effort: r.effort }),
            ...(r.account && { account: r.account }),
            mode: mode(),
            at: Date.now(),
            group: group ?? '',
            // each run of a compare in its own checkout, or they'd edit one folder (v1 forced it)
            ...(p && { cwd, projectId: p.id, color: p.color, worktree: true }),
            ...(dirs.length > 0 && { dirs }),
            ...(attachments.length > 0 && { attachments }),
            ...(yes && { confirmLimit: true }),
          })
          .catch(() => undefined);
      }
      shell.navigate({ t: 'openTask', id: id as EntityKey as never });
    };
    // on another computer: its checkout of the project's repo first, found or cloned there (v1 places.js), said meanwhile
    const go = (attachments: readonly Attachment[]): void => {
      const peer = target();
      if (!p || peer === home) return start(attachments, p?.path ?? '');
      const where = shell.client.peers.name(peer);
      shell.toast.show(t('shell.runOn.setting', { name: p.name, where }));
      placeOn({ cwd: p.path, peer, home, name: p.name, where, run: (cmd, args) => shell.client.commands.run(cmd, args), say: (name, w) => t('shell.runOn.noOrigin', { name, where: w }) }).then(
        (cwd) => start(attachments, cwd),
        (err: unknown) => {
          setText(prompt);
          shell.toast.error(err instanceof Error ? err.message : t('shell.runOn.failed', { where }));
        },
      );
    };
    if (files().length === 0) go([]);
    else void att.take().then(go);
  };
  const attach = (o: PickOptions): void => {
    setPicking(true);
    void shell.platform.pickFiles(o).then((picked) => {
      setPicking(false);
      if (picked.length) att.add(picked);
    });
  };
  // the repo is fetched while you type, so Send starts at once (v1 warmLaunch)
  const warmUp = warmer(Date.now, (path) => void shell.client.commands.run(warm, { peer: home, path }).catch(() => undefined));
  const warmIt = (): void => warmUp(worktree() && target() === home ? project()?.path : undefined);
  // no coding agent on this computer: the install strip (v1 setup.js)
  const setup = useSetup(shell, () => home, () => harnessOf(harnesses(), INSTALLABLE).logo, () => input?.focus());
  // the Recipes row: the picked project's recipes, each opening its run dialog (v1 renderLaunchRecipes)
  const recipes = useRecipes(shell, () => home, () => (target() === home ? project()?.path : undefined));
  const newRecipeAct = { onClick: () => shell.navigate(newRecipeIntent(project()?.path, mode())) };
  // Start from an issue (wisp.issues' picker, an overlay view; without that plugin, navigation drops it)
  const issueAct = {
    onClick: () => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'issues.picker' as never, params: { project: project()?.id ?? '', name: project()?.name ?? '' } } }),
  };
  // the phone apps fold the bar's tail into More, once it's full (a first run has its own More options)
  const foldPhone = (): boolean => phone && !firstRun();
  // vs + (v1 launcher.js compare): the next harness as another run; the phone's More offers it too (QA phone r4 #1)
  const compareAct: Act = {
    onClick: () => {
      const was = runs();
      const next = nextRun(
        agent(),
        was.map((r) => r.agent),
        harnesses().map((h) => h.id),
      );
      if (next) {
        const a = account();
        setRuns([...was, newRun(next, { agent: agent(), model: model(), effort: effort(), account: a?.id ?? '', ...(modelLabel() && { modelLabel: modelLabel() }), ...(a && { accountLabel: a.label }) })]);
        // What's different is for one copy: a third run makes it a compare again
        setVary(undefined);
        // a repeat is rarely the one you want: straight into picking it (v1 vsBtn.onclick)
        if (next === agent() || was.some((r) => r.agent === next)) (setPicker(false), setAutoPop(false), setCmpPicker(was.length));
      }
    },
  };
  // the phone app's mic (v1 dictate.js): none elsewhere
  const mic = useMic(shell, { text, set: (v) => (setText(v), menu.typed(v), ask(v)), el: () => input });
  const tool = (id: string, part: string, icon: LauncherTool['icon'], label: string, title: string, more: Partial<LauncherTool> = {}): LauncherTool => ({ id, part, icon, label, title, ...more });
  const view = (id: string): void => shell.navigate({ t: 'openView', id: id as never, params: null as Json });
  const settingsSheet = (pane: string): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane } } });
  const shown = (): { agent: string; label: string } | undefined => (agent() === 'auto' ? autoPick(preview()) : undefined);
  const pick = (): { name: string; logo?: string } => harnessOf(harnesses(), shown()?.agent ?? (agent() === 'auto' ? auto() : agent()));
  // where New task runs (the bar's place button and the Delegate panel's lead): this computer, a machine, a plugin's place
  const whereAct = (): Act => ({
    menu: {
      items: [
        ...whereItems(
          runOnPeers(),
          placeOf() ? runOn() : target(),
          home,
          (p) => shell.client.peers.name(p as PeerId),
          t,
          targets().map((d) => ({ id: d.id, label: d.label(), icon: d.icon })),
        ),
        { kind: 'separator' },
        { value: 'add', label: t('shell.home.addMachine'), icon: 'plus' },
      ],
      // a pick is the one place (the menu closes on it, as every menu does)
      onSelect: (v) => (v === 'add' ? view('machine.this') : setRunOn(v)),
    },
  });
  // where a delegate can run away from the lead (v1 team.js whereMenu): this computer when the lead is elsewhere, and
  // every other machine
  const delegatePlaces = (): DelegatePlace[] => [
    ...(target() === home ? [] : [{ id: 'here', label: t('shell.card.thisComputer'), icon: 'laptop' as const }]),
    ...runOnPeers()
      .filter((p) => p !== home && p !== target())
      .map((p) => ({ id: p, label: shell.client.peers.name(p as PeerId), icon: 'server' as const })),
  ];
  // where it works (owner, 2026-11-15): the one project selector, the same list as ⌘P on this box's place
  const placeTarget: PlaceTarget = {
    pick: () => placePick(project()?.id, also()),
    set: (ids) => {
      setWhere(ids[0] ?? 'none');
      setOwnAlso(ids.slice(1));
    },
    other: () => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.projects' as never, params: null } }),
  };
  onCleanup(() => releasePlaces(placeTarget));
  const place = (): LauncherProps['place'] => {
    const p = project();
    const picked = p ? [p, ...pickedProjects(also(), projects())] : [];
    const label = p ? pickLabel(picked, t) : t('shell.home.noFolder');
    const tip = p ? picked.map((x) => x.path).join('\n') : t('shell.home.noFolderTitle');
    return {
      label,
      colors: picked.map((x) => x.color || 'slate'),
      icon: 'text',
      title: `${tip}\n${t('shell.home.placeTitle')}`,
      ariaLabel: t('shell.home.placeAria', { name: label }),
      onOpen: () => openPlaces(shell, placeTarget),
    };
  };
  const extras = () => [
    ...files().map((f, i) => ({ id: `file:${i}:${f.name}`, icon: 'clip' as const, label: f.name, removeLabel: t('shell.home.remove', { name: f.name }), onRemove: () => att.remove(f) })),
    ...(when()
      ? [{ id: 'when', icon: 'clock' as const, label: t('shell.when.chip', { when: startsAt(whenAt(when() ?? 'hour', now()), now()) }), removeLabel: t('shell.when.clear'), onRemove: () => setWhen(undefined) }]
      : []),
  ];
  const pluginTips = useSignal(computed(() => (shell.homeTips?.()() ?? []).filter((d) => d.offered?.(d.client) ?? true)));
  const offeredTips = (): readonly string[] => (autoOn() ? TIPS : TIPS.filter((x) => !AUTO_TIPS.has(x)));
  // UX 2026-11-18 #8: one tip at a time, the setup cards (GitHub, the Grid) among them; More tips moves one on
  const setupTips = (): SetupTip[] => [
    ...(githubTip(ghTools())
      ? [{ kind: 'tool' as const, icon: 'github' as const, title: t('shell.tips.github'), body: t('shell.tips.githubBody'), cta: t('shell.tips.signIn'), onClick: () => settingsSheet('accounts') }]
      : []),
    // plugins' set-up tips (wisp.grid-client: Reach this wisp from anywhere, never on a grid box; docs/ux/2026-11-18-grid.md #9)
    ...pluginTips().map((d): SetupTip => ({ kind: 'grid', icon: d.icon, title: d.title(), body: d.body(), cta: d.cta(), onClick: () => d.open({ client: d.client, settings: settingsSheet }) })),
  ];
  const tipNow = () => tipPick(tipAt(), setupTips().length, offeredTips());
  const tips = (): PlainTip[] =>
    ((p) => (p?.kind === 'tip' ? [{ before: t(`shell.tips.${p.id}`), bold: t(`shell.tips.${p.id}b`), after: t(`shell.tips.${p.id}a`), href: `${GUIDE}#${p.id}` }] : []))(tipNow());
  const setupNow = (): SetupTip[] => ((p) => (p?.kind === 'setup' ? setupTips().slice(p.i, p.i + 1) : []))(tipNow());
  // the footer's tools, one definition each: the old footer (the phone's sheet, the first run) and the desktop's one
  // footer (docs/ux/2026-11-18-desktop.md #3) draw the same controls
  const whereTool = (): LauncherTool =>
    tool(
      'where',
      'launcher.where',
      placeOf()?.icon ?? (target() === home ? 'laptop' : 'server'),
      // a phone's setup chip names the place in two words (UX 2026-11-18 phone #5); the sentence stays its tooltip
      placeOf()?.label() ?? (target() === home ? t(shell.platform.phone ? 'shell.card.thisComputer' : 'shell.home.where') : shell.client.peers.name(target())),
      t('shell.home.where'),
      {
        text: target() !== home || placeOf() !== undefined,
        ...((target() !== home || placeOf() !== undefined) && { on: true }),
        act: whereAct(),
      },
    );
  const teamTool = (): LauncherTool =>
    tool('team', 'launcher.team', 'agent', team().length ? presetName() ?? t(teamSize(team()) === 1 ? 'shell.home.delegates.one' : 'shell.home.delegates.other', { count: teamSize(team()) }) : t('shell.home.delegate'), t('shell.home.delegateTitle'), {
      text: true,
      ...(team().length > 0 && { on: true }),
      act: {
        panel: {
          label: t('shell.home.delegate'),
          part: 'launcher.team.panel',
          cardPart: 'delegate.panel',
          content: () => (
            <DelegatePanel
              lead={{
                agent: agent(),
                run: [agentName(t, harnesses(), agent()), agent() === 'auto' ? (shown()?.label ?? '') : runnerPick({ agent: agent(), name: pick().name, model: modelWord({ model: model(), modelLabel: modelLabel() }), effort: effort(), account: '', fallback: defaultName(agent(), t('shell.runner.default')) })].filter(Boolean).join(' · '),
                logo: agent() === 'auto' ? undefined : harnessOf(harnesses(), agent()).logo,
                place: { icon: placeOf()?.icon ?? (target() === home ? 'laptop' : 'server'), label: placeOf()?.label() ?? (target() === home ? t('shell.card.thisComputer') : shell.client.peers.name(target())), title: t('shell.home.where') },
                placeAct: whereAct(),
              }}
              team={team()}
              onChange={pickTeam}
              peer={target()}
              places={delegatePlaces()}
              onAddMachine={() => view('machine.this')}
              preset={preset()}
              onPreset={setPreset}
            />
          ),
        },
      },
    });
  const goalTool = (): LauncherTool =>
    tool('goal', 'launcher.goal', 'target', t('shell.home.goal'), t('shell.home.goalTitle'), {
      text: true,
      ...(goalOn() && { on: true }),
      act: { onClick: () => (setGoalOn(!goalOn()), queueMicrotask(() => (goalOn() ? goalInput : input)?.focus())) },
    });
  const worktreeTool = (): LauncherTool =>
    tool('worktree', 'launcher.worktree', 'branch', worktree() ? t('shell.home.worktree') : t('shell.home.inPlace'), t('shell.home.worktreeTitle'), {
      text: true,
      on: worktree(),
      act: { onClick: () => void shell.client.commands.run(setSetting, { peer: home, key: WORKTREE_KEY, value: worktree() ? 'off' : 'on' }).catch(() => undefined) },
    });
  const laterTool = (): LauncherTool =>
    tool('later', 'launcher.later', 'clock', t('shell.home.later'), t('shell.home.later'), {
      ...(when() && { on: true }),
      act: { menu: { items: whenMenu(t, now()), onSelect: (v) => setWhen(v as When) } },
    });
  // UX 2026-11-18 #3 (docs/ux/2026-11-18-desktop.md, decided by the owner's delegate): the desktop's one footer, `+`, the
  // mode, the harness · model, Where and Send; what's on shows as a chip with its ✕. The phone's sheet and the first run
  // keep their own bar
  const desk = (): boolean => !phone && !firstRun();
  // Delegate… in `+` opens the Delegate panel on its chip (shown while the panel is open or the team has someone)
  const [delegateOpen, setDelegateOpen] = createSignal(false);
  const canCompare = (): boolean =>
    runs().length < MAX_RUNNERS - 1 &&
    nextRun(
      agent(),
      runs().map((r) => r.agent),
      harnesses().map((h) => h.id),
    ) !== undefined;
  const setWorktree = (on: boolean): void => void shell.client.commands.run(setSetting, { peer: home, key: WORKTREE_KEY, value: on ? 'on' : 'off' }).catch(() => undefined);
  const plus = (): LauncherTool =>
    plusTool({
      part: 'launcher.plus',
      label: t('shell.home.plus'),
      title: t('shell.home.plusTitle'),
      sections: [
        [
          { id: 'attach', part: 'launcher.attach', icon: 'clip', label: t('shell.home.attach'), act: attachAct(shell, attach, picking()) },
          { id: 'issue', part: 'launcher.issue', icon: 'issue', label: t('shell.home.issueRow'), act: issueAct },
          {
            id: 'recipes',
            part: 'launcher.recipes',
            icon: 'bolt',
            label: t('shell.home.recipes'),
            act: {
              menu: {
                items: [...recipes.items().map((r) => ({ value: `recipe:${r.id}`, label: r.label })), { value: 'recipe:new', label: t('shell.home.recipesNew'), icon: 'plus' as const }],
                onSelect: (v) => (v === 'recipe:new' ? newRecipeAct.onClick() : recipes.items().find((r) => `recipe:${r.id}` === v)?.act.onClick?.()),
              },
            },
          },
        ],
        [
          !goalOn() && { id: 'goal', part: 'launcher.goal', icon: 'target', label: t('shell.home.goalRow'), act: { onClick: () => (setGoalOn(true), queueMicrotask(() => goalInput?.focus())) } },
          team().length === 0 && !delegateOpen() && { id: 'team', part: 'launcher.team', icon: 'agent', label: t('shell.home.delegateRow'), act: { onClick: () => setDelegateOpen(true) } },
          canCompare() && { id: 'compare', part: 'launcher.compare', icon: 'split', label: t('shell.home.compareRow'), act: compareAct },
        ],
        [!when() && { id: 'later', part: 'launcher.later', icon: 'clock', label: t('shell.home.later'), act: laterTool().act }],
      ],
    });
  // Where it runs and how, one chip (Where and Worktree were two chips for one question): "This computer · Worktree"
  const whereChip = (): LauncherTool => {
    const place = placeOf()?.label() ?? (target() === home ? t('shell.card.thisComputer') : shell.client.peers.name(target()));
    // No folder: no git repo, so no worktree either (v1 paintScratch)
    const how = project() === undefined ? '' : worktree() ? t('shell.home.worktree') : t('shell.home.inPlace');
    const where = whereAct().menu;
    return tool('where', 'launcher.where', placeOf()?.icon ?? (target() === home ? 'laptop' : 'server'), how ? `${place} · ${how}` : place, t('shell.home.whereHow'), {
      text: true,
      act: {
        menu: {
          items: [
            ...(where?.items ?? []),
            ...(how
              ? [
                  {
                    kind: 'group' as const,
                    id: 'how',
                    label: t('shell.home.folderHow'),
                    items: [
                      { value: 'how:worktree', label: t('shell.home.worktree'), icon: 'branch' as const, checked: worktree(), part: 'launcher.worktree' as const },
                      { value: 'how:inPlace', label: t('shell.home.inPlace'), icon: 'folder' as const, checked: !worktree(), part: 'launcher.worktree' as const },
                    ],
                  },
                ]
              : []),
          ],
          onSelect: (v) => (v === 'how:worktree' || v === 'how:inPlace' ? setWorktree(v === 'how:worktree') : where?.onSelect(v)),
        },
      },
    });
  };
  // what's on, after Where: a chip each, cleared in one press (Compare's runs stay in the harness pill with their ×)
  const onChips = (): LauncherTool[] => {
    const team0 = teamTool();
    const panel = team0.act?.panel;
    return [
      ...(goalOn()
        ? [
            tool('goal', 'launcher.goal', 'target', (goalDraft() ?? '').trim() ? t('shell.home.goalChip', { text: (goalDraft() ?? '').trim() }) : t('shell.home.goal'), t('shell.home.goalTitle'), {
              text: true,
              on: true,
              act: { onClick: () => goalInput?.focus() },
              clear: { label: t('shell.home.goalOff'), act: { onClick: () => setGoalOn(false) } },
            }),
          ]
        : []),
      ...(team().length > 0 || delegateOpen()
        ? [
            {
              ...team0,
              ...(panel && { act: { panel: { ...panel, open: delegateOpen(), onOpenChange: setDelegateOpen } } }),
              // its ✕ from the start (a chip that gained one on the first pick was drawn again, the open panel with it):
              // it clears the team, or with nobody yet closes the panel
              clear: team().length > 0 ? { label: t('shell.home.delegateOff'), act: { onClick: () => (pickTeam([]), setPreset('')) } } : { label: t('shell.home.delegateClose'), act: { onClick: () => setDelegateOpen(false) } },
            },
          ]
        : []),
      ...(when()
        ? [
            tool('later', 'launcher.later', 'clock', t('shell.when.chip', { when: startsAt(whenAt(when() ?? 'hour', now()), now()) }), whenTip(whenAt(when() ?? 'hour', now()), now()), {
              text: true,
              on: true,
              act: laterTool().act ?? {},
              clear: { label: t('shell.when.clear'), act: { onClick: () => setWhen(undefined) } },
            }),
          ]
        : []),
    ];
  };
  return (
    <>
    {/* the phone apps: New task is a page (Android's full-screen dialog) or a sheet (iOS) with Close / Cancel and Send in
        its bar; the browser at a phone's width keeps v1's Back and greeting */}
    <Show when={phone ? props.onBack : undefined}>
      {(close) => <PhoneCompose onClose={close()} onSend={send} canSend={text().trim() !== '' || files().length > 0} />}
    </Show>
    <SplitHome {...(!sheet() && { greeting: t('shell.home.hello') })} {...(!phone && props.onBack && { onBack: props.onBack, backLabel: t('shell.home.back') })}>
      <Launcher
        inLabel={t('shell.home.in')}
        place={place()}
        placeholder={desk() ? t('shell.home.placeholderPlus') : promptHint()}
        value={text()}
        onInput={(v, e) => {
          setText(v);
          // a task id a phone keyboard's clipboard put in becomes its mention (v1 #800): ask about what's left
          ask(menu.typed(v, e));
          warmIt();
        }}
        onSend={send}
        onKey={onKey}
        firstRun={firstRun()}
        setup={target() === home ? setup() : undefined}
        suggest={suggest()}
        onPaste={(e) => menu.paste(e) || att.paste(e)}
        onFiles={att.add}
        inputRef={(el) => {
          input = el;
          // v1 launcher.js hint(): while focus is elsewhere the placeholder names the shortcut that brings it back
          el.addEventListener('focus', () => setPromptFocused(true));
          el.addEventListener('blur', () => setPromptFocused(false));
          setPromptFocused(document.activeElement === el);
        }}
        mode={{ label: t(`shell.mode.${mode()}`), title: t('shell.home.fullAutoTitle'), icon: MODE_ICON[mode()], act: { menu: { items: modeMenu(t, mode()), onSelect: (v) => setMode(v as Mode) } } }}
        runner={{
          label: agentName(t, harnesses(), agent()),
          // the pill's tooltip: Auto's (what it will run, as pickTitle says) or the harness's
          title: agent() === 'auto' ? t('shell.home.autoTitle') : t('shell.runner.title'),
          auto: agent() === 'auto',
          pick: runnerPick({ agent: agent(), shown: shown()?.label, name: pick().name, model: modelWord({ model: model(), modelLabel: modelLabel() }), effort: effort(), account: account()?.label ?? '', fallback: t('shell.runner.default') }),
          // a picked harness's model, thinking and account as v1's parts: the narrow bar folds them one by one (launcher.css)
          ...(agent() !== 'auto' && { pickParts: runnerParts({ model: modelWord({ model: model(), modelLabel: modelLabel() }), effort: effort(), account: account()?.label ?? '', fallback: defaultName(agent(), t('shell.runner.default')) }) }),
          pickTitle:
            agent() !== 'auto'
              ? t('shell.runner.title')
              : shown()
                ? t('shell.home.autoWhy', { label: preview()?.pick?.label ?? '', why: preview()?.why ?? '' })
                : t('shell.home.pickTitle', { name: pick().name }),
          ...(pick().logo && { logo: pick().logo }),
          compare: runs().length > 0 ? t('shell.home.compareMore', { n: MAX_RUNNERS }) : t('shell.home.compare'),
          ...(ring() && { usage: ring() }),
          // ONE pill, ONE press: the two-pane harness picker (v1 runner-picker.js); model, thinking and account are its right pane
          act: { onClick: () => (setAutoPop(false), setCmpPicker(undefined), setPicker(!picker())) },
          open: picker(),
          onWarm: warmPicker,
          // what will Auto run: Auto's pick, the pill's own segment (v1 #autoPv), opens the AutoPop
          pickAct: { onClick: () => (setPicker(false), setCmpPicker(undefined), setAutoPop(!autoPop())) },
          pickOpen: autoPop(),
          compareAct,
          compareFull: firstRun() || runs().length >= MAX_RUNNERS - 1,
          // the desktop's one footer: Compare is in `+` (UX 2026-11-18 #3)
          ...(desk() && { compareInMenu: true }),
          compares: runs().map((r, i) => {
            const h = harnessOf(harnesses(), r.agent);
            const can = varies(r, i);
            const words = can.length ? variantWords(t, vary()) : [];
            return {
              label: h.name,
              logo: h.logo,
              removeLabel: t('shell.home.compareRemove', { name: h.name }),
              onRemove: () => (setCmpPicker(undefined), setRuns(runs().filter((_, k) => k !== i)), setVary(undefined)),
              ...runWords(r, words, t('shell.runner.default')),
              // the same two-pane harness picker as the lead's (v1 runnerPicker on .runner-btn.cmp-x), What's different in it
              act: { onClick: () => (setPicker(false), setAutoPop(false), setCmpPicker(cmpPicker() === i ? undefined : i)) },
              open: cmpPicker() === i,
            };
          }),
        }}
        lead={desk() ? [plus()] : firstRunTools(firstRun(), 'lead', [
          tool('attach', 'launcher.attach', 'clip', t('shell.home.attach'), t('shell.home.attach'), { act: attachAct(shell, attach, picking()) }),
          ...(mic ? [tool('mic', 'launcher.mic', 'mic', mic.label(), mic.label(), { on: mic.on(), act: { onClick: mic.toggle } })] : []),
          whereTool(),
        ])}
        mid={desk() ? [whereChip()] : firstRunTools(firstRun(), 'mid', [
          // v1 #771: no Try it two ways button; the runner's + adds the second run
          // v1 team.js paintTeamBtn: "Delegate", or how many agents the lead can hand work to; its panel (teamEditor)
          teamTool(),
        ])}
        tail={desk() ? onChips() : ((tools: LauncherTool[]) =>
          foldPhone()
            ? [
                foldTail(tools, {
                  label: t('shell.home.moreOptions'),
                  title: t('shell.home.moreOptionsTitle'),
                  // only while there's another harness to add: a press that did nothing would answer nothing (ADR-0017)
                  ...(runs().length < MAX_RUNNERS - 1 &&
                    nextRun(
                      agent(),
                      runs().map((r) => r.agent),
                      harnesses().map((h) => h.id),
                    ) !== undefined && { compare: { label: t('shell.home.compareRow'), act: compareAct } }),
                  issue: { label: t('shell.home.issueTitle'), act: issueAct },
                  recipes: { label: t('shell.home.recipes'), items: recipes.items().map((r) => ({ id: r.id, label: r.label, act: r.act })), add: { label: t('shell.home.recipesNew'), act: newRecipeAct } },
                }),
              ]
            : tools)(firstRunTools(
          firstRun(),
          'tail',
          [
            goalTool(),
            // No folder: no git repo, so no worktree either; the toggle would only mislead (v1 paintScratch)
            ...(project() === undefined
              ? []
              : [
                  worktreeTool(),
                ]),
            laterTool(),
          ],
          tool('more', 'launcher.more', 'dots', t('shell.home.moreOptions'), t('shell.home.moreOptionsTitle'), { text: true, act: { onClick: openFull } }),
        ))}
        // a bar too narrow for Goal, Worktree and Start later folds the last of them into More (QA desktop r2 #4); the
        // phone's bar has its own More, the first run its More options
        {...(!phone && !firstRun() && { tailMore: { label: t('shell.home.moreOptions'), title: t('shell.home.moreTail') } })}
        sendLabel={when() ? t('shell.when.send', { at: aheadLabel(whenAt(when() ?? 'hour', now()), now()) }) : t('shell.home.send')}
        {...(checking() && { sendAct: { pending: true } })}
        // the phone sheet: the round Send says what the header's says (one state: nothing to send yet, QA r2 #12)
        {...(sheet() && text().trim() === '' && files().length === 0 && { sendOff: true })}
        hold={((h) =>
          h && {
            say: h.say,
            goLabel: t('shell.hold.start'),
            goTitle: t('shell.hold.startTitle'),
            keepLabel: t('shell.hold.keep'),
            ...(waitable(h.until, Date.now()) && { wait: { label: t('shell.hold.wait', { at: aheadLabel(h.until ?? 0, Date.now()) }), title: t('shell.hold.waitTitle') } }),
            onGo: () => ((pass = { kind: 'confirmed' }), send()),
            onWait: () => h.until !== undefined && ((pass = { kind: 'wait', at: h.until }), send()),
            onKeep: () => (setHold(undefined), input?.focus()),
          })(hold())}
        {...(goal() !== undefined && {
          goal: {
            label: t('shell.home.doneWhen'),
            placeholder: t('shell.home.goalPlaceholder'),
            value: goal() ?? '',
            onInput: setGoal,
            ref: (el: HTMLInputElement) => (goalInput = el),
            end: goalEnd,
          },
        })}
        extras={desk() ? extras().filter((x) => x.id !== 'when') : extras()}
        recipes={{
          label: t('shell.home.recipes'),
          empty: recipes.empty(),
          items: recipes.items(),
          add: t('shell.home.recipesNew'),
          issue: t('shell.home.issue'),
          issueTitle: t('shell.home.issueTitle'),
          addAct: newRecipeAct,
          issueAct,
          // the phone apps: Recipes and Start from an issue are in the bar's More (phone-bar.ts), not a row of their own
          ...((foldPhone() || desk()) && { inMore: true }),
        }}
      />
      {/* usage at a glance under the prompt box (wisp.budget's plan windows), v1 #790 .sh-usage; not in the phone sheet */}
      <Show when={!sheet()}>
        <Reserved part="home.usage" height={usageH()} onHeight={(px) => void shell.client.settings.set('shell.home.usageHeight', Math.min(400, px)).catch(() => undefined)}>
          <Slot input="shell.home:usage" layout="contents" />
        </Reserved>
      </Show>
      {/* opened from the Auto pick; its own chunk (Auto is off unless its switch is on, owner #37) */}
      <Show when={autoPop()}>
        <HomeAutoPop
          preview={preview}
          harnesses={harnesses}
          neverModel={neverModel}
          typeWord={typeWord}
          onClose={() => setAutoPop(false)}
          onPick={(c) => {
            // run another pick instead: it's yours now, and the task tells Auto what it had shown (autoOverride)
            setAutoPop(false);
            if (!c) return;
            pickAgent(c.agent);
            setModel(c.model);
            // its name from Auto's label ("Codex · GPT-5 · High", v1 shortLabel), so the pill names it, not Default
            setModelLabel(c.model ? (c.label.split(' · ')[1] ?? '') : '');
            setEffort(c.effort);
            keepPick(c.agent, c.model, c.model ? (c.label.split(' · ')[1] ?? '') : '', c.effort);
          }}
          onNever={(x, type) => {
            setAutoPop(false);
            void never(x, type);
          }}
        />
      </Show>
      <RunnerPicker
        open={picker()}
        onOpenChange={setPicker}
        anchor={() => document.querySelector('[data-part="launcher.harness"][data-lead]')}
        back={() => input}
        peer={target()}
        auto={autoOn()}
        accounts
        // the names ride along, so a thinking or account change (withSetting) keeps the model's and account's words
        value={{ agent: agent(), model: model(), effort: effort(), account: account()?.id ?? '', ...(modelLabel() && { modelLabel: modelLabel() }), ...(account() && { accountLabel: account()?.label ?? '' }) }}
        onChange={(r) => {
          const was = agent();
          setAgent(r.agent);
          setModel(r.model);
          setModelLabel(r.modelLabel ?? '');
          setEffort(r.effort);
          setAccount(r.account ? { id: r.account, label: r.accountLabel ?? r.account } : undefined);
          keepPick(r.agent, r.model, r.modelLabel ?? '', r.effort);
          // back on Auto, or another goal: what it will run is asked again
          if (r.agent === 'auto') ask(text(), was !== 'auto');
        }}
      />
      {/* a compare run's harness picker: the same one, anchored on that run's button; picks here don't touch the lead */}
      <Show when={((i) => (i === undefined ? undefined : runs()[i] && { i, r: runs()[i] as Runner }))(cmpPicker())}>
        {(c) => (
          <RunnerPicker
            open
            onOpenChange={(o) => !o && setCmpPicker(undefined)}
            anchor={() => document.querySelectorAll('[data-part="launcher.compare.runner"]')[c().i]}
            back={() => input}
            peer={target()}
            accounts
            value={c().r}
            onChange={(r) => {
              setRuns(runs().map((x, k) => (k === c().i ? r : x)));
              // another harness or model: no longer a copy of the lead with one change
              if (varies(r, c().i).length === 0) setVary(undefined);
            }}
            vary={{
              can: (id, r) => varies({ ...r, agent: id }, c().i),
              value: vary(),
              setting: subSetting(),
              onChange: (v, r) => {
                setRuns(runs().map((x, k) => (k === c().i ? r : x)));
                setVary(v);
              },
            }}
          />
        )}
      </Show>
      <Show when={!sheet()}>
        <NextList
          heading={t('shell.section.needs')}
          count={needs().length}
          countLabel={needs().length > 5 ? t('shell.home.needsOf', { shown: 5, all: needs().length }) : undefined}
          rows={needs()
            .slice(0, 5)
            .map((s) => ({
              id: s.id,
              title: s.title,
              color: String(s.summary['color'] ?? 'slate'),
              state: statusOf(s, t).label,
              tone: statusOf(s, t).tone,
              why: String(s.summary['question'] || s.summary['preview'] || '').split('\n')[0]?.trim() ?? '',
              time: shortAgo(s.updated_at, now()),
              timeTitle: longTime(s.updated_at),
            }))}
          onOpen={(id) => shell.navigate({ t: 'openTask', id: sessionKey(untrack(shell.client.entities.keys(SESSION)), id) as never })}
        />
        {/* notes on plans other plugins add (wisp.budget's Spare allowance), v1 .sh-spare */}
        <Reserved part="home.cards" height={cardsH()} onHeight={(px) => void shell.client.settings.set('shell.home.cardsHeight', Math.min(800, px)).catch(() => undefined)}>
          <Slot input="shell.home:cards" layout="contents" pickAgent={pickAgent} agent={agent} />
        </Reserved>
        <Tips
          heading={t('shell.tips.heading')}
          more={t('shell.tips.more')}
          moreTitle={t('shell.tips.moreTitle')}
          onMore={() => setTipAt(tipAt() + 1)}
          onOpen={(href) => shell.platform.openExternal(href)}
          setup={setupNow()}
          tips={tips()}
        />
        <KeysHint
          items={[
            { keys: ['J', 'K'], label: t('shell.keys.move') },
            { keys: ['↵'], label: t('shell.keys.open') },
            { keys: ['E'], label: t('shell.keys.done') },
            { keys: ['Esc'], label: t('shell.keys.back') },
            { keys: ['['], label: t('shell.keys.hide') },
          ]}
        />
        {/* the telemetry plugin's "Hit a bug…" line, while feedback can be sent (v1 split.js .sh-fb, B50) */}
        <Slot input="shell.home:feedback" layout="contents" />
      </Show>
    </SplitHome>
    </>
  );
}
