// `shell-task.receipt`, in each turn's trail after its token count (`shell.turn:trail`, the turn's key as its id,
// ADR-0095): " · 90% cache · 3% of weekly", quiet, every window and the whole count in its title (v1 receipt.js; the
// receipt sat on a line of its own under the trail, the phone's foot on three lines: QA r1 #19).
import { Show, createMemo, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { useSignal } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { docOf, receiptOf, type TurnDoc } from './model.ts';
import { trailWords } from './trail.ts';

export default function TrailReceipt(props: ExtensionProps): JSX.Element {
  const shell = useShell();
  const row = useSignal(props.client.entities.row<unknown>('turn', (props.id ?? '') as never));
  const line = createMemo(() => ((d: TurnDoc | undefined) => (d ? receiptOf(d, shell.t) : null))(docOf(row())));
  return (
    <Show when={line()}>
      {(l) => (
        <span data-part="turn.receipt.more" title={l().tip}>
          {trailWords(l().parts)}
        </span>
      )}
    </Show>
  );
}
