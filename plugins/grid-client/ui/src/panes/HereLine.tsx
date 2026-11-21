// The line under This computer: its state, then its address on the grid as a link once it has one (TRY-17).
import { Show, type JSX } from 'solid-js';
import { Link } from '@wisp/ui';
import type { Here } from '../here.ts';
import { host } from './state.ts';

export function HereLine(props: { readonly here: Here }): JSX.Element {
  return (
    <>
      {props.here.line}
      <Show when={props.here.url}>
        {(u) => (
          <>
            {' · '}
            {/* one line, cut where it doesn't fit, the whole address on hover (QA round 2 #14: 3 lines on a phone) */}
            <Link href={u()} external truncate title={host(u())} part="grid-client.here.url">
              {host(u())}
            </Link>
          </>
        )}
      </Show>
    </>
  );
}
