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
