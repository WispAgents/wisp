// A control's panel (Act.panel): the kit's bare popover around what the caller draws, on the caller's own button.
// Loaded apart from the first page (lazy-part.ts): act.tsx draws the button at once and binds this once it's here.
// On the phone looks it's a sheet of its own (QA r6 phone #7): a popover anchored to a phone's control covers the
// screen it's on and clips at the edge; iOS and M3 open a panel as a sheet.
import { Show, untrack, type JSX } from 'solid-js';
import { Popover } from '../controls/Popover.tsx';
import { Sheet } from '../controls/Dialog.tsx';
import type { Placement } from '../controls/Menu.tsx';
import type { PartName } from '../core/props.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';

export interface ActPanelProps {
  readonly label: string;
  readonly part: PartName;
  readonly cardPart?: PartName | undefined;
  readonly placement: Placement;
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly content: () => JSX.Element;
  readonly render: (trigger: () => Record<string, unknown>) => JSX.Element;
}

export function ActPanel(props: ActPanelProps): JSX.Element {
  const look = usePlatform();
  return (
    <Show when={look() === 'web'} fallback={<PanelSheet {...props} />}>
      <Popover
        part={props.part}
        {...(props.cardPart && { cardPart: props.cardPart })}
        bare
        inWindow
        placement={props.placement}
        trigger={{ icon: 'agent', 'aria-label': props.label }}
        open={props.open}
        onOpenChange={props.onOpenChange}
        render={props.render}
      >
        {/* drawn once per open, not again when the caller's act is rebuilt (a toolbar recomputed on every change of
            what's in the panel would redraw it, and the field you type in would lose its caret): what it shows follows
            its own signals */}
        {untrack(() => props.content)()}
      </Popover>
    </Show>
  );
}

/** the phone's panel: the caller's button opens a sheet titled with the panel's name, holding the same content */
export function PanelSheet(props: Omit<ActPanelProps, 'placement'>): JSX.Element {
  const trigger = (): Record<string, unknown> => ({
    'aria-haspopup': 'dialog',
    'aria-expanded': props.open ? 'true' : 'false',
    onClick: (e: MouseEvent) => {
      e.stopPropagation();
      props.onOpenChange(true);
    },
  });
  return (
    <>
      {props.render(trigger)}
      <Sheet title={props.label} open={props.open} onOpenChange={props.onOpenChange} part={props.part} detents={['large']}>
        <Show when={props.open}>{untrack(() => props.content)()}</Show>
      </Sheet>
    </>
  );
}
