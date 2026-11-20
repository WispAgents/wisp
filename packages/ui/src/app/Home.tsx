// Home as v1 draws it (web/css/split.css .split-home, launcher.css, composer.css, auto.css, tips):
// the greeting, the prompt box (where it runs, the prompt, its toolbar, Recipes), what needs you,
// then the tips. On a phone it's its own page with a back button in its head.
import { For, Index, Show, createEffect, createMemo, createSignal, on, type JSX } from 'solid-js';
import { growTextarea } from '../core/autosize.ts';
import { LauncherSetup, type LauncherSetupProps } from './LauncherSetup.tsx';
import { HoldRow, type HoldRowProps } from '../composer/HoldRow.tsx';
import { flag } from '../core/props.ts';
import { useOpenWithin } from '../core/open-within.ts';
import { ForKeyed } from '../core/keyed.tsx';
import { Icon } from '../content/Icon.tsx';
import { PromptSuggestList, dropHandlers } from '../composer/PromptSuggest.tsx';
import { suggestKeyDown, type PromptSuggest } from '../composer/suggest-keys.ts';
import type { IconName } from '../content/icons.ts';
import { ActIcon, WithAct, actAttrs, type Act } from './act.tsx';
import { foldOrder, foldTail, type LauncherTool } from './launcher-fold.ts';
import { useTailFold } from './launcher-fold-measure.ts';
import { useBarLabels } from './bar-labels.ts';
import { Tool } from './launcher-tool.tsx';
import { ProjectSelect, type ProjectSelectProps } from './ProjectSelect.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';

/** v1 launcher.js: the prompt's tallest, in px, before it scrolls */
const PROMPT_MAX = 340;

export interface SplitHomeProps {
  /** none in the phone apps' New task sheet, whose bar says New task (r291) */
  readonly greeting?: string;
  /** stack only: back to the list */
  readonly backLabel?: string;
  readonly onBack?: () => void;
  readonly children?: JSX.Element;
}

export function SplitHome(props: SplitHomeProps): JSX.Element {
  const look = usePlatform();
  return (
    <div data-kit="split-home" data-part="split.home">
      {/* the desktop app's window moves from Home's top as from a task's head (owner 2026-11-18): a strip in the top
          padding, so it never covers content; [data-drag] does nothing outside the desktop app; the phone apps have no window */}
      <Show when={look() === 'web'}>
        <div data-kit="sh-drag" data-drag="" aria-hidden="true" />
      </Show>
      <Show when={props.onBack}>
        <div data-kit="sh-bar">
          <button type="button" data-kit="sh-back" data-part="split.back" aria-label={props.backLabel} title={props.backLabel} onClick={() => props.onBack?.()}>
            <Icon name="back" />
          </button>
          <h2 data-kit="sh-hello" data-part="split.greeting">
            {props.greeting}
          </h2>
        </div>
      </Show>
      <Show when={!props.onBack && props.greeting !== undefined}>
        <h2 data-kit="sh-hello" data-part="split.greeting">
          {props.greeting}
        </h2>
      </Show>
      {props.children}
    </div>
  );
}

export type { LauncherTool } from './launcher-fold.ts';

/** One recipe in the launcher's Recipes row (v1 .lp-chip.rc): its name, its prompt as the tip, and whether it asks
 * for fields (a "…" after its name). */
export interface LauncherRecipe {
  readonly id: string;
  readonly label: string;
  readonly title: string;
  readonly fields: boolean;
  readonly act: Act;
}

/** One more harness the launch runs on (v1 .vs-del + .runner-btn.cmp-x). */
export interface LauncherCompare {
  readonly label: string;
  readonly logo?: string | undefined;
  /** "Remove Codex from the comparison" */
  readonly removeLabel: string;
  readonly onRemove: () => void;
  /** its press: the harness picker for this run (v1 runnerPicker on .runner-btn.cmp-x) */
  readonly act: Act;
  /** its picker is open: the button stays lit while it is */
  readonly open?: boolean;
  /** the run's own model (v1 .rb-m), after the name: "Default" on its harness's default; none on a copy that says its variant */
  readonly model?: string | undefined;
  /** Try it two ways: a copy of the lead's words for its one change ("Subagents on Haiku"), after the name */
  readonly variant?: readonly string[];
}

export interface LauncherProps {
  readonly inLabel: string;
  /** where it works (owner, 2026-11-15): the one project selector in place of v1's row of place chips; it opens the
   *  same list as ⌘P, with No folder and Other folder… after the projects */
  readonly place: Omit<ProjectSelectProps, 'part' | 'where'>;
  readonly placeholder: string;
  readonly value: string;
  /** what's in the box after each input, with the input event (v1 #800: what a phone keyboard's clipboard put in) */
  readonly onInput: (v: string, e?: InputEvent) => void;
  readonly onSend: () => void;
  /** before the first task (v1 .launcher.first-run): on a narrow box the mode keeps its words, so two sparkles never sit unnamed side by side */
  readonly firstRun?: boolean;
  /** the box's own keys after its menus (↑ history, Shift+Tab mode): true when taken */
  readonly onKey?: (e: KeyboardEvent) => boolean;
  /** the @ and / menus (under the box) */
  readonly suggest?: PromptSuggest | undefined;
  /** a paste into the box (files): the host may take it */
  readonly onPaste?: (e: ClipboardEvent) => void;
  /** files dropped on the box; it lights up while they're over it */
  readonly onFiles?: (files: File[]) => void;
  readonly inputRef?: (el: HTMLTextAreaElement) => void;
  readonly mode: { readonly label: string; readonly title: string; readonly icon?: IconName; readonly act?: Act };
  readonly runner: {
    readonly label: string;
    readonly title: string;
    /** Auto is the pick: the harness button shows Auto's mark and `pick` is Auto's preview (launcher.autoPreview, with
     * `logo`). Otherwise the harness button shows the harness's `logo`, and `pick` is its model · thinking · account
     * (launcher.model, v1 .rb-m), never the harness's name again. */
    readonly auto: boolean;
    readonly pick: string;
    /** a picked harness's `pick` in v1's parts (.rb-m/.rb-e/.rb-a; `def`: the harness's default model), so a narrow bar
     * folds the account (720px), then the effort and a default model, where the harness's name stays (520px).
     * Additive (2026-11-15). */
    readonly pickParts?: { readonly model: string; readonly def: boolean; readonly effort: string; readonly account: string };
    /** a picked formula or task file's agent runs (audit H5): its name is what runs, so a narrow bar keeps it over the
     *  model (audit H tasks note); a phone row with room shows both. Additive (2026-11-20). */
    readonly file?: boolean;
    readonly pickTitle: string;
    readonly logo?: string;
    readonly compare: string;
    /** the pill's harness button (v1 .runner-btn): ONE press that opens the harness picker; a harness's model rides in it */
    readonly act?: Act;
    /** the harness picker is open: the pill stays lit while it is */
    readonly open?: boolean;
    /** Auto's pick, its own segment of the same pill (v1 #autoPv): what will Auto run, the AutoPop */
    readonly pickAct?: Act;
    /** the AutoPop is open: the pick stays lit while it is */
    readonly pickOpen?: boolean;
    /** the pill's first hover or focus: fetch the picker's code (ADR-0067) */
    readonly onWarm?: () => void;
    readonly compareAct?: Act;
    /** the harnesses the same task also runs on (v1 launcher compare): each a "vs" that removes it, then its picker */
    readonly compares?: readonly LauncherCompare[];
    /** four runs already: no vs + */
    readonly compareFull?: boolean;
    /** Compare is in the composer's `+` menu: the pill's own + goes (its runs keep their ×). Additive (2026-11-18). */
    readonly compareInMenu?: boolean;
    /** the picked harness past 80% of a plan window: a ring round its logo, its words in the ring's tooltip (UX
     *  2026-11-18 #8, where Home's usage bar was). Additive (2026-11-18). */
    readonly usage?: { readonly pct: number; readonly tone: 'warn' | 'bad'; readonly title: string } | undefined;
  };
  /** before the runner: attach, where */
  readonly lead: readonly LauncherTool[];
  /** after the runner: try two ways, delegate */
  readonly mid: readonly LauncherTool[];
  /** after the gap: goal, worktree, later */
  readonly tail: readonly LauncherTool[];
  /** a bar too narrow for its tail folds the tail's last tools into a More menu with this name, as many as it must
   *  (design/layout.md §2, QA desktop r2 #4); none: it never folds (the phone's own More, the first run). Additive. */
  readonly tailMore?: { readonly label: string; readonly title: string } | undefined;
  readonly sendLabel: string;
  /** nothing to send yet: Send shows it, as the phone sheet's header Send does (one state, QA r2 #12). Additive. */
  readonly sendOff?: boolean;
  readonly sendAct?: Act;
  /** Goal on: its condition, under the prompt (v1 .l-goal) */
  readonly goal?: {
    readonly label: string;
    readonly placeholder: string;
    readonly value: string;
    readonly onInput: (v: string) => void;
    readonly ref?: (el: HTMLInputElement) => void;
    /** what goes at the line's end (v1 #judgesBtn: the goal plugin's Judges). Additive (2026-11-16, B44). */
    readonly end?: JSX.Element;
  };
  /** files attached, and when it starts if not now: chips under the prompt */
  readonly extras?: readonly { readonly id: string; readonly icon: IconName; readonly label: string; readonly removeLabel: string; readonly onRemove: () => void }[];
  /** Ask near the limit (v1 .l-hold): the send waits for you under the bar, Start anyway / Wait until the reset / ×;
   *  the recipes row steps aside while it shows (v1 .launcher.hold-on). Additive (2026-11-16). */
  readonly hold?: Omit<HoldRowProps, 'place' | 'box'> | undefined;
  /** no coding agent on this computer (v1 .l-setup): the install strip over the prompt. Additive (2026-11-13). */
  readonly setup?: LauncherSetupProps | undefined;
  readonly recipes: {
    readonly label: string;
    readonly empty: string;
    readonly add: string;
    readonly issue: string;
    readonly issueTitle: string;
    readonly addAct?: Act;
    readonly issueAct?: Act;
    /** the phone apps: Recipes and Start from an issue are in the bar's More menu (the shell's), so no row of their
     *  own (QA r2 #2). Additive (2026-11-18). */
    readonly inMore?: boolean;
    /** the recipes for the picked project (v1 .lp-chip.rc), a "…" on one with fields to fill. Additive (2026-11-13). */
    readonly items?: readonly LauncherRecipe[];
  };
}


/** v1 #launcher: the prompt box on Home. */
export function Launcher(props: LauncherProps): JSX.Element {
  // Try it two ways (v1 .runners.trial / .l-bar.l-try): one copy of the lead that says its change; the bar folds for it
  const trial = (): boolean => props.runner.compares?.length === 1 && (props.runner.compares[0]?.variant?.length ?? 0) > 0;
  const key = (e: KeyboardEvent): void => {
    if (suggestKeyDown(props.suggest, e)) return;
    if (props.onKey?.(e)) return;
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      props.onSend();
    }
  };
  const empty = (): boolean => props.value.trim() === '';
  // the tail as it fits: its last tools in More when the bar is too narrow for them (launcher-fold.ts)
  // which tools fold follows foldOrder (a tool whose panel is open folds last, QA r10 desktop #1); the bar keeps its order
  const fold = useTailFold({ tail: () => foldOrder(props.tail), on: () => props.tailMore !== undefined });
  // an unmeasured bar (the phone apps'): its pill's + and words fold when it doesn't fit, so More never goes under Send
  // (QA phone r4 #1; Try another agent is in More)
  const barLabels = useBarLabels();
  const tail = createMemo((): readonly LauncherTool[] => {
    const more = props.tailMore;
    // read once: the host's getter may build its tools anew on each read, and the folded set is by identity
    const all = props.tail;
    const n = all.length;
    const k = more ? Math.min(fold.folded(), n) : 0;
    if (!more || k === 0) return all;
    const out = new Set(foldOrder(all).slice(n - k));
    return [...all.filter((t) => !out.has(t)), foldTail(all.filter((t) => out.has(t)), more)];
  });
  const phone = ((look) => (): boolean => look() !== 'web')(usePlatform());
  // the phone apps with no recipes: no Recipes row, and Start from an issue is one of the bar's tools (v1 launcher.js:
  // "the phone app has it in its bar"); it sat alone on a row of its own under Send
  const issueInBar = (): boolean => phone() && (props.recipes.items?.length ?? 0) === 0 && props.recipes.inMore !== true;
  // Send with nothing written: the prompt takes the caret and its placeholder lights up until you type
  const [nudge, setNudge] = createSignal(false);
  const [over, setOver] = createSignal(false);
  let input: HTMLTextAreaElement | undefined;
  // v1 launcher.js autosize(ta): the prompt grows with what's typed (or a restored draft) up to 340 px, then scrolls
  createEffect(on(() => props.value, () => input && growTextarea(input, PROMPT_MAX)));
  const send = (): void => {
    if (props.sendAct?.pending) return;
    if (!empty()) return props.onSend();
    input?.focus();
    setNudge(true); // until the next key or the caret leaves
  };
  // a menu of its bar open (they live in a portal): it looks as it does with the caret in it (v1 :has(.open))
  const within = useOpenWithin();
  return (
    <div data-kit="sh-launch">
      <section
        data-kit="launcher"
        data-part="launcher"
        ref={within.ref}
        data-open={flag(within.open())}
        data-nudge={flag(nudge())}
        data-drop={flag(over())}
        data-first-run={flag(props.firstRun)}
        // the bar's form is measured (launcher-fold-measure.ts), not v1's 520 px rule: its icons only when its labels don't fit
        data-measure={flag(props.tailMore !== undefined)}
        data-squeeze={props.tailMore !== undefined && fold.squeeze() > 0 ? String(fold.squeeze()) : undefined}
        {...dropHandlers(() => props.onFiles, setOver)}
      >
        <div data-kit="l-projects" data-part="launcher.projects">
          <span data-kit="l-in">{props.inLabel}</span>
          <ProjectSelect {...props.place} part="launcher.place" where="launcher" />
        </div>
        <Show when={props.setup}>{(s) => <LauncherSetup {...s()} />}</Show>
        <div data-kit="mn-wrap">
          <textarea
            data-kit="prompt"
            data-part="launcher.input"
            rows="2"
            placeholder={props.placeholder}
            value={props.value}
            ref={(el) => ((input = el), props.inputRef?.(el))}
            onBlur={() => setNudge(false)}
            onInput={(e) => (setNudge(false), growTextarea(e.currentTarget, PROMPT_MAX), props.onInput(e.currentTarget.value, e))}
            onKeyDown={key}
            onPaste={(e) => props.onPaste?.(e)}
          />
          <PromptSuggestList suggest={props.suggest} place="below" />
        </div>
        <Show when={props.goal}>
          {(g) => (
            <div data-kit="l-goal" data-part="launcher.goalLine">
              {/* v1 .lg-k: the mark and "Done when" before the condition */}
              <span data-kit="lg-k">
                <Icon name="target" />
                <span data-text="control">{g().label}</span>
              </span>
              <input data-kit="l-goal-input" ref={(el) => g().ref?.(el)} aria-label={g().label} placeholder={g().placeholder} value={g().value} onInput={(e) => g().onInput(e.currentTarget.value)} />
              {g().end}
            </div>
          )}
        </Show>
        <Show when={(props.extras?.length ?? 0) > 0}>
          <div data-kit="l-extras">
            <For each={props.extras}>
              {(x) => (
                <span data-kit="lp-chip" data-extra="">
                  <Icon name={x.icon} />
                  <span data-text="control">{x.label}</span>
                  <button type="button" data-kit="chip-x" aria-label={x.removeLabel} title={x.removeLabel} onClick={() => x.onRemove()}>
                    <Icon name="x" />
                  </button>
                </span>
              )}
            </For>
          </div>
        </Show>
        <div data-kit="l-bar" ref={(el) => (fold.ref(el), barLabels.ref(el))} data-tight={flag(props.tailMore === undefined && barLabels.tight())} data-scroll={flag(fold.scroll())} data-try={flag(trial())} data-cmp={flag((props.runner.compares?.length ?? 0) > 0)}>
          <Index each={props.lead}>{(t) => <Tool t={t()} />}</Index>
          <WithAct
            act={props.mode.act}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="mode-btn" data-part="launcher.mode" title={props.mode.title}>
                <Icon name={props.mode.icon ?? 'spark'} />
                <span data-text="control">{props.mode.label}</span>
              </button>
            )}
          />
          <div data-kit="runners" data-open={flag(props.runner.open === true || props.runner.pickOpen === true || (props.runner.compares ?? []).some((c) => c.open === true))} data-cmp={flag((props.runner.compares?.length ?? 0) > 0)} data-many={flag((props.runner.compares?.length ?? 0) > 1)} data-trial={flag(trial())}>
            <WithAct
              act={props.runner.act}
              render={(trigger) => (
                <button
                  type="button"
                  {...trigger()}
                  data-kit="runner-btn"
                  data-part="launcher.harness"
                  data-lead=""
                  data-auto={flag(props.runner.auto)}
                  data-file={flag(props.runner.file)}
                  data-def={flag(!props.runner.auto && props.runner.pickParts?.def === true)}
                  aria-haspopup="dialog"
                  aria-expanded={props.runner.open ? 'true' : 'false'}
                  title={props.runner.usage ? `${props.runner.title} · ${props.runner.usage.title}` : props.runner.title}
                  aria-description={props.runner.usage?.title}
                  onPointerEnter={() => props.runner.onWarm?.()}
                  onFocus={() => props.runner.onWarm?.()}
                >
                  <span data-kit="rb-logo">
                    <Show
                      when={!props.runner.auto && props.runner.logo}
                      fallback={
                        <i data-kit="auto-mark">
                          <Icon name="spark" />
                        </i>
                      }
                    >
                      {(src) => <img src={src()} alt="" />}
                    </Show>
                    <Show when={props.runner.usage}>{(u) => <span data-kit="rb-ring" data-part="launcher.harness.usage" data-tone={u().tone} aria-hidden="true" style={{ '--ctx': `${u().pct}%` }} />}</Show>
                  </span>
                  <span data-kit="rb-h" data-text="control">
                    {props.runner.label}
                  </span>
                  {/* a harness's model · thinking · account, in its button after its name, the chevron last (v1 .rb-m … .chev) */}
                  <Show when={!props.runner.auto}>
                    <span data-kit="auto-pv" data-part="launcher.model">
                      <span data-kit="apv-l" data-text="control">
                        <Show when={props.runner.pickParts} fallback={props.runner.pick}>
                          {(p) => (
                            <>
                              <span data-kit="rb-m">{p().model}</span>
                              <Show when={p().effort}>
                                <span data-kit="rb-e">
                                  {/* its own node: a Try it two ways lead folds "Default" and keeps the thinking level (v1 .rb-e) */}
                                  <span data-kit="rb-sep"> · </span>
                                  {p().effort}
                                </span>
                              </Show>
                              <Show when={p().account}>
                                <span data-kit="rb-a">{` · ${p().account}`}</span>
                              </Show>
                            </>
                          )}
                        </Show>
                      </span>
                    </span>
                  </Show>
                  <Icon name="chevron" />
                </button>
              )}
            />
            {/* Auto's pick with its logo: the pill's own segment after "Auto" (v1 #autoPv), what will Auto run */}
            <Show when={props.runner.auto}>
              <WithAct
                act={props.runner.pickAct}
                render={(trigger) => (
                  <button
                    type="button"
                    {...trigger()}
                    data-kit="auto-pv"
                    data-part="launcher.autoPreview"
                    aria-haspopup="dialog"
                    aria-expanded={props.runner.pickOpen ? 'true' : 'false'}
                    title={props.runner.pickTitle}
                    aria-label={props.runner.pickTitle}
                  >
                    <span data-kit="rb-logo" data-small="">
                      <Show when={props.runner.logo}>{(src) => <img src={src()} alt="" />}</Show>
                    </span>
                    <span data-kit="apv-l" data-text="control">
                      {props.runner.pick}
                    </span>
                  </button>
                )}
              />
            </Show>
            <Index each={props.runner.compares ?? []}>
              {(c) => (
                <>
                  <button type="button" data-kit="vs-btn" data-del="" data-part="launcher.compare.remove" title={c().removeLabel} aria-label={c().removeLabel} onClick={() => c().onRemove()}>
                    <span>vs</span>
                    <Icon name="x" />
                  </button>
                  <WithAct
                    act={c().act}
                    render={(trigger) => (
                      <button
                        type="button"
                        {...trigger()}
                        data-kit="runner-btn"
                        data-cmp=""
                        data-vary={c().variant?.length ? '' : undefined}
                        data-part="launcher.compare.runner"
                        aria-haspopup="dialog"
                        aria-expanded={c().open ? 'true' : 'false'}
                        title={[c().label, ...(c().model ? [c().model as string] : []), ...(c().variant ?? [])].join(' · ')}
                      >
                        <span data-kit="rb-logo" data-small="">
                          <Show when={c().logo}>{(src) => <img src={src()} alt="" />}</Show>
                        </span>
                        <span data-kit="rb-h" data-text="control">
                          {c().label}
                        </span>
                        <Show when={c().model}>
                          {(m) => (
                            <span data-kit="rb-v" data-text="control" data-part="launcher.compare.model">
                              {m()}
                            </span>
                          )}
                        </Show>
                        <For each={c().variant ?? []}>
                          {(w) => (
                            <span data-kit="rb-v" data-text="control" data-part="launcher.compare.variant">
                              {w}
                            </span>
                          )}
                        </For>
                        <Icon name="chevron" />
                      </button>
                    )}
                  />
                </>
              )}
            </Index>
            <WithAct
              act={props.runner.compareAct}
              render={(trigger) => (
                <button
                  type="button"
                  {...trigger()}
                  data-kit="vs-btn"
                  data-part="launcher.compare"
                  hidden={props.runner.compareFull === true || props.runner.compareInMenu === true}
                  title={props.runner.compare}
                  aria-label={props.runner.compare}
                >
                  <Icon name="plus" />
                </button>
              )}
            />
          </div>
          <Index each={props.mid}>{(t) => <Tool t={t()} />}</Index>
          <span data-kit="grow" />
          {/* by tool, not by slot: a chip keeps its node (an open Delegate panel, its field's caret) as others come and go */}
          <ForKeyed each={tail()}>{(t) => <Tool t={t()} />}</ForKeyed>
          <Show when={issueInBar()}>
            <Tool t={{ id: 'issue', part: 'launcher.issue', icon: 'issue', label: props.recipes.issue, title: props.recipes.issueTitle, ...(props.recipes.issueAct && { act: props.recipes.issueAct }) }} />
          </Show>
          <button type="button" {...actAttrs(props.sendAct)} data-kit="send" data-part="launcher.send" title={props.sendLabel} aria-label={props.sendLabel} disabled={props.sendOff === true} onClick={send}>
            <ActIcon act={props.sendAct}>
              <Icon name="enter" />
            </ActIcon>
          </button>
        </div>
        <Show when={props.hold}>{(h) => <HoldRow {...h()} place="launcher" box={() => input} part="launcher.hold" />}</Show>
        <div data-kit="l-foot" hidden={props.hold !== undefined}>
          <div data-kit="l-recipes" data-part="launcher.recipes" hidden={issueInBar() || props.recipes.inMore === true}>
            {/* v1 responsive.css: an empty Recipes list is an ad for a feature, not worth a phone's screen; the phone apps
                hide it by data-empty and keep Start from an issue (v1 moved it to the bar there); the browser's HTML stays v1's */}
            <div
              data-kit="lr-list"
              data-part="launcher.recipes-wrap"
              data-layout={(props.recipes.items?.length ?? 0) > 0 ? 'wrap' : undefined}
              data-empty={phone() && (props.recipes.items?.length ?? 0) === 0 ? '' : undefined}
            >
              <span data-kit="l-in" data-icon="">
                <Icon name="bolt" />
                {props.recipes.label}
              </span>
              <For each={props.recipes.items} fallback={<span data-kit="lr-empty" title={props.recipes.empty}>{props.recipes.empty}</span>}>
                {(r) => (
                  <WithAct
                    act={r.act}
                    render={(trigger) => (
                      <button type="button" {...trigger()} data-kit="lp-chip" data-small="" data-recipe="" data-part="launcher.recipe" title={r.title}>
                        <span data-text="control">{r.label}</span>
                        <Show when={r.fields}>
                          <i data-kit="rc-f">…</i>
                        </Show>
                      </button>
                    )}
                  />
                )}
              </For>
              <WithAct
                act={props.recipes.addAct}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="lp-chip" data-add="" data-small="">
                    <Icon name="plus" />
                    {props.recipes.add}
                  </button>
                )}
              />
            </div>
            <Show when={!issueInBar()}>
              <WithAct
                act={props.recipes.issueAct}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="lp-chip" data-small="" data-part="launcher.issue" title={props.recipes.issueTitle} aria-label={props.recipes.issueTitle}>
                    <Icon name="issue" />
                    <span data-text="control">{props.recipes.issue}</span>
                  </button>
                )}
              />
            </Show>
          </div>
        </div>
      </section>
    </div>
  );
}

export interface NextRow {
  readonly id: string;
  readonly title: string;
  readonly color: string;
  readonly state?: string;
  /** the status's tone, for its colour (good, accent, warn, bad…) */
  readonly tone?: string;
  /** why it needs you, in one line: the question it asks, else the start of its answer (TRY-27) */
  readonly why?: string;
  readonly time: string;
  readonly timeTitle: string;
}

/** v1 .sh-next: what needs you, up to five, each opening its task. */
export function NextList(props: {
  readonly heading: string;
  readonly count: number;
  /** said in place of the count when fewer rows show than it counts (v1 paintHome "5 of 9"). Additive (2026-11-17). */
  readonly countLabel?: string | undefined;
  readonly rows: readonly NextRow[];
  readonly onOpen: (id: string) => void;
}): JSX.Element {
  // nothing needs you: no section at all (v1 paintHome: .sh-next hidden when it has no rows)
  return (
    <Show when={props.rows.length > 0}>
      <div data-kit="sh-next" data-part="split.next">
        <h4>
          {props.heading}
          <span data-kit="sec-n">{props.countLabel ?? props.count}</span>
        </h4>
        {/* by task, each row made once (the flicker gate: they were made again on every change) */}
        <ForKeyed each={props.rows}>
          {(r) => (
            <button type="button" data-kit="sh-task" data-color={r().color} data-id={r().id} onClick={() => props.onOpen(r().id)}>
              <span data-kit="q-dot" />
              <span data-kit="sh-main">
                <span data-kit="sh-t" data-text="control" title={r().title}>
                  {r().title}
                </span>
                <Show when={r().why}>
                  <span data-kit="sh-why" title={r().why}>
                    {r().why}
                  </span>
                </Show>
              </span>
              <Show when={r().state}>
                <span data-kit="sh-s" data-tone={r().tone}>
                  {r().state}
                </span>
              </Show>
              <span data-kit="sh-time" title={r().timeTitle}>
                {r().time}
              </span>
            </button>
          )}
        </ForKeyed>
      </div>
    </Show>
  );
}

export interface SetupTip {
  readonly kind: 'tool' | 'grid';
  readonly icon: IconName;
  readonly title: string;
  readonly body: string;
  readonly cta: string;
  readonly onClick?: () => void;
}

export interface PlainTip {
  /** the text, split around its bold words */
  readonly before: string;
  readonly bold: string;
  readonly after: string;
  readonly href?: string;
}

/** v1 .sh-tips: setup cards, then one-line tips. */
export function Tips(props: {
  readonly heading: string;
  readonly more: string;
  readonly moreTitle: string;
  readonly setup: readonly SetupTip[];
  readonly tips: readonly PlainTip[];
  readonly onMore?: () => void;
  readonly onOpen?: (href: string) => void;
}): JSX.Element {
  return (
    <div data-kit="sh-tips" data-part="tips">
      <h4>
        {props.heading}
        <button type="button" data-kit="tip-more" title={props.moreTitle} onClick={() => props.onMore?.()}>
          {props.more}
        </button>
      </h4>
      <For each={props.setup}>
        {(s) => (
          <button type="button" data-kit="sh-tip" data-setup="" data-part={s.kind === 'grid' ? 'tips.setup tips.grid' : 'tips.setup tips.tools'} onClick={() => s.onClick?.()}>
            <span data-kit="ts-badge">
              <Icon name={s.icon} />
            </span>
            <span data-kit="ts-text">
              <b>{s.title}</b>
              <small>{s.body}</small>
            </span>
            <span data-kit="ts-cta">
              {s.cta}
              <Icon name="arrow" />
            </span>
          </button>
        )}
      </For>
      <For each={props.tips}>
        {(t) => (
          <a
            data-kit="sh-tip"
            href={t.href}
            target="_blank"
            rel="noopener"
            onClick={(e) => {
              if (!props.onOpen || !t.href) return;
              e.preventDefault();
              props.onOpen(t.href);
            }}
          >
            <Icon name="spark" />
            <span>
              {t.before}
              <b>{t.bold}</b>
              {t.after}
            </span>
            <Icon name="external" />
          </a>
        )}
      </For>
    </div>
  );
}

/** v1 .sh-foot .sh-fb: bugs and ideas for the wisp team, a quiet line at the foot of home. */
export function FeedbackLine(props: { readonly label: string; readonly onClick: () => void }): JSX.Element {
  return (
    <p data-kit="sh-foot">
      <button type="button" data-kit="sh-fb" data-part="home.feedback" onClick={() => props.onClick()}>
        <Icon name="bug" />
        {props.label}
      </button>
    </p>
  );
}

/** v1 .sh-keys: the list's keys, in a line under the tips (not on a phone). */
export function KeysHint(props: { readonly items: readonly { readonly keys: readonly string[]; readonly label: string }[] }): JSX.Element {
  return (
    <p data-kit="sh-keys">
      <For each={props.items}>
        {(it, i) => (
          <>
            <For each={it.keys}>{(k) => <kbd>{k}</kbd>}</For>
            {` ${it.label}${i() < props.items.length - 1 ? ' · ' : ''}`}
          </>
        )}
      </For>
    </p>
  );
}
