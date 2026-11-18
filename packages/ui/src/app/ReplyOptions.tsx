// The reply box's options on a phone (docs/ux/2026-11-18-phone.md #1): the bar keeps attach, one chip that sums up how
// the reply runs ("✦ Full auto · Codex") and Send; the chip opens one sheet (an iOS sheet with detents, an M3 modal
// bottom sheet) holding what the bar held: the permissions as a choice list, the agent, Delegate, the goal, Send later
// and the queue switch. Every act is the bar's own (the same menus, panel and presses), so nothing a phone could do is
// lost, only one tap deeper. Only the phone looks draw it (ReplyBox reads the look); the browser's bar is unchanged.
import { For, Show, createSignal, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Sheet } from '../controls/Dialog.tsx';
import type { MenuEntry, MenuItem } from '../controls/menu-entries.ts';
import { WithAct, type Act } from './act.tsx';

/** the phone's words for the sheet and its rows (the shell's strings) */
export interface ReplyOptionLabels {
  /** the chip's accessible name and the sheet's title ("Reply options") */
  readonly options: string;
  /** the permissions' heading ("Permissions") */
  readonly mode: string;
  /** the harness row ("Agent") */
  readonly agent: string;
}

export interface ReplyOptionsProps {
  readonly labels: ReplyOptionLabels;
  readonly mode: {
    readonly label: string;
    readonly icon?: IconName | undefined;
    readonly act?: Act | undefined;
  };
  readonly runner: {
    readonly title: string;
    readonly logo?: string | undefined;
    readonly model?: string | undefined;
    readonly act?: Act | undefined;
  };
  /** a row per remaining tool, in the bar's order; `on` lights it (the goal while it's going, the queue while it holds) */
  readonly rows: readonly {
    readonly id: string;
    readonly part: string;
    readonly icon: IconName;
    readonly label: string;
    readonly on?: boolean | undefined;
    readonly act?: Act | undefined;
  }[];
}

const items = (es: readonly MenuEntry[]): MenuItem[] =>
  es.flatMap((e): MenuItem[] => (e.kind === 'separator' || e.kind === 'submenu' ? [] : e.kind === 'group' ? [...e.items] : [e]));

/** the harness's name: the runner title's first line ("Codex · gpt-6-sol"), its model beside it when given apart */
const runnerName = (r: ReplyOptionsProps['runner']): string => [r.title.split('\n')[0] ?? '', r.model].filter(Boolean).join(' · ');

export function ReplyOptions(props: ReplyOptionsProps): JSX.Element {
  const [open, setOpen] = createSignal(false);
  /** a press that leaves the sheet for what it opens (the harness picker over the box, the goal line in it) */
  const after = (act: Act | undefined): void => {
    setOpen(false);
    queueMicrotask(() => act?.onClick?.());
  };
  return (
    <>
      {/* the harness picker opens from here (its anchor), as it opened from the harness button */}
      <button
        type="button"
        data-kit="c-opts"
        data-part="composer.harness"
        aria-haspopup="dialog"
        aria-expanded={open() ? 'true' : 'false'}
        aria-label={`${props.labels.options}: ${props.mode.label} · ${runnerName(props.runner)}`}
        title={props.labels.options}
        onClick={() => setOpen(true)}
      >
        <Icon name={props.mode.icon ?? 'spark'} />
        <span data-kit="c-opts-mode" data-text="control">
          {props.mode.label}
        </span>
        <span data-kit="rb-logo" data-bare="">
          <Show when={props.runner.logo}>{(src) => <img src={src()} alt="" />}</Show>
        </span>
        <Icon name="chevron" size="xs" />
      </button>
      <Sheet title={props.labels.options} open={open()} onOpenChange={setOpen} part="composer.options" detents={['medium', 'large']}>
        <div data-kit="opts-list">
          <ModeRows mode={props.mode} label={props.labels.mode} />
          <ToolRows {...props} after={after} />
        </div>
      </Sheet>
    </>
  );
}

/** the permissions as a choice list: the mode menu's own items, its check on the one in use */
function ModeRows(props: { readonly mode: ReplyOptionsProps['mode']; readonly label: string }): JSX.Element {
  return (
    <Show when={props.mode.act?.menu}>
      {(m) => (
        <div data-kit="opts-group" role="radiogroup" aria-label={props.label}>
          <span data-kit="opts-head" data-text="control">
            {props.label}
          </span>
          <For each={items(m().items)}>
            {(it) => (
              <button
                type="button"
                role="radio"
                data-kit="opts-row"
                data-part="composer.options.mode"
                aria-checked={it.checked === true}
                onClick={() => m().onSelect(it.value)}
              >
                <Show when={it.icon}>{(i) => <Icon name={i()} />}</Show>
                <span data-kit="opts-text">
                  <span data-text="control">{it.label}</span>
                  <Show when={it.hint}>
                    <small>{it.hint}</small>
                  </Show>
                </span>
                <Show when={it.checked}>
                  <Icon name="check" />
                </Show>
              </button>
            )}
          </For>
        </div>
      )}
    </Show>
  );
}

/** the agent row (it leaves for the harness picker) and the bar's other tools, each its own act */
function ToolRows(props: ReplyOptionsProps & { readonly after: (act: Act | undefined) => void }): JSX.Element {
  const after = props.after;
  return (
    <div data-kit="opts-group">
      <button type="button" data-kit="opts-row" data-part="composer.options.agent" onClick={() => after(props.runner.act)}>
        <span data-kit="rb-logo" data-bare="">
          <Show when={props.runner.logo}>{(src) => <img src={src()} alt="" />}</Show>
        </span>
        <span data-kit="opts-text">
          <span data-text="control">{props.labels.agent}</span>
          <small>{runnerName(props.runner)}</small>
        </span>
        <Icon name="chevron" />
      </button>
      <For each={props.rows}>
        {(r) => (
          <WithAct
            act={r.act?.onClick && !r.act.menu && !r.act.panel ? { ...r.act, onClick: () => after(r.act) } : r.act}
            render={(trigger) => (
              <button
                type="button"
                {...trigger()}
                data-kit="opts-row"
                data-part={r.part}
                data-on={flag(r.on)}
                aria-pressed={r.on === undefined ? undefined : r.on}
              >
                <Icon name={r.icon} />
                <span data-kit="opts-text">
                  <span data-text="control">{r.label}</span>
                </span>
                <Show when={r.on}>
                  <Icon name="check" />
                </Show>
              </button>
            )}
          />
        )}
      </For>
    </div>
  );
}
