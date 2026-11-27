// Meter: how much of something is used (a plan's usage window, a disk), with an optional line
// where something happens (Automations stop at 80%). The value is a share, 0..1.
import { For, Show, type JSX } from 'solid-js';
import type { Tone } from '@wisp/tokens';
import { styleOf, sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/settings.css';

export interface MeterProps extends KitProps {
  /** 0..1; clamped. */
  readonly value: number;
  /** The accessible name: "5-hour window". */
  readonly label: string;
  /** Read out instead of the percent: "42% used, resets 21:00". */
  readonly valueText?: string;
  /** A line across the bar, 0..1: where a limit or a stop line sits. */
  readonly mark?: number;
  readonly markLabel?: string;
  readonly tone?: Tone;
}

const clamp = (v: number): number => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);
const pct = (v: number): string => `${Math.round(clamp(v) * 1000) / 10}%`;

export function Meter(props: MeterProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.meter';
  return (
    <div
      id={props.id}
      data-kit="meter"
      data-part={part()}
      data-tone={props.tone ?? 'accent'}
      role="meter"
      aria-label={props.label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamp(props.value) * 100)}
      aria-valuetext={props.valueText}
      style={styleOf({ '--k-v': pct(props.value) })}
    >
      <span data-kit="meter-fill" data-part={sub(part(), 'fill')} />
      <Show when={props.mark}>{(m) => <span data-kit="meter-mark" data-part={sub(part(), 'mark')} title={props.markLabel} style={styleOf({ '--k-m': pct(m()) })} />}</Show>
    </div>
  );
}

/** One part of a [`StackedMeter`]. */
export interface MeterSegment {
  /** keys the segment, and its legend's line */
  readonly key: string;
  /** "Build output 120 GB": the legend says it, and the bar's segment is named by it */
  readonly label: string;
  /** its share of the whole, 0..1 (the rest of the bar is the track: free space) */
  readonly value: number;
  /** `neutral` reads as "everything else" */
  readonly tone?: Tone | 'faint';
}

export interface StackedMeterProps extends KitProps {
  /** The accessible name: "Disk". */
  readonly label: string;
  /** Read out for the whole bar: "139.9 GB free of 994.7 GB". */
  readonly valueText?: string;
  readonly segments: readonly MeterSegment[];
  /** what the track is ("Free 139.9 GB"); shown last in the legend */
  readonly rest?: string;
}

/** A bar split into parts, with a legend under it (This computer's disk: build output, wisp's folders, everything else,
 * free). Each part's width moves when its share does, so a removal shows as the part shrinking. Additive (2026-11-27). */
export function StackedMeter(props: StackedMeterProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.stackedMeter';
  return (
    <div id={props.id} data-kit="stacked-meter" data-part={part()}>
      <div data-kit="stacked-meter-bar" data-part={sub(part(), 'bar')} role="img" aria-label={props.valueText ? `${props.label}: ${props.valueText}` : props.label}>
        <For each={props.segments}>
          {(s) => <span data-kit="stacked-meter-seg" data-part={sub(part(), 'seg')} data-tone={s.tone ?? 'accent'} title={s.label} style={styleOf({ '--k-v': pct(s.value) })} />}
        </For>
      </div>
      <ul data-kit="stacked-meter-legend" data-layout="wrap" data-part={sub(part(), 'legend')}>
        <For each={props.segments}>
          {(s) => (
            <li data-kit="stacked-meter-key" data-tone={s.tone ?? 'accent'}>
              {s.label}
            </li>
          )}
        </For>
        <Show when={props.rest}>
          {(r) => (
            <li data-kit="stacked-meter-key" data-rest="">
              {r()}
            </li>
          )}
        </Show>
      </ul>
    </div>
  );
}
