// SelectionBar: a list's bar while it picks rows (ADR-0065 step 13, design/mobile.md §A.4 Selection mode), in place of
// the screen's TopAppBar. The kit draws the platform's own, in the platform's own words (kit strings `select.*`):
//   Android  the M3 contextual top app bar: × (leave) at the start, "n selected" as its title, Select all and the bulk
//            actions as icons at the end (a Toolbar: what doesn't fit goes into ⋮, Select all first, as Gmail's).
//            Unpicking the last row leaves it.
//   iOS      edit mode: Select All / Deselect All leading, "n Selected" (Select Items with none) centred, Done trailing
//            in bold; the bulk actions in a bottom toolbar over the tab bar (HIG: Mail, Files). It stays until Done.
// The actions' words come from props; nothing here names a feature. In the browser it is Android's shape on the web
// kit, which no browser screen uses today.
import { createEffect, on, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { useT, type Translate } from '../core/i18n.tsx';
import { Button, IconButton } from '../controls/Button.tsx';
import { Toolbar, type ToolbarItem } from '../composites/Toolbar.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';

/** A bulk action: what it does to every picked row. */
export interface SelectionAction {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
  readonly onSelect: () => void;
  /** none of the picked rows can take it (or none is picked) */
  readonly disabled?: boolean;
  readonly danger?: boolean;
}

export interface SelectionBarProps extends KitProps {
  /** how many rows are picked */
  readonly count: number;
  /** every row the list shows is picked: Select all turns into Deselect all */
  readonly allPicked: boolean;
  readonly onAll: () => void;
  /** leaving selection (×, Done, Android's last row unpicked) */
  readonly onExit: () => void;
  readonly actions: readonly SelectionAction[];
  /** the actions' name for a screen reader ("Selected tasks") */
  readonly actionsLabel: string;
  readonly t?: Translate;
}

const items = (actions: readonly SelectionAction[]): ToolbarItem[] =>
  actions.map((a, i) => ({
    id: a.id,
    label: a.label,
    icon: a.icon,
    onSelect: a.onSelect,
    ...(a.disabled === undefined ? {} : { disabled: a.disabled }),
    ...(a.danger ? { tone: 'bad' as const } : {}),
    // in their order: the last ones fold into More first
    priority: actions.length - i,
  }));

export function SelectionBar(props: SelectionBarProps): JSX.Element {
  const look = usePlatform();
  const t = useT(() => props.t);
  const part = (): PartName => props.part ?? 'ui.selection-bar';
  const ios = (): boolean => look() === 'ios';
  // M3: the contextual bar goes once nothing is picked; iOS's edit mode waits for Done
  createEffect(
    on(
      () => props.count,
      (n, was) => {
        if (!ios() && n === 0 && (was ?? 0) > 0) props.onExit();
      },
    ),
  );
  const title = (): string => (ios() ? (props.count ? t('select.count-ios', { count: props.count }) : t('select.none-ios')) : t('select.count', { count: props.count }));
  const allLabel = (): string => t(props.allPicked ? (ios() ? 'select.deselect-ios' : 'select.deselect') : ios() ? 'select.all-ios' : 'select.all');
  return (
    <>
      <header id={props.id} data-kit="app-bar" data-kind="select" data-part={part()}>
        <div data-kit="app-bar-row">
          <Show when={ios()} fallback={<IconButton icon="x" label={t('select.exit')} onClick={() => props.onExit()} part={sub(part(), 'exit')} />}>
            <Button variant="ghost" onClick={() => props.onAll()} part={sub(part(), 'all')}>
              {allLabel()}
            </Button>
          </Show>
          <h1 data-kit="app-bar-title" data-text="control" data-part={sub(part(), 'title')} aria-live="polite">
            {title()}
          </h1>
          <div data-kit="app-bar-actions">
            <Show
              when={ios()}
              fallback={
                <Toolbar
                  aria-label={props.actionsLabel}
                  items={[...items(props.actions), { id: 'all', label: allLabel(), icon: 'check', priority: 0, onSelect: () => props.onAll() }]}
                  justify="end"
                  part={sub(part(), 'actions')}
                  t={t}
                />
              }
            >
              <span data-kit="app-bar-confirm">
                <Button variant="ghost" onClick={() => props.onExit()} part={sub(part(), 'exit')}>
                  {t('done')}
                </Button>
              </span>
            </Show>
          </div>
        </div>
      </header>
      {/* iOS: the bulk actions in the toolbar over the tab bar */}
      <Show when={ios()}>
        <div data-kit="select-toolbar" data-part={sub(part(), 'toolbar')}>
          <Toolbar aria-label={props.actionsLabel} items={items(props.actions)} part={sub(part(), 'tools')} t={t} />
        </div>
      </Show>
    </>
  );
}
