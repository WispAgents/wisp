// The rules every kit stylesheet follows on the tokens, in one place both gates run: `npm run check`'s css rule
// (tools/check/rules/css.ts) and the kit's vitest (packages/ui/test/css-rules.test.ts). A sheet fails both or
// neither (round 244: the check passed a raw 600px query and a data-part "…wrap" flex-wrap that the vitest refused).
// Pure: no fs, no node types, no dependency, so the web tsconfig and the tools program both compile it.

// ## Scanner: a small CSS scanner: enough to find selectors, at-rule preludes and declarations with their lines.
// Not a parser; the kit's CSS is plain and Lightning CSS validates it at build time.

export interface CssPrelude {
  text: string;
  line: number;
  at: boolean; // '@media …', '@layer …'
}
export interface CssDecl {
  prop: string;
  value: string;
  line: number;
  /** the preludes it sits in, outermost first: ['@layer reset', '[hidden]'] */
  within: string[];
}

/** Comments become spaces (newlines kept) so lines stay right. */
function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '));
}

export function scanCss(input: string): { preludes: CssPrelude[]; decls: CssDecl[] } {
  const css = stripComments(input);
  const preludes: CssPrelude[] = [];
  const decls: CssDecl[] = [];
  let line = 1;
  let start = 0;
  let startLine = 1;
  let depth = 0; // parentheses
  let quote: string | undefined;
  const stack: string[] = [];
  const flush = (end: number, kind: 'prelude' | 'decl'): void => {
    const raw = css.slice(start, end);
    const lead = raw.length - raw.trimStart().length;
    const text = raw.trim();
    const at = startLine + (raw.slice(0, lead).match(/\n/g)?.length ?? 0);
    if (text) {
      if (kind === 'prelude') preludes.push({ text, line: at, at: text.startsWith('@') });
      else {
        const colon = text.indexOf(':');
        if (colon > 0 && !text.startsWith('@')) decls.push({ prop: text.slice(0, colon).trim(), value: text.slice(colon + 1).trim(), line: at, within: [...stack] });
      }
    }
    if (kind === 'prelude') stack.push(text);
  };
  for (let i = 0; i < css.length; i++) {
    const ch = css[i];
    if (ch === '\n') line++;
    if (quote) {
      if (ch === '\\') {
        if (css[i + 1] === '\n') line++;
        i++;
      }
      else if (ch === quote) quote = undefined;
      continue;
    }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '(') depth++;
    else if (ch === ')') depth = Math.max(0, depth - 1);
    else if (depth === 0 && (ch === '{' || ch === ';' || ch === '}')) {
      flush(i, ch === '{' ? 'prelude' : 'decl');
      if (ch === '}') stack.pop();
      start = i + 1;
      startLine = line;
    }
  }
  flush(css.length, 'decl');
  return { preludes, decls };
}

/** Splits on commas outside parentheses and brackets. */
export function splitTop(s: string, sep = ','): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of s) {
    if (ch === '(' || ch === '[') depth++;
    if (ch === ')' || ch === ']') depth--;
    if (ch === sep && depth === 0) {
      out.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

// ## Layout: design/layout.md §6 ("nothing wraps that shouldn't"). In kit CSS:
//   - `flex-wrap: wrap` (or wrap-reverse, or a `flex-flow` that wraps) only inside the `Wrap` layout:
//     a bar collapses in a declared order, it never wraps its row (§2, §5);
//   - control text never wraps: no `white-space: normal | pre-wrap | pre-line | break-spaces` and no
//     `text-wrap: wrap | balance | pretty` on a control part (§1).
// A rule is Wrap's when every selector of its own rule targets the Wrap layout itself: its last compound names
// `[data-kit='wrap']` or `[data-layout='wrap']` (layout.css). A group that wraps puts `data-layout="wrap"` on its
// element (or is a `Wrap`); a data-part named "…wrap" or a child of a Wrap is not a way around it (round 244).
// Control parts are `[data-text='control']` and the kit's control components.

const WRAP_SUBJECT = /\[data-(kit|layout)=(['"]?)wrap\2\]/;
const CONTROL_SCOPE = /\[data-text=['"]?control\b|\[data-kit=['"]?(button|icon-button|pill|chip|tab|tabs-trigger|menu-item|list-row-title|toolbar|bar|badge|breadcrumb|time|count)\b/;
const WRAPS_FLEX = /(^|\s)wrap(-reverse)?(\s|$)/;
const WRAPS_TEXT = /^(normal|pre-wrap|pre-line|break-spaces)\b/;
const WRAPS_TEXT_WRAP = /^(wrap|balance|pretty)\b/;

export interface LayoutProblem {
  line: number;
  message: string;
}

/** The last compound of a selector: '.a > [x] [y]:hover' → '[y]:hover'. */
export function subjectOf(selector: string): string {
  let depth = 0;
  const s = selector.trim();
  for (let i = s.length - 1; i >= 0; i--) {
    const ch = s[i] ?? '';
    if (ch === ')' || ch === ']') depth++;
    else if (ch === '(' || ch === '[') depth--;
    else if (depth === 0 && /[\s>+~]/.test(ch)) return s.slice(i + 1);
  }
  return s;
}

/** A compound without its functional pseudo-classes: "[a]:not([b]):where([c])" → "[a]" (a Wrap named inside
 *  :not() or :where() doesn't make the element a Wrap). */
function outsideParens(compound: string): string {
  let depth = 0;
  let out = '';
  for (const ch of compound) {
    if (ch === '(') depth++;
    if (depth === 0) out += ch;
    if (ch === ')') depth = Math.max(0, depth - 1);
  }
  return out;
}

/** True when the rule a declaration sits in is the Wrap layout's own (every selector's subject is Wrap). */
export function isWrapRule(d: CssDecl): boolean {
  const own = d.within.filter((p) => !p.startsWith('@')).at(-1);
  if (!own) return false;
  return splitTop(own).every((sel) => WRAP_SUBJECT.test(outsideParens(subjectOf(sel))));
}

function selectors(d: CssDecl): string {
  return d.within.filter((p) => !p.startsWith('@')).join(' ');
}

export function layoutProblems(decls: readonly CssDecl[]): LayoutProblem[] {
  const out: LayoutProblem[] = [];
  for (const d of decls) {
    const prop = d.prop.toLowerCase();
    const value = d.value.toLowerCase().trim();
    const where = selectors(d);
    if ((prop === 'flex-wrap' || prop === 'flex-flow') && WRAPS_FLEX.test(value) && !isWrapRule(d)) {
      out.push({ line: d.line, message: `${prop}: ${value} outside Wrap's own rule: put data-layout="wrap" on the group (a bar collapses its items, it never wraps its row: design/layout.md §2, §5)` });
    }
    const textWraps = (prop === 'white-space' && WRAPS_TEXT.test(value)) || (prop === 'text-wrap' && WRAPS_TEXT_WRAP.test(value));
    if (textWraps && CONTROL_SCOPE.test(where)) {
      out.push({ line: d.line, message: `${prop}: ${value} on a control part: control text is nowrap + ellipsis (design/layout.md §1)` });
    }
  }
  return out;
}

// ## px, and the whole sheet
// A declaration's px is on the token scale (PX_SCALE); a px in an @media / @container condition is a breakpoint token
// (BREAKPOINT_PX, tokens/breakpoint.json: CSS queries can't read variables, so the value is written out).

export interface KitScale {
  /** px a declaration may use (space, radius, font size, size). */
  readonly scale: ReadonlySet<number>;
  /** px an @media / @container condition may use. */
  readonly breakpoints: ReadonlySet<number>;
}

export interface KitCssProblem {
  readonly kind: 'px' | 'layout';
  readonly line: number;
  readonly message: string;
}

const TOKENS = 'packages/tokens/tokens';
const PX = /(?<![\w.#-])(-?\d*\.?\d+)px\b/g;
const QUERY = /^@(media|container)\b/;

/** px inside a string or url() is content (an inline SVG), not a length. */
function lengths(v: string): string {
  return v.replace(/url\([^)]*\)/g, '').replace(/"[^"]*"|'[^']*'/g, '');
}

/** Every px and layout problem in one kit stylesheet. `sets` undefined: the token files don't exist, px isn't checked. */
export function kitCssProblems(css: string, sets: KitScale | undefined): KitCssProblem[] {
  const out: KitCssProblem[] = [];
  const { preludes, decls } = scanCss(css);
  const px = (text: string, line: number, query: boolean): void => {
    if (!sets) return;
    for (const m of text.matchAll(PX)) {
      const v = Math.abs(Number(m[1]));
      if (v === 0) continue;
      if (query && !sets.breakpoints.has(v)) out.push({ kind: 'px', line, message: `${m[0]} in a query is not a breakpoint token (${TOKENS}/breakpoint.json): add one or use one` });
      if (!query && !sets.scale.has(v)) out.push({ kind: 'px', line, message: `${m[0]} is off the token scale (${TOKENS}): use a token` });
    }
  };
  for (const d of decls) if (!d.prop.startsWith('--')) px(lengths(d.value), d.line, false);
  for (const p of preludes) if (p.at) px(p.text, p.line, QUERY.test(p.text));
  for (const p of layoutProblems(decls)) out.push({ kind: 'layout', ...p });
  return out.sort((a, b) => a.line - b.line);
}

// ## The kit's own sheets (packages/ui/src/css/*.css): every rule the kit's vitest enforces, one implementation
// (round 265: the vitest refused `:is(` in queued.css while `npm run check` passed it). Each problem names its rule,
// so the vitest asserts one rule per test and the check reports them all.

export type KitSheetRule =
  | 'px'
  | 'layout'
  | 'layer-order'
  | 'layer-block'
  | 'has-is'
  | 'sibling-universal'
  | 'colour'
  | 'class'
  | 'white-space-normal'
  | 'pressed-translate'
  | 'head-title-wraps';

export interface KitSheetProblem {
  readonly kind: KitSheetRule;
  readonly line: number;
  readonly message: string;
}

export const LAYER_ORDER = '@layer reset, tokens, kit, plugins;';

function lineAt(css: string, index: number): number {
  let n = 1;
  for (let i = 0; i < index && i < css.length; i++) if (css[i] === '\n') n++;
  return n;
}

/** Every rule a kit sheet breaks. `name` is its path under packages/ui/src/css ('index.css', 'composites.css'). */
export function kitSheetProblems(name: string, raw: string, sets: KitScale | undefined): KitSheetProblem[] {
  const css = stripComments(raw);
  const out: KitSheetProblem[] = kitCssProblems(raw, sets).map((p) => ({ ...p }));
  const add = (kind: KitSheetRule, index: number, message: string): void => {
    out.push({ kind, line: lineAt(css, index), message });
  };
  const index = name === 'index.css';
  // one layer order, in index.css, plugins last
  const orders = [...css.matchAll(/@layer\s+[\w\s,]+;/g)];
  for (const m of orders) if (!index || m[0] !== LAYER_ORDER || m !== orders[0]) add('layer-order', m.index ?? 0, `${m[0]}: the layer order is declared once, in index.css (${LAYER_ORDER})`);
  if (index && !css.trim().startsWith(LAYER_ORDER)) add('layer-order', 0, `index.css starts with ${LAYER_ORDER}`);
  // every other sheet is one @layer kit / reset block, nothing outside it
  if (!index) {
    const body = css.trim();
    const lead = css.length - css.trimStart().length;
    if (!/^@layer (kit|reset) \{/.test(body)) add('layer-block', lead, 'a kit sheet is one @layer kit { … } or @layer reset { … } block');
    else {
      let depth = 0;
      let closedAt = -1;
      for (let i = 0; i < body.length; i++) {
        if (body[i] === '{') depth++;
        if (body[i] === '}' && --depth === 0) {
          closedAt = i;
          break;
        }
      }
      if (closedAt !== body.length - 1) add('layer-block', lead + Math.max(0, closedAt + 1), 'something sits outside the sheet\'s @layer block');
    }
  }
  // no :has() and no :is(): they make every style recalc walk large boxes
  for (const m of css.matchAll(/:(has|is)\(/g)) add('has-is', m.index ?? 0, `${m[0]} in a kit sheet: every style recalc walks large boxes (name the part, or use :where())`);
  // F-phone-8: no `X + *` / `X > * + *`: every element is tried, every parent marked sibling-affected
  const at = (m: RegExpMatchArray): number => (m.index ?? 0) + ((m[1] ?? '').length - (m[1] ?? '').trimStart().length);
  for (const m of css.matchAll(/([^{}]*)\{/g)) {
    const sel = (m[1] ?? '').trim();
    if (sel.startsWith('@')) continue;
    for (const one of sel.split(',')) {
      const s = one.trim();
      if (/[+~]\s*\*?\s*(::?[\w-]+(\([^)]*\))?)*\s*$/.test(s) && /[+~]\s*(\*|::?|$)/.test(s)) add('sibling-universal', at(m), `a sibling combinator lands on a universal selector: ${s} (name what follows, or :where(X) > :not(:first-child))`);
    }
  }
  // no hard-coded colours (index.css holds the token values)
  if (!index)
    for (const m of css.matchAll(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|:\s*(white|black)\b/g)) add('colour', m.index ?? 0, `hard-coded colour ${m[0].trim()}: use a colour token`);
  // the kit styles by data-kit attributes, never by class names (decimals like 0.5 never follow a combinator)
  for (const m of css.matchAll(/(^|[\s,>+~(])\.[a-zA-Z_-][\w-]*\s*[{,:[]/gm)) add('class', (m.index ?? 0) + (m[1]?.length ?? 0), `class selector ${m[0].trim()}: style by data-kit / data-part`);
  // control text never gets white-space: normal
  for (const m of css.matchAll(/white-space:\s*normal/g)) add('white-space-normal', m.index ?? 0, 'white-space: normal: control text is nowrap + ellipsis (design/layout.md §1)');
  for (const m of css.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    const [, sel = '', body = ''] = m;
    // v1 #782: the press is a scale transform; a button centred by translate(-50%) jumps under the finger
    if (/(button|btn|icon-button|link)/.test(sel) && /transform:\s*translate(X)?\(\s*-50%/.test(body)) add('pressed-translate', at(m), `${sel.trim()}: a pressable centred by translate(-50%) jumps when pressed`);
    // v1 #778: the task head's title is one line at every width
    if (sel.includes("[data-kit='sh-title']") && /white-space:\s*(normal|pre-wrap)|line-clamp|text-wrap-mode:\s*wrap/.test(body)) add('head-title-wraps', at(m), `${sel.trim()}: the task head's title is one line at every width`);
  }
  return out.sort((a, b) => a.line - b.line);
}

// ## The phone apps' sheets (packages/ui/src/css/platform/<look>.css, ADR-0065 §5): every ban the platform vitest
// (packages/ui/test/platform-css.test.ts) enforces, one implementation (rounds 288 and 291: a raw px passed
// `npm run check` that the vitest refused). They restate the layer order (they load on their own), every rule is
// scoped to its look, and they use the platform's own tokens: no raw px at all, except in a size-class query.

export type PlatformSheetRule = 'layout' | 'platform-px' | 'platform-query' | 'platform-scope' | 'has-is' | 'colour' | 'wrap' | 'class' | 'sibling-universal' | 'title-wraps';

export interface PlatformSheetProblem {
  readonly kind: PlatformSheetRule;
  readonly line: number;
  readonly message: string;
}

/** M3 window size classes (compact < 600 dp, expanded ≥ 840 dp) and the browser's split (900): the only px in a query. */
export const PLATFORM_BREAKPOINTS: ReadonlySet<number> = new Set([600, 840, 900]);

export interface PlatformRule {
  readonly selector: string;
  readonly body: string;
  /** The at-rules it sits in, joined by a space ('@layer kit @media (max-width: 600px)'). */
  readonly at: string;
  /** Index of the selector's start in the comment-stripped sheet. */
  readonly index: number;
}

/** Every rule of a platform sheet with the at-rules it sits in (comments stripped, line positions kept). */
export function platformRules(raw: string): PlatformRule[] {
  const css = stripComments(raw);
  const out: PlatformRule[] = [];
  const stack: string[] = [];
  let head = '';
  let start = 0;
  for (let i = 0; i < css.length; i++) {
    const c = css[i] ?? '';
    if (c === '{') {
      const sel = head.trim();
      const index = start + (head.length - head.trimStart().length);
      head = '';
      start = i + 1;
      if (sel.startsWith('@')) {
        stack.push(sel);
        continue;
      }
      const end = css.indexOf('}', i);
      const stop = end < 0 ? css.length : end;
      out.push({ selector: sel, body: css.slice(i + 1, stop), at: stack.join(' '), index });
      i = stop;
      start = i + 1;
    } else if (c === '}') {
      stack.pop();
      head = '';
      start = i + 1;
    } else if (c === ';' && stack.length === 0) {
      head = '';
      start = i + 1;
    } else head += c;
  }
  return out;
}

/** Every ban a phone sheet breaks. `look` is its file name without .css ('android', 'ios'). */
export function platformSheetProblems(look: string, raw: string): PlatformSheetProblem[] {
  const css = stripComments(raw);
  const out: PlatformSheetProblem[] = [];
  const add = (kind: PlatformSheetRule, index: number, message: string): void => {
    out.push({ kind, line: lineAt(css, index), message: `${look}: ${message}` });
  };
  // every rule sits in @layer kit under its own look; a keyframe's steps are no selectors, its name is the look's own
  for (const r of platformRules(raw)) {
    if (!/^@layer kit/.test(r.at)) add('platform-scope', r.index, `${r.selector} sits outside @layer kit`);
    const frames = /@keyframes ([\w-]+)/.exec(r.at);
    if (frames) {
      if (!(frames[1] ?? '').startsWith(`k-${look}-`)) add('platform-scope', r.index, `@keyframes ${frames[1] ?? ''} is not named k-${look}-…`);
      continue;
    }
    for (const one of r.selector.split(',')) if (!one.trim().startsWith(`:root[data-platform='${look}'] `)) add('platform-scope', r.index, `${one.trim()} is not scoped to :root[data-platform='${look}']`);
    // the bar title is one line (content.css: nowrap, ellipsis)
    if (/\[data-kit='app-bar-title'\]$/.test(r.selector) && /white-space:\s*(normal|pre-wrap)|line-clamp/.test(r.body)) add('title-wraps', r.index, `${r.selector}: the bar title never wraps`);
  }
  // px: a query's only px are the size classes; anywhere else a raw px is a missed platform token
  const body = css.replace(/@import[^;]*;/g, (m) => m.replace(/[^\n]/g, ' '));
  for (const m of body.matchAll(/(@media[^{]*)\{|\d ?px/g)) {
    if (m[1]) {
      for (const q of m[1].matchAll(/(\d+)px/g)) if (!PLATFORM_BREAKPOINTS.has(Number(q[1]))) add('platform-query', (m.index ?? 0) + (q.index ?? 0), `${q[0]} in a query is not a size class (${[...PLATFORM_BREAKPOINTS].join(', ')})`);
    } else add('platform-px', m.index ?? 0, `raw px (…${body.slice(Math.max(0, (m.index ?? 0) - 6), (m.index ?? 0) + m[0].length).trim()}): use the platform's token`);
  }
  for (const m of css.matchAll(/:(has|is)\(/g)) add('has-is', m.index ?? 0, `${m[0]}: every style recalc walks large boxes`);
  for (const m of css.matchAll(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|:\s*(white|black)\b/g)) add('colour', m.index ?? 0, `hard-coded colour ${m[0].trim()}: use a colour token`);
  for (const m of css.matchAll(/flex-wrap:\s*wrap|white-space:\s*normal/g)) add('wrap', m.index ?? 0, `${m[0]}: nothing wraps but Wrap (design/layout.md)`);
  for (const m of css.matchAll(/(^|[\s,>+~(])\.[a-zA-Z_-][\w-]*\s*[{,:[]/gm)) add('class', (m.index ?? 0) + (m[1]?.length ?? 0), `class selector ${m[0].trim()}: style by data-kit / data-part`);
  for (const m of css.matchAll(/[+~]\s*\*/g)) add('sibling-universal', m.index ?? 0, `${m[0]}: a sibling combinator lands on a universal selector`);
  // design/layout.md §6, as on every kit sheet: no flex-wrap outside Wrap, no wrapping white-space on control parts
  for (const p of layoutProblems(scanCss(raw).decls)) out.push({ kind: 'layout', line: p.line, message: `${look}: ${p.message}` });
  return out.sort((a, b) => a.line - b.line);
}
