// The px values the design tokens allow (packages/tokens/tokens/*.json). The kit's CSS may use a px
// value only if it is on this scale (a query: only a breakpoint) (plan §5: "a stylelint rule fails CI on any px value outside the
// token scale, even inside the kit").

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { isRecord } from './workspace.ts';

export const TOKENS_DIR = 'packages/tokens/tokens';
/** Files whose bare numbers are lengths in px (space.json: { "2": 8 }). */
const LENGTH_FILE = /space|size|radius|border|width|breakpoint|gap|inset|shadow/i;
const PX = /^(-?\d*\.?\d+)px$/;

function collect(v: unknown, numbersArePx: boolean, out: Set<number>): void {
  if (typeof v === 'string') {
    for (const part of v.split(/\s+/)) {
      const m = PX.exec(part);
      if (m?.[1]) out.add(Math.abs(Number(m[1])));
    }
  } else if (typeof v === 'number') {
    if (numbersArePx) out.add(Math.abs(v));
  } else if (Array.isArray(v)) {
    for (const x of v) collect(x, numbersArePx, out);
  } else if (isRecord(v)) {
    // DTCG dimension: { "$value": { "value": 4, "unit": "px" } }
    if (typeof v['value'] === 'number' && v['unit'] === 'px') out.add(Math.abs(v['value']));
    for (const [k, x] of Object.entries(v)) if (k !== '$description' && k !== 'description') collect(x, numbersArePx, out);
  }
}

/** Files whose px are not lengths a declaration may use: breakpoints (queries only) and shadows (their own tokens). */
const NOT_SCALE = /breakpoint|shadow/i;

/** The px a kit declaration may use, and those an @media / @container condition may use (breakpoint.json); the same
 *  sets as @wisp/tokens' PX_SCALE and BREAKPOINT_PX (tools/check/test/ui.test.ts holds them equal). undefined when the
 *  token files don't exist yet. */
export function kitScale(root: string): { scale: Set<number>; breakpoints: Set<number> } | undefined {
  const dir = join(root, TOKENS_DIR);
  if (!existsSync(dir)) return undefined;
  const files = readdirSync(dir).filter((f) => f.endsWith('.json'));
  if (!files.length) return undefined;
  const scale = new Set<number>([0]);
  const breakpoints = new Set<number>();
  for (const f of files) {
    const json: unknown = JSON.parse(readFileSync(join(dir, f), 'utf8'));
    if (/breakpoint/i.test(f)) collect(json, true, breakpoints);
    else if (!NOT_SCALE.test(f)) collect(json, LENGTH_FILE.test(f), scale);
  }
  return { scale, breakpoints };
}
