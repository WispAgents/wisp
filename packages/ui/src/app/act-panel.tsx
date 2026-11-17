// A control's panel (Act.panel): the kit's bare popover around what the caller draws, on the caller's own button.
// Loaded apart from the first page (lazy-part.ts): act.tsx draws the button at once and binds this once it's here.
import { untrack, type JSX } from 'solid-js';
import { Popover } from '../controls/Popover.tsx';
import type { Placement } from '../controls/Menu.tsx';
import type { PartName } from '../core/props.ts';

export function ActPanel(props: {
  readonly label: string;
  readonly part: PartName;
  readonly cardPart?: PartName | undefined;
  readonly placement: Placement;
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly content: () => JSX.Element;
  readonly render: (trigger: () => Record<string, unknown>) => JSX.Element;
}): JSX.Element {
  return (
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
  );
}
