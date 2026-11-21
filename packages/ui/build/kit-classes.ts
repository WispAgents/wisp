// The kit's class mirror (ADR-0104, perf audit 2026-11-18 #5): the kit styles itself through `data-kit`, but Blink
// buckets attribute selectors by attribute name only, so every element with a `data-kit` was tried against all ~3,000
// `[data-kit=…]` rules on every style recalc (2,000 rules per element; 35–45 % of each interaction's style time).
// Class selectors are bucketed by class name. This build step keeps the source as it is (data-kit, the readable
// contract every test and check reads) and, in the bundles only:
//   - JSX: every element with a `data-kit` also gets `class="k-<kit>"` (static, or from the same expression);
//   - kit CSS: `[data-kit=x]` becomes `.k-x`, except the kits written outside JSX (KEEP_ATTR), whose elements have no
//     class and keep their attribute selector.
// Used by apps/web's and the gallery's Vite configs. Text edits only, on the same line, with a sourcemap that keeps
// every original column.
import ts from 'typescript';
import type { Plugin } from 'vite';

/**
 * Kits written outside JSX (HTML strings, setAttribute, or JSX outside the kit's transform), so their elements carry
 * no `k-` class: their rules stay `[data-kit=…]`. test/kit-classes.test.ts fails when an imperative writer of another
 * kit appears without being listed here.
 */
export const KEEP_ATTR: ReadonlySet<string> = new Set([
  'term-row', // terminal/paint.ts: rows built as HTML strings and with setAttribute
  'term-probe', // terminal/Terminal.tsx: the measuring probe, setAttribute
  'sheet', // controls/phone.tsx: the phone sheet, setAttribute (Dialog's JSX sheet shares the name)
  'sheet-handle', // controls/phone.tsx, setAttribute
  'plain-tip', // controls/phone.tsx, setAttribute
  'plain-tip-hint', // controls/phone.tsx, setAttribute
]);

/** One insertion into the source: `text` at offset `at`. */
interface Edit {
  readonly at: number;
  readonly text: string;
}

/** An intrinsic element (`div`, `svg:g`), or Solid's Dynamic, which hands `class` to the element it draws. */
const takesClass = (tag: string): boolean => /^[a-z]/.test(tag) || tag === 'Dynamic';

const hasClass = (attrs: ts.JsxAttributes, sf: ts.SourceFile): boolean =>
  attrs.properties.some((p) => ts.isJsxAttribute(p) && ['class', 'className', 'classList'].includes(p.name.getText(sf)));

/** The edits that give every `data-kit` element (and `'data-kit'` prop object) its `k-` class. */
export function jsxEdits(code: string, file: string): Edit[] {
  const sf = ts.createSourceFile(file, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const edits: Edit[] = [];
  const visit = (n: ts.Node): void => {
    if (ts.isJsxAttribute(n) && n.name.getText(sf) === 'data-kit' && n.initializer) {
      const owner = n.parent.parent;
      if (takesClass(owner.tagName.getText(sf)) && !hasClass(n.parent, sf)) {
        const init = n.initializer;
        if (ts.isStringLiteral(init)) edits.push({ at: n.end, text: ` class="k-${init.text}"` });
        else if (ts.isJsxExpression(init) && init.expression) edits.push({ at: n.end, text: ` class={\`k-\${${init.expression.getText(sf)}}\`}` });
      }
    } else if (ts.isPropertyAssignment(n) && ts.isStringLiteral(n.name) && n.name.text === 'data-kit' && ts.isStringLiteral(n.initializer)) {
      // a props object spread onto an element (anchored-card's attrs())
      edits.push({ at: n.end, text: `, class: 'k-${n.initializer.text}'` });
    }
    ts.forEachChild(n, visit);
  };
  visit(sf);
  return edits;
}

const ATTR_KIT = /\[data-kit=(['"]?)([\w-]+)\1\]/g;

/** A kit stylesheet with `[data-kit=x]` turned into `.k-x` (KEEP_ATTR's kits stay); selectors keep their lines. */
export function cssClasses(code: string): string {
  return code.replace(ATTR_KIT, (whole, _q: string, kit: string) => (KEEP_ATTR.has(kit) ? whole : `.k-${kit}`));
}

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function vlq(n: number): string {
  let v = n < 0 ? (-n << 1) | 1 : n << 1;
  let out = '';
  do {
    let digit = v & 31;
    v >>>= 5;
    if (v > 0) digit |= 32;
    out += B64[digit];
  } while (v > 0);
  return out;
}

/** Apply insertions and build a sourcemap where every original column still maps to itself. */
export function applyEdits(code: string, edits: readonly Edit[], source: string): { readonly code: string; readonly map: string } {
  const sorted = [...edits].sort((a, b) => a.at - b.at);
  let out = '';
  let last = 0;
  for (const e of sorted) {
    out += code.slice(last, e.at) + e.text;
    last = e.at;
  }
  out += code.slice(last);
  // per original line: segments at generated column 0, and after each insertion on that line
  const lines = code.split('\n');
  const starts: number[] = [];
  let off = 0;
  for (const l of lines) {
    starts.push(off);
    off += l.length + 1;
  }
  let ei = 0;
  let prevOrigCol = 0;
  let prevLine = 0;
  const mapped: string[] = [];
  for (let line = 0; line < lines.length; line++) {
    const start = starts[line] ?? 0;
    const end = start + (lines[line]?.length ?? 0);
    const segs: string[] = [];
    let genCol = 0;
    let prevGen = 0;
    let shift = 0;
    const seg = (gen: number, orig: number): void => {
      segs.push(vlq(gen - prevGen) + vlq(0) + vlq(line - prevLine) + vlq(orig - prevOrigCol));
      prevGen = gen;
      prevLine = line;
      prevOrigCol = orig;
    };
    seg(0, 0);
    while (ei < sorted.length && (sorted[ei]?.at ?? Infinity) <= end) {
      const e = sorted[ei] as Edit; // the loop's guard read it
      shift += e.text.length;
      genCol = e.at - start + shift;
      seg(genCol, e.at - start);
      ei++;
    }
    mapped.push(segs.join(','));
  }
  const map = JSON.stringify({ version: 3, sources: [source], sourcesContent: [code], names: [], mappings: mapped.join(';') });
  return { code: out, map };
}

const KIT_CSS = /[\\/]packages[\\/]ui[\\/]src[\\/].*\.css$/;

/**
 * The Vite plugins. JSX before Solid compiles it. CSS once Vite has put it together, because the kit's sheets arrive
 * through `@import`s that Vite inlines without running other plugins on them: in a build the emitted CSS files, in
 * dev the CSS module Vite serves.
 */
export function kitClasses(): Plugin[] {
  return [
    {
      name: 'wisp-kit-classes:jsx',
      enforce: 'pre',
      transform(code, id) {
        const [file = '', query = ''] = id.split('?');
        if (!file.endsWith('.tsx') || file.includes('/node_modules/') || /\braw\b/.test(query) || !code.includes('data-kit')) return null;
        const edits = jsxEdits(code, file);
        return edits.length ? applyEdits(code, edits, file) : null;
      },
    },
    {
      name: 'wisp-kit-classes:css-dev',
      apply: 'serve',
      enforce: 'post',
      transform(code, id) {
        const [file = '', query = ''] = id.split('?');
        if (!KIT_CSS.test(file) || /\braw\b/.test(query) || !code.includes('data-kit')) return null;
        const out = cssClasses(code);
        return out === code ? null : { code: out, map: null };
      },
    },
    {
      name: 'wisp-kit-classes:css-build',
      apply: 'build',
      generateBundle(_options, bundle) {
        for (const asset of Object.values(bundle)) {
          if (asset.type !== 'asset' || !asset.fileName.endsWith('.css')) continue;
          const text = typeof asset.source === 'string' ? asset.source : new TextDecoder().decode(asset.source);
          if (text.includes('data-kit')) asset.source = cssClasses(text);
        }
      },
    },
  ];
}
