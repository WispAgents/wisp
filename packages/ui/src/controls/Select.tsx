// Select on Zag: a button the same box as an input, its options in a menu-like list (v1's
// .kit-sel). Typeahead, arrow keys, Home/End; a hidden native select carries it in forms.
// The button draws at once; the machine (./select-machine.tsx, with floating-ui) loads apart from the first page
// (./lazy-part.ts) and binds to it. A press before it's there shows the button pressed and opens the list as it lands.
// The phone apps (ADR-0065 step 6, design/mobile.md §A.7): a short list is the same machine in the look's shape (M3
// exposed dropdown menu; the iOS pull-down menu), from the look's stylesheet. A long one (more than SHEET_AFTER
// choices) opens as a sheet instead: Android's bottom sheet of radio rows, iOS's checkmark list (./phone.tsx, loaded
// apart with the overlays' phone halves: the browser never fetches it). A choice ticks the picker haptic (§A.5).
import { createEffect, createSignal, createUniqueId, For, mergeProps, onCleanup, Show, untrack, type Accessor, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type Labelled, type PartName } from '../core/props.ts';
import { lazyPart, warm, type Replay } from './lazy-part.ts';
import type { SelectApi } from './select-machine.tsx';
import { Icon } from '../content/Icon.tsx';
import { useField, useFieldText } from './Field.tsx';
import type { FieldSize } from './Input.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useGestureHost } from '../gestures/host.tsx';
import { phoneOverlays } from './phone-part.ts';

export interface SelectOption {
  readonly value: string;
  readonly label: string;
  readonly disabled?: boolean;
  /** A heading drawn over the first option of each run with the same group (the "When" picker's sources by plugin). */
  readonly group?: string;
}

export interface SelectProps extends KitProps, Labelled {
  readonly items: readonly SelectOption[];
  readonly value: string | undefined;
  readonly onChange: (value: string) => void;
  /** Shown while nothing is picked. */
  readonly placeholder?: string;
  readonly size?: FieldSize;
  readonly disabled?: boolean;
  readonly invalid?: boolean;
  readonly name?: string;
  /** Code-ish values. */
  readonly mono?: boolean;
}

const machine = lazyPart(() => import('./select-machine.tsx'));

/** In the phone apps a list longer than this opens as a sheet (M3: a menu holds a short set; HIG: a long set is a
 * pushed checkmark list), not a menu at the button. */
export const SHEET_AFTER = 6;

/** Whether `look` draws a list of `count` choices as a sheet. */
export const selectSheet = (look: 'android' | 'ios' | 'web', count: number): boolean => look !== 'web' && count > SHEET_AFTER;

type Bind = Record<string, unknown>;

export function Select(props: SelectProps): JSX.Element {
  const field = useField();
  useFieldText()();
  const look = usePlatform();
  const host = useGestureHost();
  const id = createUniqueId();
  const sheet = (): boolean => selectSheet(look(), props.items.length);
  // the phone sheet: open at once on the press (the trigger shows it), its rows as the chunk lands
  const [sheetOpen, setSheetOpen] = createSignal(false);
  let trigger: HTMLButtonElement | undefined;
  const choose = (value: string): void => {
    if (look() !== 'web') host.haptic('select');
    props.onChange(value);
  };
  const withChoose = mergeProps(props, { onChange: choose });
  // a list that shrinks back to a menu while its sheet is open: nothing is open any more
  createEffect(() => {
    if (!sheet()) setSheetOpen(false);
  });
  const [api, setApi] = createSignal<Accessor<SelectApi>>();
  const [pending, setPending] = createSignal<Replay>();
  // a list that grows into a sheet before its early press found the machine: that press is gone with the menu it was
  // for (the machine never mounts to replay it), so the button doesn't stay open and a later shrink replays nothing
  createEffect(() => {
    if (sheet()) setPending(undefined);
  });
  const bound = (): SelectApi | undefined => api()?.();
  const part = (): PartName => props.part ?? 'ui.select';
  const invalid = (): boolean => Boolean(props.invalid ?? field()?.invalid);
  const triggerId = (): string | undefined => field()?.id ?? props.id;
  const chosen = (): SelectOption | undefined => props.items.find((i) => i.value === props.value);
  const press = (r: Replay): void => {
    if (sheet()) {
      setSheetOpen(true);
      phoneOverlays.load().catch(() => setSheetOpen(false));
      return;
    }
    setPending(r);
    machine.load().catch(() => setPending(undefined));
  };
  const open = (): boolean => pending() !== undefined || sheetOpen();
  const plainTrigger = (): Bind => ({
    type: 'button',
    id: triggerId() ?? `select:${id}:trigger`,
    role: 'combobox',
    'aria-haspopup': 'listbox',
    'aria-expanded': open(),
    'data-state': open() ? 'open' : 'closed',
    disabled: props.disabled === true,
    'data-invalid': flag(invalid()),
    onPointerEnter: warm(sheet() ? phoneOverlays : machine),
    onFocus: warm(sheet() ? phoneOverlays : machine),
    onPointerDown: warm(sheet() ? phoneOverlays : machine),
    onClick: (e: MouseEvent) => press({ el: e.currentTarget as HTMLElement, type: 'click' }),
    onKeyDown: (e: KeyboardEvent) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      e.preventDefault();
      press({ el: e.currentTarget as HTMLElement, type: 'keydown', key: e.key });
    },
  });
  const has = (): boolean => {
    const a = bound();
    return a ? a.hasSelectedItems : chosen() !== undefined;
  };
  const text = (): string => {
    const a = bound();
    return a ? a.valueAsString : (chosen()?.label ?? '');
  };
  return (
    <div {...(bound()?.getRootProps() ?? { 'data-state': open() ? 'open' : 'closed' })} data-kit="select" data-part={part()}>
      <button
        ref={(el) => {
          trigger = el;
        }}
        {...(bound()?.getTriggerProps() ?? plainTrigger())}
        aria-label={props['aria-label']}
        aria-labelledby={props['aria-labelledby']}
        aria-describedby={props['aria-describedby'] ?? field()?.describedBy}
        data-kit="select-trigger"
        // v1's name for a list's button in dialogs and Settings (ui/select.js .kit-sel), beside the kit's own
        data-part={`${sub(part(), 'trigger')} form.select`}
        data-size={props.size ?? 'md'}
        data-mono={flag(props.mono)}
        data-placeholder={flag(!has())}
      >
        <span data-kit="select-value" data-text="control">{has() ? text() : (props.placeholder ?? '')}</span>
        <Icon name="chevron" size="sm" />
      </button>
      <select {...(bound()?.getHiddenSelectProps() ?? { name: props.name, value: props.value ?? '', disabled: props.disabled === true, 'aria-hidden': true, tabIndex: -1, hidden: true })}>
        <For each={props.items}>{(item) => <option value={item.value}>{item.label}</option>}</For>
      </select>
      <Show when={!sheet() && machine.ready()}>
        {(m) => {
          const M = m().SelectMachine;
          // the early press, taken now (the machine replays it on mount); once bound it's the machine's, so it no longer
          // holds the plain button open: a list that grows into a sheet unmounts the machine and the button goes back to
          // its plain props, closed
          const early = untrack(pending);
          onCleanup(() => setApi(undefined));
          const bind = (a: Accessor<SelectApi>): void => {
            setApi(() => a);
            setPending(undefined);
          };
          return <M base={withChoose} id={id} invalid={invalid()} triggerId={triggerId()} part={part()} bind={bind} pending={early} />;
        }}
      </Show>
      <Show when={sheet() && sheetOpen() && phoneOverlays.ready()}>
        {(m) => {
          const S = m().SelectSheet;
          return (
            <S
              look={look() === 'ios' ? 'ios' : 'android'}
              items={props.items}
              value={props.value}
              title={props['aria-label'] ?? trigger?.labels?.[0]?.firstChild?.textContent ?? props.placeholder}
              part={part()}
              onChoose={choose}
              onClose={() => {
                setSheetOpen(false);
                trigger?.focus({ preventScroll: true });
              }}
            />
          );
        }}
      </Show>
    </div>
  );
}
