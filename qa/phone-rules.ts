// What a phone view is, for the three scripts that check the phone apps (ADR-0065 §5): qa/platform.mjs (the
// metric table), qa/layout.mjs --phone (nothing wraps, bars collapse by priority) and qa/feedback.mjs --phone
// (every press shows a change within 100 ms). All three read the same env, walk the same sizes × themes × text
// scales × views, and skip a view with no platform look yet the same way (a skip with its reason, never a pass).
// Pure: no browser, no clock (tests: tools/test/qa-phone.test.ts). The browser half is qa/phone.mjs.

import { type Finding, type Look, pickSizes, type Size, type Status } from './platform-rules.ts';

export interface PhoneEnv {
  readonly looks: readonly Look[];
  readonly sizes: readonly Size[];
  readonly themes: readonly string[];
  /** text scale in percent: 100 and 200 by default (Android font scale / iOS Dynamic Type) */
  readonly scales: readonly number[];
  /** VIEWS: the names to keep; undefined means every phone view */
  readonly views?: readonly string[];
  /** PLATFORM_FORCE=1: check a view with no platform look yet too (what build steps 3–7 still have to fix) */
  readonly force: boolean;
}

const list = (v: string | undefined, d: string): string[] =>
  (v || d)
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);

/** PLATFORMS, SIZES, THEMES, SCALES, VIEWS, PLATFORM_FORCE, with ADR-0065 §5's defaults. Throws on a value that
 * picks nothing (an unknown look, a size no look has, a scale that isn't a positive number), so a typo is never a
 * silent empty run that "passes". */
export function phoneEnv(env: Readonly<Record<string, string | undefined>>, defaults: { readonly themes?: string } = {}): PhoneEnv {
  const looks = list(env.PLATFORMS, 'android,ios');
  const bad = looks.filter((l) => l !== 'android' && l !== 'ios');
  if (bad.length) throw new Error(`PLATFORMS: ${bad.join(', ')} is not a phone look (android, ios)`);
  const only = env.SIZES ? list(env.SIZES, '') : undefined;
  const sizes = pickSizes(looks, only);
  if (!sizes.length) throw new Error(`no sizes for PLATFORMS=${looks.join(',')} SIZES=${env.SIZES ?? ''}`);
  const scales = list(env.SCALES, '100,200').map(Number);
  if (scales.some((s) => !Number.isFinite(s) || s <= 0)) throw new Error(`SCALES=${env.SCALES}: text scales are positive percentages (100,200)`);
  const views = env.VIEWS ? list(env.VIEWS, '') : undefined;
  return { looks: looks as Look[], sizes, themes: list(env.THEMES, defaults.themes ?? 'light,dark'), scales, ...(views ? { views } : {}), force: env.PLATFORM_FORCE === '1' };
}

/** The phone views (qa/layout-areas.mjs `shell`) a run keeps; a VIEWS name that isn't one throws. */
export function phoneViews<V extends { readonly name: string }>(all: readonly V[], only?: readonly string[]): V[] {
  if (!only) return [...all];
  const unknown = only.filter((n) => !all.some((v) => v.name === n));
  if (unknown.length) throw new Error(`VIEWS: no phone view ${unknown.join(', ')} (the views are ${all.map((v) => v.name).join(', ')})`);
  return all.filter((v) => only.includes(v.name));
}

export interface PhoneRun<V> {
  readonly size: Size;
  readonly theme: string;
  readonly scale: number;
  readonly view: V;
}

/** Every run, sizes outermost then themes, scales and views (qa/platform.mjs's order). */
export function phoneRuns<V>(sizes: readonly Size[], themes: readonly string[], scales: readonly number[], views: readonly V[]): PhoneRun<V>[] {
  const out: PhoneRun<V>[] = [];
  for (const size of sizes) for (const theme of themes) for (const scale of scales) for (const view of views) out.push({ size, theme, scale, view });
  return out;
}

/** One run's name, the same in every script's output: `android 360x800 dark 200% home`. */
export function runLabel(look: string, size: { readonly width: number; readonly height: number }, theme: string | undefined, scale: number, view: string): string {
  return [look, `${size.width}x${size.height}`, theme, `${scale}%`, view].filter(Boolean).join(' ');
}

export type Gate = { readonly run: true; readonly forced: boolean } | { readonly run: false; readonly skip: string } | { readonly run: false; readonly fail: Finding };

/** Whether a measured view is checked: lookStatus's verdict, with PLATFORM_FORCE turning a "no platform look yet"
 * skip into a (forced) check. "data-platform not set" is never forced: the page isn't the phone app at all. */
export function gate(status: Status, force: boolean): Gate {
  if (status.check) return { run: true, forced: false };
  if ('fail' in status) return { run: false, fail: status.fail };
  if (force && status.skip.startsWith('no platform look yet')) return { run: true, forced: true };
  return { run: false, skip: status.skip };
}

export interface Tally {
  readonly pass: number;
  readonly fail: number;
  readonly skip: number;
  /** each distinct skip reason once, in first-seen order */
  readonly reasons: readonly string[];
}

export function tally(rows: readonly { readonly status: 'pass' | 'fail' | 'skip'; readonly skip?: string }[]): Tally {
  const n = (s: string) => rows.filter((r) => r.status === s).length;
  return { pass: n('pass'), fail: n('fail'), skip: n('skip'), reasons: [...new Set(rows.flatMap((r) => (r.status === 'skip' && r.skip ? [r.skip] : [])))] };
}
