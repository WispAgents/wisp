// Automations (v1 features/automations.js openAutomations, automations.css): the lede, Recipes · Triggers with their
// counts, then the tab's list (each item an icon, its name, a line, tags and actions), "+ New …" and a hint; or a
// form: Starts when (the trigger kinds in groups behind one field, owner 2026-11-28), then rows of a label and its control (.au-form), the When presets
// with what the box will do read back, a warning, and Cancel / Create. Value for value; the owner draws the data.
import { createMemo, createSignal, createUniqueId, For, Index, Show, type JSX } from 'solid-js';
import { flag, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Popover } from '../controls/Popover.tsx';
import '../css/menu-filter.css';
import '../css/app-automations.css';

/** words with `{ code }` runs, as v1 wrote `<code>{name}</code>` in its lines */
export type Rich = readonly (string | { readonly code: string })[];

function RichText(props: { readonly text: Rich }): JSX.Element {
  return <Index each={props.text}>{(x) => (typeof x() === 'string' ? (x() as string) : <code>{(x() as { code: string }).code}</code>)}</Index>;
}

export interface AutomationsTab {
  readonly value: string;
  readonly label: string;
  readonly count: number;
}

export function AutomationsBody(props: {
  readonly lede: Rich;
  readonly tabs: readonly AutomationsTab[];
  readonly tab: string;
  readonly onTab: (v: string) => void;
  readonly children?: JSX.Element;
}): JSX.Element {
  return (
    <div data-kit="au-body" data-part="automations.modal.body">
      <p data-kit="au-lede">
        <RichText text={props.lede} />
      </p>
      <div data-kit="au-tabs" role="tablist" data-part="automations.tabs">
        <For each={props.tabs}>
          {(x) => (
            <button type="button" role="tab" aria-selected={props.tab === x.value} data-on={flag(props.tab === x.value)} data-value={x.value} onClick={() => props.onTab(x.value)}>
              {x.label}
              <span>{x.count}</span>
            </button>
          )}
        </For>
      </div>
      {props.children}
    </div>
  );
}

export interface AutoItem {
  readonly id: string;
  readonly icon: IconName;
  readonly name: string;
  readonly line: Rich;
  readonly tags?: readonly string[];
  readonly off?: boolean;
  readonly actions?: JSX.Element;
}

export function AutoList(props: {
  readonly items: readonly AutoItem[];
  readonly empty: string;
  readonly add: { readonly label: string; readonly onClick: () => void };
  readonly hint?: Rich;
}): JSX.Element {
  return (
    <>
      <div data-kit="au-list" data-part="automations.list">
        <Show when={props.items.length > 0} fallback={<p data-kit="au-empty">{props.empty}</p>}>
          <For each={props.items}>
            {(it) => (
              <div data-kit="au-item" data-off={flag(it.off)} data-part="automations.item">
                <span data-kit="au-ico">
                  <Icon name={it.icon} />
                </span>
                <div data-kit="au-main">
                  <b>{it.name}</b>
                  <p>
                    <RichText text={it.line} />
                  </p>
                  <Show when={(it.tags?.length ?? 0) > 0}>
                    <span data-kit="au-tags" data-layout="wrap">
                      <For each={it.tags}>{(t) => <i>{t}</i>}</For>
                    </span>
                  </Show>
                </div>
                <Show when={it.actions}>
                  <span data-kit="au-acts">{it.actions}</span>
                </Show>
              </div>
            )}
          </For>
        </Show>
      </div>
      <button type="button" data-kit="au-add" data-part="automations.new" onClick={() => props.add.onClick()}>
        <Icon name="plus" />
        {props.add.label}
      </button>
      <Show when={props.hint}>
        {(h) => (
          <p data-kit="au-hint">
            <RichText text={h()} />
          </p>
        )}
      </Show>
    </>
  );
}

export interface TriggerKind {
  /** the option's own key (a kind, or kind:preset) */
  readonly key: string;
  readonly label: string;
  readonly hint: string;
  readonly icon: IconName;
}

/** a heading over some kinds (On GitHub, From an app) */
export interface TriggerKindGroup {
  readonly id: string;
  readonly label: string;
  readonly kinds: readonly TriggerKind[];
}

export interface TriggerKindsProps {
  readonly groups: readonly TriggerKindGroup[];
  readonly value: string;
  readonly onPick: (key: string) => void;
  /** the search box's placeholder and name, and the line shown when nothing matches */
  readonly search: string;
  readonly empty: string;
}

/** what a typed query keeps: each word in the label, the hint or the group's name */
export function matchKinds(groups: readonly TriggerKindGroup[], query: string): TriggerKindGroup[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [...groups];
  return groups
    .map((g) => ({ ...g, kinds: g.kinds.filter((k) => words.every((w) => `${k.label} ${k.hint} ${g.label}`.toLowerCase().includes(w))) }))
    .filter((g) => g.kinds.length > 0);
}

/** Starts when (owner 2026-11-28, design B; it replaced v1's .au-kinds tiles): one field showing the picked kind, its
 *  icon and hint, that opens the kinds in groups under a search box. Typing narrows them, ↑ ↓ move, Enter picks, Escape
 *  closes; the focus stays in the box (aria-activedescendant), as a combobox's. */
export function TriggerKinds(props: TriggerKindsProps): JSX.Element {
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal('');
  const [active, setActive] = createSignal('');
  const id = createUniqueId();
  const all = createMemo(() => props.groups.flatMap((g) => g.kinds));
  const picked = (): TriggerKind | undefined => all().find((k) => k.key === props.value) ?? all()[0];
  const shown = createMemo(() => matchKinds(props.groups, query()));
  const flat = createMemo(() => shown().flatMap((g) => g.kinds));
  const optId = (key: string): string => `${id}-${key.replace(/[^a-z0-9]/gi, '-')}`;
  // what Enter takes: the lit row while it's still shown, else the first match
  const lit = (): string | undefined => (flat().some((k) => k.key === active()) ? active() : flat()[0]?.key);
  const openChange = (o: boolean): void => {
    setOpen(o);
    if (!o) return;
    setQuery('');
    setActive(props.value);
  };
  const pick = (key: string): void => {
    props.onPick(key);
    setOpen(false);
  };
  const move = (by: number): void => {
    const xs = flat();
    if (!xs.length) return;
    const at = xs.findIndex((k) => k.key === lit());
    const next = xs[(at + by + xs.length) % xs.length];
    if (!next) return;
    setActive(next.key);
    document.getElementById(optId(next.key))?.scrollIntoView({ block: 'nearest' });
  };
  const keys = (e: KeyboardEvent): void => {
    if (e.isComposing) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      move(e.key === 'ArrowDown' ? 1 : -1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const k = lit();
      if (k) pick(k);
    }
  };
  return (
    <Popover
      trigger={{ label: picked()?.label ?? '' }}
      part="automations.kinds"
      cardPart="automations.kinds.menu"
      placement="bottom-start"
      bare
      open={open()}
      onOpenChange={openChange}
      vars={{ '--popover-w': 'var(--au-kinds-w)' }}
      render={(trigger) => (
        <button {...trigger()} type="button" data-kit="au-kind" data-part="automations.kinds" aria-haspopup="listbox" data-on={flag(open())}>
          <Show when={picked()}>
            {(k) => (
              <>
                <Icon name={k().icon} />
                <span data-kit="au-kind-text">
                  <b data-text="control">{k().label}</b>
                  <small data-text="control">{k().hint}</small>
                </span>
              </>
            )}
          </Show>
          <Icon name="chevron" />
        </button>
      )}
    >
      <div data-kit="menu-filter" data-part="automations.kinds.filter">
        <Icon name="search" size="sm" tone="faint" />
        <input
          ref={(el) => requestAnimationFrame(() => el.focus({ preventScroll: true }))}
          type="search"
          role="combobox"
          aria-expanded="true"
          aria-controls={`${id}-list`}
          aria-activedescendant={lit() ? optId(lit() as string) : undefined}
          aria-label={props.search}
          placeholder={props.search}
          value={query()}
          autocomplete="off"
          spellcheck={false}
          onInput={(e) => setQuery(e.currentTarget.value)}
          onKeyDown={keys}
        />
      </div>
      <div id={`${id}-list`} role="listbox" aria-label={props.search} data-kit="au-kind-list" data-part="automations.kinds.list">
        <For each={shown()} fallback={<p data-kit="au-kind-empty">{props.empty}</p>}>
          {(g) => (
            <div role="group" aria-labelledby={`${id}-g-${g.id}`} data-kit="au-kind-group">
              <span id={`${id}-g-${g.id}`} data-kit="menu-group-label">
                {g.label}
              </span>
              <For each={g.kinds}>
                {(k) => (
                  <div
                    id={optId(k.key)}
                    role="option"
                    aria-selected={k.key === props.value}
                    title={k.hint}
                    data-kit="menu-item"
                    data-kind={k.key}
                    data-highlighted={flag(k.key === lit())}
                    data-on={flag(k.key === props.value)}
                    onPointerMove={() => setActive(k.key)}
                    // the box keeps the focus: a press on a row picks it without blurring the search
                    onPointerDown={(e) => e.preventDefault()}
                    onClick={() => pick(k.key)}
                  >
                    <Icon name={k.icon} />
                    <span data-kit="menu-item-label">{k.label}</span>
                  </div>
                )}
              </For>
            </div>
          )}
        </For>
      </div>
    </Popover>
  );
}

/** v1 .au-form > label: the label (92px) and its control on one row, a line under; `stack`: the label over it */
export function AutoRow(props: { readonly label: JSX.Element; readonly stack?: boolean; readonly part?: string; readonly children: JSX.Element }): JSX.Element {
  return (
    <div data-kit="au-row" data-stack={flag(props.stack)} data-part={props.part ?? 'automations.row'}>
      <span data-kit="au-label">{props.label}</span>
      {props.children}
    </div>
  );
}

/** v1 .au-presets: the schedule's chips under When (a group that wraps, as v1's), and what the box will do read back */
export function AutoPresets(props: {
  readonly items: readonly string[];
  readonly onPick: (v: string) => void;
  readonly read?: { readonly text: string; readonly bad?: boolean } | undefined;
}): JSX.Element {
  return (
    <p data-kit="au-presets" data-layout="wrap" data-part="automations.presets">
      <For each={props.items}>
        {(w) => (
          <button type="button" data-kit="lp-chip" onClick={() => props.onPick(w)}>
            {w.charAt(0).toUpperCase() + w.slice(1)}
          </button>
        )}
      </For>
      <span data-kit="au-read" data-bad={flag(props.read?.bad)} aria-live="polite">
        {props.read?.text ?? ''}
      </span>
    </p>
  );
}

export function AutoNote(props: { readonly tone: 'warn' | 'muted'; readonly children: JSX.Element }): JSX.Element {
  return (
    <p data-kit={props.tone === 'warn' ? 'au-warn' : 'au-note'}>
      <Show when={props.tone === 'warn'}>
        <Icon name="warn" />
      </Show>
      <span>{props.children}</span>
    </p>
  );
}

export function AutoFoot(props: { readonly cancel: string; readonly ok: string; readonly busy?: boolean; readonly onCancel: () => void; readonly onOk: () => void }): JSX.Element {
  return (
    <div data-kit="au-foot" data-part="automations.foot">
      <button type="button" data-kit="au-cancel" onClick={() => props.onCancel()}>
        {props.cancel}
      </button>
      <button type="button" data-kit="au-ok" data-part="automations.create" aria-busy={props.busy ? 'true' : undefined} onClick={() => props.onOk()}>
        {props.ok}
      </button>
    </div>
  );
}

/** v1 .au-runner: the runner button (logo, harness, model, chevron; v1 paintRunnerBtn) and Use launcher default */
export function AutoRunner(props: {
  readonly harness: string;
  readonly model?: string | undefined;
  readonly logo?: string | undefined;
  readonly title: string;
  readonly onClick: () => void;
  readonly reset?: { readonly label: string; readonly onClick: () => void } | undefined;
  /** another owner's name for it (a goal judge's runner). Additive (2026-11-17). */
  readonly part?: PartName;
  /** the button, for the picker that opens under it. Additive (2026-11-17). */
  readonly ref?: (el: HTMLButtonElement) => void;
}): JSX.Element {
  return (
    <div data-kit="au-runner" data-part={props.part ?? 'automations.runner'}>
      <button type="button" data-kit="au-runner-btn" ref={(el) => props.ref?.(el)} aria-haspopup="dialog" title={props.title} onClick={() => props.onClick()}>
        <span data-kit="rb-logo">
          <Show when={props.logo} fallback={<Icon name="spark" />}>
            {(src) => <img src={src()} alt="" />}
          </Show>
        </span>
        <span data-kit="rb-h" data-text="control">
          {props.harness}
        </span>
        <Show when={props.model}>
          {(m) => (
            <span data-kit="rb-m" data-text="control">
              {m()}
            </span>
          )}
        </Show>
        <Icon name="chevron" />
      </button>
      <Show when={props.reset}>
        {(r) => (
          <button type="button" data-kit="au-link" onClick={() => r().onClick()}>
            {r().label}
          </button>
        )}
      </Show>
    </div>
  );
}
