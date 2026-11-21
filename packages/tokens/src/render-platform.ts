// The phone apps' token sets (ADR-0065 §5, design/mobile.md §A.8): tokens/platform/<look>.json → one
// stylesheet per look, every value under :root[data-platform='<look>'] in @layer tokens. They are
// separate files on purpose: only the phone app loads one (packages/ui/src/platform/styles.ts), so the
// browser's initial CSS carries none of it (ADR-0011 budgets). Pure: no file access.

export const LOOKS = ['android', 'ios'] as const;
export type Look = (typeof LOOKS)[number];

const NAME = /^[a-z0-9][a-z0-9-]*$/;

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function fail(where: string, why: string): never {
  throw new Error(`tokens: ${where}: ${why}`);
}

/** The group's entries, `$doc` skipped, each name checked and each one citing its spec. */
function group(v: unknown, where: string): Array<[string, Record<string, unknown>]> {
  if (v === undefined) return [];
  if (!isRecord(v)) fail(where, 'expected an object');
  const out: Array<[string, Record<string, unknown>]> = [];
  for (const [k, val] of Object.entries(v)) {
    if (k.startsWith('$')) continue;
    if (!NAME.test(k)) fail(where, `bad token name "${k}" (lowercase, digits and dashes)`);
    if (!isRecord(val)) fail(`${where}.${k}`, 'expected an object');
    if (typeof val['spec'] !== 'string' || !val['spec'].trim()) fail(`${where}.${k}`, 'cite its spec (design/mobile.md §A.8: numbers are the platform spec’s)');
    out.push([k, val]);
  }
  return out;
}

function str(v: Record<string, unknown>, key: string, where: string): string {
  const s = v[key];
  if (typeof s !== 'string' || s === '') fail(where, `expected a non-empty "${key}"`);
  if (/[;{}]/.test(s)) fail(where, `"${key}" can't hold ; { or }`);
  return s;
}

const round = (n: number): number => Math.round(n * 1000) / 1000;

/** A spring's position at t seconds, SwiftUI's way: `duration` is the perceptual duration (ω0 = 2π / duration)
 * and `bounce` sets the damping (ζ = 1 − bounce). */
function springAt(duration: number, bounce: number): (t: number) => number {
  const w0 = (2 * Math.PI) / duration;
  const zeta = 1 - bounce;
  if (zeta >= 1) return (t) => 1 - (1 + w0 * t) * Math.exp(-w0 * t);
  const wd = w0 * Math.sqrt(1 - zeta * zeta);
  return (t) => 1 - Math.exp(-zeta * w0 * t) * (Math.cos(wd * t) + ((zeta * w0) / wd) * Math.sin(wd * t));
}

/** When the spring has settled (within 0.1 % of its end for good), in ms rounded to 10: the CSS duration. */
export function springSettleMs(duration: number, bounce: number): number {
  const at = springAt(duration, bounce);
  let last = 0;
  for (let i = 0; i <= 3000; i++) {
    const t = (duration * 3 * i) / 3000;
    if (Math.abs(1 - at(t)) >= 0.001) last = t;
  }
  return Math.ceil((last * 1000) / 10) * 10;
}

/**
 * A spring as a CSS `linear()` easing, sampled from 0 to its settle time (`springSettleMs`, which the
 * token's `-dur` twin carries); the last point is exactly 1, so the animation lands where it's going.
 */
export function springLinear(duration: number, bounce: number, points = 32): string {
  if (!(duration > 0) || !(bounce >= 0 && bounce < 1)) throw new Error(`tokens: spring(${duration}, ${bounce}): duration > 0 and 0 ≤ bounce < 1`);
  const at = springAt(duration, bounce);
  const settle = springSettleMs(duration, bounce) / 1000;
  const ys: number[] = [];
  for (let i = 0; i < points; i++) ys.push(round(at((settle * i) / (points - 1))));
  ys[ys.length - 1] = 1;
  return `linear(${ys.join(', ')})`;
}

const line = (name: string, value: string, spec: string): string => `--${name}: ${value}; /* ${spec.replace(/\*\//g, '')} */`;

function block(selector: string, lines: readonly string[], indent: string): string {
  return `${indent}${selector} {\n${lines.map((l) => `${indent}  ${l}`).join('\n')}\n${indent}}`;
}

/** One look's stylesheet. `json` is tokens/platform/<look>.json. */
export function renderPlatform(look: Look, json: unknown): string {
  const where = `platform/${look}`;
  if (!isRecord(json)) fail(where, 'expected an object');
  const names = new Set<string>();
  const once = (k: string): string => {
    if (names.has(k)) fail(where, `"${k}" is defined twice`);
    names.add(k);
    return k;
  };
  const fixed: string[] = [];
  for (const [k, v] of group(json['static'], `${where}.static`)) fixed.push(line(once(k), str(v, 'value', `${where}.static.${k}`), String(v['spec'])));
  for (const [k, v] of group(json['springs'], `${where}.springs`)) {
    const d = v['duration'];
    const b = v['bounce'];
    if (typeof d !== 'number' || typeof b !== 'number') fail(`${where}.springs.${k}`, 'expected numeric duration (s) and bounce');
    fixed.push(line(once(k), springLinear(d, b), String(v['spec'])));
    fixed.push(line(once(`${k}-dur`), `${springSettleMs(d, b)}ms`, `${k} settles (within 0.1 %) after this: its animation's duration`));
  }
  for (const [k, v] of group(json['aliases'], `${where}.aliases`)) fixed.push(line(once(k), str(v, 'value', `${where}.aliases.${k}`), String(v['spec'])));
  const light: string[] = [];
  const dark: string[] = [];
  for (const [k, v] of group(json['themed'], `${where}.themed`)) {
    const spec = String(v['spec']);
    light.push(line(once(k), str(v, 'light', `${where}.themed.${k}`), spec));
    dark.push(line(k, str(v, 'dark', `${where}.themed.${k}`), spec));
  }
  const root = `:root[data-platform='${look}']`;
  const parts = [
    `/* Generated by packages/tokens/src/build.ts from tokens/platform/${look}.json. Edit the JSON, then \`npm run build\`. */`,
    `/* The ${look === 'android' ? 'Android' : 'iPhone'} app's tokens (ADR-0065 §5): loaded by the phone app only, never by the browser. */`,
    // the kit's layer order, stated again: if this sheet reached the page before the kit's, it would otherwise set it
    '@layer reset, tokens, kit, plugins;',
    '@layer tokens {',
    block(root, [...fixed, ...light], '  '),
  ];
  if (dark.length) {
    // the same rules as tokens.css: the OS's scheme unless [data-theme] says otherwise, on the root or any element
    parts.push('  @media (prefers-color-scheme: dark) {', block(`${root}:not([data-theme='light'])`, dark, '    '), '  }');
    parts.push(block(`${root}[data-theme='dark'], ${root} [data-theme='dark']`, dark, '  '));
    parts.push(block(`${root}[data-theme='light'], ${root} [data-theme='light']`, light, '  '));
  }
  parts.push('}', '');
  return parts.join('\n');
}
