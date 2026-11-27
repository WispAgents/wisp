// A crew member's editor (design/crew.md, ADR-0141): a hero (the spirit in a glow of its colour, the dice, the name
// typed large, the role, the @handle), then four tabs: Who (Start from, the spirit maker, the task panel's tint), Their
// job, When (Checks in, Also starts when, First check-in now) and Where & how (the bar: where, project, harness,
// delegates, mode, how many at once), and a foot that stays in view (Back, Cancel, Next, the action). Every tab can be
// skipped: the action works from any of them once there's a name. The caller owns the draft; this draws it.
import { createSignal, For, Show, useContext, type JSX } from 'solid-js';
import { flag, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { WithAct, ActIcon, type Act } from '../app/act.tsx';
import { Face, faceColor } from './Face.tsx';
import { Spirit } from './Spirit.tsx';
import { SpiritMaker, type SpiritMakerProps } from './Maker.tsx';
import { SurfaceTitle } from '../controls/surface-title.ts';
import '../css/crew-editor.css';

export interface CrewEditorTemplate {
  readonly key: string;
  readonly label: string;
  readonly avatar: string;
}

export interface CrewEditorChip {
  readonly value: string;
  readonly label: string;
  readonly icon?: IconName;
}

export interface CrewEditorBarItem {
  readonly id: string;
  /** tog: a plain toggle (where, project, delegates); runner: harness and model; mode: what they may do */
  readonly kind: 'tog' | 'runner' | 'mode';
  readonly icon: IconName;
  readonly label: string;
  /** the runner's second word (Default) */
  readonly note?: string;
  readonly title?: string;
  /** the mode's glyph in warn (Full auto) or accent (Plan) */
  readonly tone?: 'warn' | 'accent' | undefined;
  readonly act: Act;
}

export interface CrewEditorFormProps {
  readonly start?: { readonly label: string; readonly templates: readonly CrewEditorTemplate[]; readonly picked: string | undefined; readonly onPick: (key: string) => void; readonly import?: { readonly label: string; readonly title: string; readonly onClick: () => void } | undefined } | undefined;
  readonly face: { readonly avatar: string; readonly name: string; readonly color?: string; readonly title: string; readonly onRoll: () => void };
  /** the spirit maker under the face (v1 .ce-maker), while it's open */
  readonly maker?: SpiritMakerProps | undefined;
  /** `focus`: the caret starts here (a new member: v1 put it in Name as the dialog opened) */
  readonly name: { readonly label: string; readonly value: string; readonly placeholder: string; readonly onInput: (v: string) => void; readonly focus?: boolean };
  readonly role: { readonly label: string; readonly value: string; readonly placeholder: string; readonly onInput: (v: string) => void };
  /** under the names: how to reach them, or what's wrong with the name */
  readonly handle: { readonly text: string; readonly code?: string; readonly after?: string; readonly bad?: boolean };
  readonly job: { readonly label: string; readonly hint: string; readonly value: string; readonly placeholder: string; readonly onInput: (v: string) => void };
  readonly cadence: { readonly label: string; readonly hint: string; readonly chips: readonly CrewEditorChip[]; readonly value: string; readonly onPick: (v: string) => void; readonly custom: { readonly value: string; readonly placeholder: string; readonly read: string; readonly bad: boolean; readonly onInput: (v: string) => void } | undefined };
  readonly events: { readonly label: string; readonly hint: string; readonly chips: readonly CrewEditorChip[]; readonly on: readonly string[]; readonly onToggle: (v: string) => void };
  /** the task panel's tint while they're picked on the rail (v1 .ce-tint): their glow, none, or a crew colour */
  readonly tint?: {
    readonly label: string;
    readonly hint: string;
    readonly glow: { readonly color: string; readonly label: string };
    readonly none: string;
    readonly colors: readonly string[];
    readonly value: string;
    readonly onPick: (v: string) => void;
  } | undefined;
  readonly bar: readonly CrewEditorBarItem[];
  readonly parallel: { readonly label: string; readonly title: string; readonly value: number; readonly less: string; readonly more: string; readonly onStep: (by: number) => void };
  readonly checkin?: { readonly label: string; readonly on: boolean; readonly onToggle: (on: boolean) => void } | undefined;
  /** the tabs' names, Back, Next (`next(step)`: Next: When) and the hero's × */
  readonly steps: { readonly who: string; readonly job: string; readonly when: string; readonly where: string; readonly back: string; readonly next: (step: string) => string; readonly close: string };
  readonly cancel: { readonly label: string; readonly onClick: () => void };
  readonly submit: { readonly label: string; readonly onClick: () => void };
  readonly part?: PartName;
}

function Chips(props: { readonly chips: readonly CrewEditorChip[]; readonly on: (v: string) => boolean; readonly onPick: (v: string) => void; readonly pressed?: boolean }): JSX.Element {
  return (
    <For each={props.chips}>
      {(c) => (
        <button type="button" data-kit="lp-chip" data-on={flag(props.on(c.value))} aria-pressed={props.pressed ? props.on(c.value) : undefined} onClick={() => props.onPick(c.value)}>
          <Show when={c.icon}>{(i) => <Icon name={i()} size="sm" />}</Show>
          {c.label}
        </button>
      )}
    </For>
  );
}

type Step = 'who' | 'job' | 'when' | 'where';
const STEPS: readonly Step[] = ['who', 'job', 'when', 'where'];

// size-ok: one form, its hero, four tabs and the foot
export function CrewEditorForm(props: CrewEditorFormProps): JSX.Element {
  const [step, setStep] = createSignal<Step>('who');
  const at = (): number => STEPS.indexOf(step());
  const title = useContext(SurfaceTitle);
  const submit = (e: SubmitEvent): void => {
    e.preventDefault();
    props.submit.onClick();
  };
  // a tab is ticked once it says something: a name, a job, a cadence or an event other than the first (only when asked)
  const done = (s: Step): boolean =>
    s === 'who' ? props.name.value.trim() !== '' : s === 'job' ? props.job.value.trim() !== '' : s === 'when' ? props.events.on.length > 0 || (props.cadence.value !== '' && props.cadence.value !== props.cadence.chips[0]?.value) : false;
  const label = (s: Step): string => props.steps[s];
  const go = (by: number): void => {
    const next = STEPS[at() + by];
    if (next) setStep(next);
  };
  return (
    <div data-kit="ce" data-part={props.part ?? 'crew.editor'}>
      <form data-kit="ce-form" onSubmit={submit}>
        <header data-kit="ce-hero" data-color={faceColor(props.face.avatar, props.face.name, props.face.color)} data-part="crew.editor.hero">
          <Show when={title?.()}>{(tt) => <span data-kit="ce-eyebrow">{tt()}</span>}</Show>
          <button type="button" data-kit="icon-button" data-variant="ghost" data-size="sm" data-a="close" aria-label={props.steps.close} title={props.steps.close} onClick={() => props.cancel.onClick()}>
            <Icon name="x" size="md" />
          </button>
          <button
            type="button"
            data-kit="ce-face"
            title={props.face.title}
            aria-label={props.face.title}
            onClick={() => {
              setStep('who');
              props.face.onRoll();
            }}
          >
            <Face avatar={props.face.avatar} name={props.face.name} {...(props.face.color !== undefined && { color: props.face.color })} size="xl" />
            <span data-kit="ce-dice">
              <Icon name="rerun" size="sm" />
            </span>
          </button>
          <input
            data-kit="ce-name"
            data-part="crew.editor.name"
            name="name"
            aria-label={props.name.label}
            ref={(el) => props.name.focus && queueMicrotask(() => el.focus({ preventScroll: true }))}
            maxlength="24"
            value={props.name.value}
            placeholder={props.name.placeholder}
            spellcheck={false}
            autocomplete="off"
            required
            onInput={(e) => props.name.onInput(e.currentTarget.value)}
          />
          <input data-kit="ce-role" data-part="crew.editor.role" name="role" aria-label={props.role.label} maxlength="60" value={props.role.value} placeholder={props.role.placeholder} spellcheck={false} autocomplete="off" onInput={(e) => props.role.onInput(e.currentTarget.value)} />
          <p data-kit="ce-handle" data-bad={flag(props.handle.bad === true)} aria-live="polite">
            <Icon name={props.handle.bad ? 'warn' : 'user'} size="sm" />
            <span>
              {props.handle.text}
              <Show when={props.handle.code}>{(c) => <code>{c()}</code>}</Show>
              {props.handle.after}
            </span>
          </p>
        </header>
        <nav data-kit="ce-tabs" role="tablist" data-part="crew.editor.tabs">
          <For each={STEPS}>
            {(s, i) => (
              <button type="button" role="tab" data-kit="ce-tab" data-a={s} aria-selected={step() === s} data-on={flag(step() === s)} data-done={flag(done(s) && step() !== s)} onClick={() => setStep(s)}>
                <span data-kit="ce-num">
                  <Show when={done(s) && step() !== s} fallback={i() + 1}>
                    <Icon name="check" size="sm" />
                  </Show>
                </span>
                <span>{label(s)}</span>
              </button>
            )}
          </For>
        </nav>
        <div data-kit="ce-pane" role="tabpanel" aria-label={label(step())} data-step={step()}>
          <Show when={step() === 'who'}>
            <Show when={props.start}>
              {(st) => (
                <div data-kit="ce-stack" data-part="crew.editor.templates">
                  <span>{st().label}</span>
                  <div data-kit="ce-templates">
                    <For each={st().templates}>
                      {(tp) => (
                        <button type="button" data-kit="ce-tpl" data-on={flag(st().picked === tp.key)} aria-pressed={st().picked === tp.key} onClick={() => st().onPick(tp.key)}>
                          <span data-kit="ce-tav">
                            <Spirit code={tp.avatar} small />
                          </span>
                          <span>{tp.label}</span>
                        </button>
                      )}
                    </For>
                    <Show when={st().import}>
                      {(im) => (
                        <button type="button" data-kit="ce-tpl" data-import="" title={im().title} onClick={() => im().onClick()}>
                          <span data-kit="ce-tav">
                            <Icon name="download" size="md" />
                          </span>
                          <span>{im().label}</span>
                        </button>
                      )}
                    </Show>
                  </div>
                </div>
              )}
            </Show>
            <Show when={props.maker}>{(mk) => <SpiritMaker {...mk()} />}</Show>
            <Show when={props.tint}>
              {(ti) => (
                <div data-kit="ce-stack" data-part="crew.editor.tint">
                  <span>
                    {ti().label} <small>{ti().hint}</small>
                  </span>
                  <div data-kit="ce-tints" data-layout="wrap" role="group" aria-label={ti().label}>
                    <button type="button" data-kit="ce-swatch" data-on={flag(ti().value === '')} aria-pressed={ti().value === ''} title={ti().glow.label} aria-label={ti().glow.label} onClick={() => ti().onPick('')}>
                      <i data-kit="ce-sw" data-color={ti().glow.color} />
                    </button>
                    <button type="button" data-kit="ce-swatch" data-none="" data-on={flag(ti().value === 'none')} aria-pressed={ti().value === 'none'} title={ti().none} aria-label={ti().none} onClick={() => ti().onPick('none')}>
                      <Icon name="x" size="sm" />
                    </button>
                    <For each={ti().colors}>
                      {(col) => (
                        <button type="button" data-kit="ce-swatch" data-on={flag(ti().value === col)} aria-pressed={ti().value === col} title={col} aria-label={col} onClick={() => ti().onPick(col)}>
                          <i data-kit="ce-sw" data-color={col} />
                        </button>
                      )}
                    </For>
                  </div>
                </div>
              )}
            </Show>
          </Show>
          <Show when={step() === 'job'}>
            <label data-kit="ce-stack" data-part="crew.editor.brief">
              <span>
                {props.job.label} <small>{props.job.hint}</small>
              </span>
              <textarea data-kit="ce-textarea" data-big="" name="brief" rows="9" maxlength="4000" value={props.job.value} placeholder={props.job.placeholder} onInput={(e) => props.job.onInput(e.currentTarget.value)} />
            </label>
          </Show>
          <Show when={step() === 'when'}>
            <div data-kit="ce-stack" data-part="crew.editor.cron">
              <span>
                <Icon name="clock" size="sm" />
                {props.cadence.label} <small>{props.cadence.hint}</small>
              </span>
              <div data-kit="ce-cadence" data-layout="wrap">
                <Chips chips={props.cadence.chips} on={(v) => v === props.cadence.value} onPick={(v) => props.cadence.onPick(v)} />
              </div>
              <Show when={props.cadence.custom}>
                {(c) => (
                  <div data-kit="ce-cron" data-layout="wrap">
                    <input data-kit="ce-input" name="cron" value={c().value} placeholder={c().placeholder} spellcheck={false} autocomplete="off" onInput={(e) => c().onInput(e.currentTarget.value)} />
                    <span data-kit="ce-cron-read" data-bad={flag(c().bad)} aria-live="polite">
                      {c().read}
                    </span>
                  </div>
                )}
              </Show>
            </div>
            <div data-kit="ce-stack" data-part="crew.editor.events">
              <span>
                <Icon name="bolt" size="sm" />
                {props.events.label} <small>{props.events.hint}</small>
              </span>
              <div data-kit="ce-cadence" data-layout="wrap" data-on-chips="">
                <Chips chips={props.events.chips} on={(v) => props.events.on.includes(v)} onPick={(v) => props.events.onToggle(v)} pressed />
              </div>
            </div>
            <Show when={props.checkin}>
              {(c) => (
                <label data-kit="ce-check">
                  <input type="checkbox" name="checkin" checked={c().on} onChange={(e) => c().onToggle(e.currentTarget.checked)} /> {c().label}
                </label>
              )}
            </Show>
          </Show>
          <Show when={step() === 'where'}>
            <div data-kit="ce-bar" data-layout="wrap" data-part="crew.editor.bar">
              <For each={props.bar}>
                {(b) => (
                  <WithAct
                    act={b.act}
                    render={(trigger) => (
                      <button type="button" data-kit={b.kind === 'mode' ? 'ce-mode' : b.kind === 'runner' ? 'ce-runner' : 'ce-tog'} data-tone={b.tone} data-a={b.id} title={b.title} {...trigger()}>
                        <ActIcon act={b.act}>
                          <Icon name={b.icon} size="sm" />
                        </ActIcon>
                        <span>{b.label}</span>
                        <Show when={b.note}>{(n) => <small>{n()}</small>}</Show>
                        <Show when={b.kind === 'runner'}>
                          <Icon name="chevron" size="sm" />
                        </Show>
                      </button>
                    )}
                  />
                )}
              </For>
              <span data-kit="ce-par" title={props.parallel.title}>
                <button type="button" aria-label={props.parallel.less} disabled={props.parallel.value <= 1} onClick={() => props.parallel.onStep(-1)}>
                  <Icon name="minus" size="sm" />
                </button>
                <b>{props.parallel.label}</b>
                <button type="button" aria-label={props.parallel.more} disabled={props.parallel.value >= 5} onClick={() => props.parallel.onStep(1)}>
                  <Icon name="plus" size="sm" />
                </button>
              </span>
            </div>
          </Show>
        </div>
        <footer data-kit="ce-foot" data-part="crew.editor.foot">
          <Show when={at() > 0}>
            <button type="button" data-kit="button" data-variant="ghost" data-size="sm" data-a="back" onClick={() => go(-1)}>
              <Icon name="back" size="sm" />
              {props.steps.back}
            </button>
          </Show>
          <span data-kit="ce-grow" />
          <button type="button" data-kit="button" data-variant="ghost" data-size="sm" data-a="cancel" onClick={() => props.cancel.onClick()}>
            {props.cancel.label}
          </button>
          <Show when={at() < STEPS.length - 1}>
            <button type="button" data-kit="button" data-variant="secondary" data-size="sm" data-a="next" aria-label={props.steps.next(label(STEPS[at() + 1] ?? 'where'))} onClick={() => go(1)}>
              <span>{props.steps.next(label(STEPS[at() + 1] ?? 'where'))}</span>
              <Icon name="arrow" size="sm" />
            </button>
          </Show>
          <button type="submit" data-kit="button" data-variant="primary" data-size="sm">
            <Icon name="check" size="sm" />
            {props.submit.label}
          </button>
        </footer>
      </form>
    </div>
  );
}
