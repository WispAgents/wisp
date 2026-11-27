// This computer's Overview (ADR-0144): `LoadRow`, one reading as a name, a bar with a mark where automations start to
// wait, and the numbers on the right (never cut: they're the reading); `DayLines`, the last 24 hours as lines over one
// time axis, with a legend. Additive (2026-11-27).
import { For, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { Skeleton } from '../content/Skeleton.tsx';
import { Meter } from '../settings/Meter.tsx';
import type { SparkPoint } from './Gauge.tsx';
import '../css/machine.css';

export interface LoadRowProps extends KitProps {
  readonly name: string;
  /** used, 0..1; undefined: still reading */
  readonly share: number | undefined;
  /** where automations start to wait, 0..1 */
  readonly mark?: number | null;
  readonly markLabel?: string;
  /** "4.1 of 24 GB" */
  readonly value: string;
  /** "waits past 85% used" */
  readonly sub?: string;
  readonly tone?: 'accent' | 'warn' | 'bad' | 'neutral';
}

export function LoadRow(props: LoadRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.load-row';
  return (
    <div id={props.id} data-kit="load-row" data-part={part()} aria-busy={props.share === undefined ? 'true' : undefined}>
      <span data-kit="load-name" data-text="control">
        {props.name}
      </span>
      <Show when={props.share !== undefined} fallback={<Skeleton width="full" />}>
        <Meter
          value={props.share ?? 0}
          label={props.name}
          valueText={props.sub ? `${props.value}, ${props.sub}` : props.value}
          {...(props.mark !== undefined && props.mark !== null && { mark: props.mark })}
          {...(props.markLabel !== undefined && { markLabel: props.markLabel })}
          tone={props.tone ?? 'accent'}
          part={sub(part(), 'bar')}
        />
      </Show>
      <span data-kit="load-value">
        <b data-text="control">{props.share === undefined ? '' : props.value}</b>
        <Show when={props.sub && props.share !== undefined}>
          <small data-text="control">{props.sub}</small>
        </Show>
      </span>
    </div>
  );
}

export interface DayLine {
  readonly key: string;
  readonly label: string;
  /** x: time across the window, y: the value over its own scale, both 0..1 */
  readonly points: readonly SparkPoint[];
  readonly tone: 'accent' | 'warn' | 'good';
}

export interface DayLinesProps extends KitProps {
  readonly lines: readonly DayLine[];
  /** the chart's accessible name: "The last 24 hours" */
  readonly label: string;
  /** under it, in words: what the lines say (screen readers read this) */
  readonly caption?: string;
  /** the time axis' two ends ("09:12", "now"): a stretch with no line reads as no reading then */
  readonly axis?: readonly [string, string];
}

const W = 600;
const H = 64;
const px = (x: number): string => (Math.min(1, Math.max(0, x)) * W).toFixed(1);
const py = (y: number): string => (H - Math.min(1, Math.max(0, y)) * (H - 4) - 2).toFixed(1);

export function DayLines(props: DayLinesProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.day-lines';
  return (
    <figure id={props.id} data-kit="day-lines" data-part={part()}>
      <ul data-kit="day-lines-legend" data-layout="wrap">
        <For each={props.lines}>
          {(l) => (
            <li data-kit="day-lines-key" data-tone={l.tone}>
              {l.label}
            </li>
          )}
        </For>
      </ul>
      <svg data-kit="day-lines-chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={props.label}>
        <For each={props.lines.filter((l) => l.points.length > 1)}>
          {(l) => <path data-kit="day-lines-line" data-tone={l.tone} d={l.points.map(([x, y], i) => `${i ? 'L' : 'M'}${px(x)} ${py(y)}`).join(' ')} />}
        </For>
      </svg>
      <Show when={props.axis}>
        {(a) => (
          <div data-kit="day-lines-axis" aria-hidden="true">
            <span>{a()[0]}</span>
            <span>{a()[1]}</span>
          </div>
        )}
      </Show>
      <Show when={props.caption}>
        <figcaption data-kit="day-lines-caption">{props.caption}</figcaption>
      </Show>
    </figure>
  );
}
