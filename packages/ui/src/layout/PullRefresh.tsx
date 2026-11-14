// PullRefresh: the look of pulling a list down to reconnect (design/mobile.md §A.4: every computer is
// reconnected now, skipping the backoff; clients never poll). Android draws the M3 indicator (a surface
// disc that follows the pull, its arc growing to a full turn at the line, spinning while refreshing); iOS
// draws the refresh control (the activity indicator's ticks appearing one by one with the pull, then
// spinning). The browser has no pull to refresh: it draws nothing. The gesture that feeds `progress`
// lands with the gesture registry (ADR-0065 build step 7); this is only how it looks.
import { For, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';

export interface PullRefreshProps extends KitProps {
  /** How far the pull has come: 0 at rest, 1 on the line where letting go refreshes (it may pass 1). */
  readonly progress: number;
  /** Letting go ran it: spin until the computers answer. */
  readonly refreshing: boolean;
  /** What refreshing does, for screen readers (e.g. *Reconnecting*). */
  readonly label: string;
}

/** the iOS activity indicator's ticks (UIActivityIndicatorView draws 8) */
export const PULL_TICKS = 8;

/** The pull as the CSS reads it: clamped to [0, 1.5], three decimals (a refresh shows full). */
export function pullAmount(progress: number, refreshing: boolean): string {
  const p = refreshing ? 1 : Math.min(Math.max(progress, 0), 1.5);
  return String(Math.round(p * 1000) / 1000);
}

export function PullRefresh(props: PullRefreshProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.pull-refresh';
  const look = usePlatform();
  return (
    <Show when={look() !== 'web' && (props.refreshing || props.progress > 0)}>
      <div
        id={props.id}
        data-kit="pull-refresh"
        data-part={part()}
        role={props.refreshing ? 'status' : undefined}
        aria-label={props.refreshing ? props.label : undefined}
        aria-hidden={props.refreshing ? undefined : 'true'}
        data-refreshing={props.refreshing ? '' : undefined}
        data-armed={props.refreshing || props.progress >= 1 ? '' : undefined}
        style={{ '--k-pull': pullAmount(props.progress, props.refreshing) }}
      >
        <Show
          when={look() === 'ios'}
          fallback={
            <span data-kit="pull-disc" data-part={sub(part(), 'disc')}>
              <svg data-kit="pull-arc" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="12" cy="12" r="9" pathLength="100" />
              </svg>
            </span>
          }
        >
          <span data-kit="pull-spinner" data-part={sub(part(), 'spinner')}>
            <For each={Array.from({ length: PULL_TICKS }, (_, i) => i)}>{(i) => <span data-kit="pull-tick" style={{ '--k-i': String(i) }} />}</For>
          </span>
        </Show>
      </div>
    </Show>
  );
}
