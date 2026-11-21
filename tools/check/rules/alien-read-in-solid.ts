// AGENTS.md §UI (state has one home), round 324: the client's state (the entity store, settings, navigation, peers,
// drafts, a plugin store, a `signal`/`computed`, the shell's nav and selection) is alien-signals, which Solid does not
// follow. A Solid reactive scope that reads it directly runs once with what was there and never again: the palette
// showed only the tasks present when it opened (round 323). So in shipped UI code (packages/{shell,ui}, plugins/*/ui,
// apps/web), an alien read lexically inside a Solid reactive scope is an error unless it goes through the bridge:
// `useSignal(…)`/`useDerived(…)` (or alien's own `computed`/`effect`/`subscribe`, `untracked`, `.peek`).
//
// Parsing only, no type checker, so "an alien read" is what the syntax shows (each name resolved to its declaration):
// - a call with no arguments of a readable: `c.entities.row|list|keys|pending(…)`, `c.peers.list|status|info(…)`,
//   `c.settings.get|lockedBy(…)` (or of a `c.settings(decls)` handle), `c.drafts.get(…)`, `c.nav.nav`, a
//   `defineStore(…)` handle's `.value`, the shell's `nav`/`selection.get`/`sidebar.open|width`/`theme.pref|resolved`;
//   a `computed`/`signal`/selector (filterBy, sortBy, mapTo, count, some, groupBy) from @wisp/client; a family's
//   `f(arg)`; a const bound to any of these; a function that returns one (declared `: Readable<…>`, or returning one
//   of these); a property declared `Readable<…>` (and nowhere as something else) in the file or a module it imports;
// - a call of a reader: a function whose body makes such a read now (not in a getter it hands out), in the file or
//   imported from a relative module or a workspace package (`export … from` followed);
// and a reactive scope is the callback (or a reader handed over by name) of createMemo, createEffect, createComputed,
// createRenderEffect, createSelector, createDeferred,
// createResource's source (not its fetcher), `on`'s deps, and every JSX expression (children, attribute values, the
// render functions <For>/<Show> call), except `on*`/`ref` attributes (handlers run once, as asked). A function inside a
// scope counts as the scope when it's called there (a `.map` callback, a render function), not when it's the argument
// of a bridge or of something that runs later (onMount, onCleanup, queueMicrotask, a timer, a `.then`) or only made
// (a getter in an object, a named helper: its calls are judged where they are). A read that is right as it is says why
// on its line or the line above: `// check:allow alien-read-in-solid <reason>`.


import ts from 'typescript';
import { finding } from '../core/report.ts';
import { commentsOf, isShipped, lineOf, propName, unwrap, visit } from '../core/source.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';
import { CLIENT_MAKERS, calleeName, declOf, has, isFn, nameOf, isRead, isReadable, isReadableType, membersShape, shapeOfDecl, type Exports, type Kind, type Kinds, type Summary } from './alien-read-model.ts';

const ID = 'alien-read-in-solid';
const ALLOW = new RegExp(`check:allow\\s+${ID}\\s+\\S`);
const SOLID_SCOPES = new Set(['createMemo', 'createEffect', 'createComputed', 'createRenderEffect', 'createSelector', 'createDeferred']);
/** What they're given runs outside the reactive scope (later, once, or untracked). */
const LATER = new Set(['onMount', 'onCleanup', 'queueMicrotask', 'setTimeout', 'setInterval', 'requestAnimationFrame', 'then', 'catch', 'finally', 'untrack', 'timeout', 'interval', 'addEventListener', 'batch']);
/** The bridge and alien's own tracking: a read inside these is followed (or meant to be one-shot). */
const BRIDGES = new Set(['useSignal', 'useDerived', 'computed', 'effect', 'effectScope', 'subscribe', 'untracked']);

const attrName = (a: ts.JsxAttribute): string => (ts.isIdentifier(a.name) ? a.name.text : `${a.name.namespace.text}:${a.name.name.text}`);
const isHandlerAttr = (a: ts.JsxAttribute): boolean => /^(on[A-Z]|on:|use:|ref$)/.test(attrName(a));

const up = (n: ts.Node): ts.Node => {
  let p = n.parent;
  while (ts.isParenthesizedExpression(p) || ts.isAsExpression(p) || ts.isSatisfiesExpression(p)) p = p.parent;
  return p;
};
/** A function inside a scope that doesn't run in it: a bridge's or a later's argument, a handler, or only made. */
function isOutside(fn: ts.Node, s: Summary): boolean {
  let p = up(fn);
  // a function in an object (`{ task: (id) => rows().find(…) }`): handed to a call made now, or a prop of an element,
  // it's called as that runs or renders, unless it's a readable prop the kit bridges (a setting source's `value`)
  const member = ts.isMethodDeclaration(fn) ? fn : ts.isPropertyAssignment(p) ? p : undefined;
  if (member && ts.isObjectLiteralExpression(member.parent)) {
    const name = propName(member.name);
    if (name && s.props.has(name)) return true;
    const holder = up(member.parent);
    if (ts.isJsxExpression(holder)) return ts.isJsxAttribute(holder.parent) && isHandlerAttr(holder.parent);
    if (!ts.isCallExpression(holder) || !holder.arguments.includes(member.parent as ts.Expression)) return true;
    p = holder;
    const n = calleeName(holder);
    return n !== undefined && (BRIDGES.has(n) || LATER.has(n) || n === 'on');
  }
  // a callback runs now, in the scope (`.map(…)`, `.filter(…)`), unless a bridge or a later takes it
  if (ts.isCallExpression(p) && p.arguments.includes(fn as ts.Expression)) {
    const n = calleeName(p);
    // `on(deps, fn)`: fn runs untracked
    if (n === 'on') return p.arguments[0] !== fn;
    return n !== undefined && (BRIDGES.has(n) || LATER.has(n));
  }
  // a render function (<For>{(r) => …}</For>, <Show>{(v) => …}</Show>) or an accessor prop runs where it's tracked
  if (ts.isJsxExpression(p)) return ts.isJsxAttribute(p.parent) && isHandlerAttr(p.parent);
  // made, not run: a getter handed over in an object (`{ value: () => x() }`), returned, or bound to a name (a call of
  // that name is judged where it's called: the name is a reader)
  return true;
}

/** Each read in `node` that runs when it runs. */
function readsIn(node: ts.Node, s: Summary, out: ts.CallExpression[]): void {
  const walk = (n: ts.Node): void => {
    if (n !== node && (isFn(n) || ts.isFunctionDeclaration(n) || ts.isMethodDeclaration(n)) && isOutside(n, s)) return;
    if (ts.isJsxAttribute(n) && isHandlerAttr(n)) return;
    if (ts.isCallExpression(n)) {
      if (isRead(n, s)) out.push(n);
      // a bridge's arguments are its own (`useSignal(c.entities.keys(k))` hands the readable over)
      if (BRIDGES.has(calleeName(n) ?? '') || nameOf(n.expression) === 'peek') return walk(n.expression);
    }
    ts.forEachChild(n, walk);
  };
  walk(node);
}

function returnsReadable(fn: ts.FunctionLikeDeclaration, s: Summary): boolean {
  if (isReadableType(fn.type)) return true;
  const b = fn.body;
  if (!b) return false;
  if (!ts.isBlock(b)) return isReadable(b, s);
  let found = false;
  const walk = (x: ts.Node): void => {
    if (ts.isFunctionLike(x)) return;
    if (ts.isReturnStatement(x) && x.expression) found ||= isReadable(x.expression, s);
    ts.forEachChild(x, walk);
  };
  ts.forEachChild(b, walk);
  return found;
}

/** The function hands back a getter that reads (`hasExtensions(input)` → `() => resolved().length > 0`). */
function returnsReader(fn: ts.FunctionLikeDeclaration, s: Summary): boolean {
  const b = fn.body;
  if (!b) return false;
  const reads = (f: ts.Node): boolean => {
    const out: ts.CallExpression[] = [];
    readsIn(f, s, out);
    return out.length > 0;
  };
  if (!ts.isBlock(b)) return isFn(unwrap(b)) && reads(unwrap(b));
  let found = false;
  const walk = (x: ts.Node): void => {
    if (ts.isFunctionLike(x)) return;
    if (ts.isReturnStatement(x) && x.expression && isFn(unwrap(x.expression))) found ||= reads(unwrap(x.expression));
    ts.forEachChild(x, walk);
  };
  ts.forEachChild(b, walk);
  return found;
}

/** What each declaration of the file is, iterated until nothing changes (a reader of a reader). */
function summarize(sf: ts.SourceFile, seed: Summary): Summary {
  const s = seed;
  const decls: { node: ts.Node; init: ts.Node }[] = [];
  visit(sf, (n) => {
    if (ts.isFunctionDeclaration(n) && n.name) decls.push({ node: n, init: n });
    if (ts.isParameter(n) && n.type) decls.push({ node: n, init: n });
    if (ts.isInterfaceDeclaration(n)) {
      const m = membersShape(n.members);
      if (m) s.types.set(n.name.text, m);
    }
    if (ts.isTypeAliasDeclaration(n) && ts.isTypeLiteralNode(n.type)) {
      const m = membersShape(n.type.members);
      if (m) s.types.set(n.name.text, m);
    }
    if (ts.isVariableDeclaration(n) && ts.isIdentifier(n.name)) {
      if (isReadableType(n.type)) s.decls.set(n, new Set(['readable']));
      if (n.initializer) decls.push({ node: n, init: unwrap(n.initializer) });
    }
    if (ts.isPropertySignature(n) || ts.isPropertyDeclaration(n) || ts.isMethodSignature(n)) {
      const p = propName(n.name);
      if (p) (!ts.isMethodSignature(n) && isReadableType(n.type) ? s.props : s.otherProps).add(p);
    }
  });
  for (let changed = true; changed; ) {
    changed = false;
    const add = (node: ts.Node, k: Kind): void => {
      const ks = s.decls.get(node) ?? new Set<Kind>();
      if (!ks.has(k)) (ks.add(k), s.decls.set(node, ks), (changed = true));
    };
    for (const { node, init } of decls) {
      const shape = shapeOfDecl(node, node === init ? undefined : init, s);
      if (shape && !s.shapes.has(node)) (s.shapes.set(node, shape), (changed = true));
      if (ts.isParameter(init)) continue;
      if (ts.isCallExpression(init)) {
        const c = calleeName(init);
        if (c === 'settings' && ts.isPropertyAccessExpression(unwrap(init.expression))) add(node, 'settings');
        if (c === 'defineStore') add(node, 'store');
        if (c === 'family') add(node, 'family');
        if (c === 'useShell') add(node, 'shell');
        if (has(init.expression, s, 'readerMaker')) add(node, 'reader');
      }
      // `const e = c.entities`: its row(…)/list(…) are the store's
      if (ts.isPropertyAccessExpression(init) && (init.name.text === 'entities' || init.name.text === 'peers')) add(node, init.name.text as Kind);
      if (ts.isFunctionDeclaration(init) || isFn(init)) {
        if (returnsReadable(init, s)) add(node, 'maker');
        const reads: ts.CallExpression[] = [];
        if (init.body) readsIn(init.body, s, reads);
        if (reads.length > 0) add(node, 'reader');
        if (returnsReader(init, s)) add(node, 'readerMaker');
      } else if (ts.isExpression(init) && isReadable(init, s)) add(node, 'readable');
    }
  }
  return s;
}

/** The file a module specifier names: a relative file, or a UI workspace package's entry (or subpath export). */
function fileOf(ctx: Ctx, from: string, spec: string): string | undefined {
  const r = ctx.resolve(from, spec);
  if (r.kind === 'relative') return r.file;
  if (r.kind !== 'workspace' || (!r.pkg.ui && r.pkg.dir !== 'packages/shell')) return undefined;
  const exp = r.pkg.manifest.exports as Record<string, unknown> | undefined;
  const target = exp?.[r.subpath ? `./${r.subpath}` : '.'];
  if (typeof target !== 'string') return undefined;
  const file = `${r.pkg.dir}/${target.replace(/^\.\//, '')}`;
  return ctx.exists(file) ? file : undefined;
}

type Memo = Map<string, Exports | 'busy'>;
const noExports = (): Exports => ({ names: new Map(), shapes: new Map(), props: new Set(), otherProps: new Set(), types: new Map() });

/** What a module exports, by the names it exports them as (`export … from` followed, cycles cut). */
function exportsOf(ctx: Ctx, file: string, memo: Memo): Exports {
  const had = memo.get(file);
  if (had === 'busy') return noExports();
  if (had) return had;
  memo.set(file, 'busy');
  const sf = ctx.source(file);
  const s = summarize(sf, seedFromImports(ctx, file, sf, memo));
  const out: Exports = { names: new Map(), shapes: new Map(), props: s.props, otherProps: s.otherProps, types: s.types };
  const own = (name: string, d: ts.Node): void => {
    if (s.decls.has(d)) out.names.set(name, s.decls.get(d) as Kinds);
    if (s.shapes.has(d)) out.shapes.set(name, s.shapes.get(d) as Set<string>);
  };
  for (const st of sf.statements) {
    if (ts.isFunctionDeclaration(st) && st.name) own(st.name.text, st);
    if (ts.isVariableStatement(st)) for (const d of st.declarationList.declarations) if (ts.isIdentifier(d.name)) own(d.name.text, d);
    if (!ts.isExportDeclaration(st) || st.isTypeOnly) continue;
    const target = st.moduleSpecifier && ts.isStringLiteral(st.moduleSpecifier) ? fileOf(ctx, file, st.moduleSpecifier.text) : undefined;
    const theirs = target ? exportsOf(ctx, target, memo) : undefined;
    for (const p of theirs?.props ?? []) out.props.add(p);
    for (const p of theirs?.otherProps ?? []) out.otherProps.add(p);
    for (const [n, t] of theirs?.types ?? []) out.types.set(n, t);
    const c = st.exportClause;
    if (theirs && !c) {
      for (const [n, k] of theirs.names) out.names.set(n, k);
      for (const [n, k] of theirs.shapes) out.shapes.set(n, k);
    }
    if (c && ts.isNamedExports(c))
      for (const e of c.elements) {
        const from = (e.propertyName ?? e.name).text;
        // `export { a as b }` of the file's own `a` (declared above, so already in out.names)
        const k = theirs ? theirs.names.get(from) : out.names.get(from);
        if (k) out.names.set(e.name.text, k);
        const sh = theirs ? theirs.shapes.get(from) : out.shapes.get(from);
        if (sh) out.shapes.set(e.name.text, sh);
      }
  }
  memo.set(file, out);
  return out;
}

/** The file's imports, each specifier marked with what it is where it's declared; @wisp/client's selectors. */
function seedFromImports(ctx: Ctx, file: string, sf: ts.SourceFile, memo: Memo): Summary {
  const s: Summary = { decls: new Map(), props: new Set(), otherProps: new Set(), types: new Map(), shapes: new Map() };
  for (const st of sf.statements) {
    if (!ts.isImportDeclaration(st) || !ts.isStringLiteral(st.moduleSpecifier)) continue;
    const spec = st.moduleSpecifier.text;
    const nb = st.importClause?.namedBindings;
    const named = nb && ts.isNamedImports(nb) ? nb.elements : [];
    if (spec === '@wisp/client' || spec === 'alien-signals') {
      for (const e of named) if (!e.isTypeOnly && CLIENT_MAKERS.has((e.propertyName ?? e.name).text)) s.decls.set(e, new Set(['maker']));
      continue;
    }
    const target = fileOf(ctx, file, spec);
    if (!target) continue;
    const theirs = exportsOf(ctx, target, memo);
    for (const p of theirs.props) s.props.add(p);
    for (const p of theirs.otherProps) s.otherProps.add(p);
    for (const e of named) {
      const theirName = (e.propertyName ?? e.name).text;
      const t = theirs.types.get(theirName);
      if (t) s.types.set(e.name.text, t);
      if (e.isTypeOnly || st.importClause?.isTypeOnly) continue;
      const k = theirs.names.get(theirName);
      if (k) s.decls.set(e, new Set(k));
      const sh = theirs.shapes.get(theirName);
      if (sh) s.shapes.set(e, sh);
    }
  }
  return s;
}

/** The local names of solid-js's reactive primitives the file imports (`createMemo as memo` counts). */
function solidNames(sf: ts.SourceFile): Map<string, string> {
  const m = new Map<string, string>();
  for (const st of sf.statements) {
    if (!ts.isImportDeclaration(st) || !ts.isStringLiteral(st.moduleSpecifier) || st.moduleSpecifier.text !== 'solid-js') continue;
    const nb = st.importClause?.namedBindings;
    if (nb && ts.isNamedImports(nb)) for (const e of nb.elements) m.set(e.name.text, (e.propertyName ?? e.name).text);
  }
  return m;
}

/** The roots of the Solid reactive scopes in a file: tracked callbacks and JSX expressions. */
function scopesOf(sf: ts.SourceFile, solid: ReadonlyMap<string, string>): ts.Node[] {
  const out: ts.Node[] = [];
  visit(sf, (n) => {
    if (ts.isCallExpression(n) && ts.isIdentifier(n.expression)) {
      const what = solid.get(n.expression.text);
      const a = n.arguments;
      if (what && SOLID_SCOPES.has(what) && a[0]) out.push(a[0]);
      if (what === 'createResource' && a.length >= 2 && a[0] && isFn(a[0])) out.push(a[0]);
      if (what === 'on' && a[0]) out.push(a[0]);
    }
    if (ts.isJsxExpression(n) && n.expression && !(ts.isJsxAttribute(n.parent) && isHandlerAttr(n.parent))) out.push(n.expression);
  });
  return out;
}

/** The alien reads inside Solid reactive scopes of one file, each call once (nested scopes overlap). */
export function alienReads(ctx: Ctx, file: string, memo: Memo = new Map()): ts.CallExpression[] {
  const sf = ctx.source(file);
  const solid = solidNames(sf);
  if (solid.size === 0 && !file.endsWith('.tsx')) return [];
  const s = summarize(sf, seedFromImports(ctx, file, sf, memo));
  const found = new Set<ts.CallExpression>();
  for (const root of scopesOf(sf, solid)) {
    // `createMemo(compute)`: a reader handed over by name runs as the scope
    if (ts.isIdentifier(root) && has(root, s, 'reader') && ts.isCallExpression(root.parent)) found.add(root.parent);
    const reads: ts.CallExpression[] = [];
    readsIn(root, s, reads);
    for (const r of reads) found.add(r);
  }
  return [...found];
}

/** UI code that ships: packages/{shell,ui}, plugins/*\/ui, apps/web (the client package is framework independent). */
const inScope = (ctx: Ctx, file: string): boolean => {
  const pkg = ctx.pkgOf(file);
  return !!pkg && (pkg.ui || pkg.dir === 'packages/shell') && isShipped(file, pkg.dir);
};

export const alienReadInSolid: Rule = {
  id: ID,
  description: 'a Solid reactive scope (createMemo/createEffect/JSX) reads the client store (alien signals) through useSignal/useDerived, or it never sees a change',
  run(ctx) {
    const out: Finding[] = [];
    const memo: Memo = new Map();
    for (const file of ctx.files) {
      if (!inScope(ctx, file)) continue;
      const reads = alienReads(ctx, file, memo);
      if (reads.length === 0) continue;
      const sf = ctx.source(file);
      const allowed = new Set(commentsOf(sf).filter((c) => ALLOW.test(c.text)).map((c) => lineOf(sf, c.pos)));
      for (const line of new Set(reads.map((r) => lineOf(sf, r)))) {
        if (allowed.has(line) || allowed.has(line - 1)) continue;
        out.push(finding(file, line, ID, `an alien-signals read (the client store, settings, nav, a signal/computed) inside a Solid reactive scope (createMemo/createEffect/JSX): Solid doesn't follow it, so it shows the value it had when it ran (a late, renamed or deleted row never shows). Read it through useSignal(…) or useDerived(…) (a real reason: \`// check:allow ${ID} <reason>\`)`));
      }
    }
    return out;
  },
};
