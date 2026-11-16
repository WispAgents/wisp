// The syntax model of alien-read-in-solid (./alien-read-in-solid.ts): what, without a type checker, is a readable of
// the client's alien signals, a reader of one, or an object holding some, each name resolved to its declaration.

import ts from 'typescript';
import { propName, unwrap } from '../core/source.ts';

export const CLIENT_MAKERS = new Set(['computed', 'signal', 'filterBy', 'sortBy', 'mapTo', 'count', 'some', 'groupBy']);
const ENTITY = new Set(['row', 'list', 'keys', 'pending']);
const PEERS = new Set(['list', 'status', 'info']);
const SETTINGS = new Set(['get', 'lockedBy']);
/** Zero-argument methods of built-ins: a property of that name declared Readable somewhere doesn't make these reads. */
const BUILTIN = new Set(['keys', 'values', 'entries', 'toString', 'valueOf', 'pop', 'shift', 'reverse', 'sort', 'trim', 'next', 'clear', 'focus', 'blur', 'click', 'now']);

/** What a declared name is. */
export type Kind = 'maker' | 'readable' | 'reader' | 'readerMaker' | 'settings' | 'store' | 'family' | 'shell' | 'entities' | 'peers';
export type Kinds = Set<Kind>;

export interface Summary {
  /** what each declaration in the file (or import specifier) is */
  decls: Map<ts.Node, Kinds>;
  /** properties declared Readable, and those declared as anything else (a name in both is not judged) */
  props: Set<string>;
  otherProps: Set<string>;
  /** an interface or type literal's Readable props, by its name (`SettingSource` → value, lockedBy) */
  types: Map<string, Set<string>>;
  /** a declaration that holds (or a function that returns) an object with readable props: `fromRow(…)`'s `value` */
  shapes: Map<ts.Node, Set<string>>;
}
/** A module's exports by name. */
export interface Exports {
  names: Map<string, Kinds>;
  shapes: Map<string, Set<string>>;
  props: Set<string>;
  otherProps: Set<string>;
  types: Map<string, Set<string>>;
}

export const nameOf = (e: ts.Expression): string | undefined => {
  const u = unwrap(e);
  if (ts.isIdentifier(u)) return u.text;
  if (ts.isPropertyAccessExpression(u)) return u.name.text;
  return undefined;
};
export const calleeName = (c: ts.CallExpression): string | undefined => nameOf(c.expression);
export const isReadableType = (t: ts.TypeNode | undefined): boolean => !!t && /^(Readable|WritableSignal)\s*</.test(t.getText());
export const isFn = (n: ts.Node): n is ts.ArrowFunction | ts.FunctionExpression => ts.isArrowFunction(n) || ts.isFunctionExpression(n);

/** The declaration `id` names: the nearest enclosing one (parameters, block and file statements, loop heads). */
export function declOf(id: ts.Identifier): ts.Node | undefined {
  const name = id.text;
  const binds = (b: ts.BindingName): boolean => (ts.isIdentifier(b) ? b.text === name : b.elements.some((e) => !ts.isOmittedExpression(e) && binds(e.name)));
  const inList = (l: ts.VariableDeclarationList): ts.Node | undefined => l.declarations.find((d) => binds(d.name));
  for (let n: ts.Node | undefined = id.parent; n; n = n.parent) {
    if (ts.isFunctionLike(n)) {
      const p = n.parameters.find((x) => binds(x.name));
      if (p) return p;
    }
    if ((ts.isForStatement(n) || ts.isForOfStatement(n) || ts.isForInStatement(n)) && n.initializer && ts.isVariableDeclarationList(n.initializer)) {
      const d = inList(n.initializer);
      if (d) return d;
    }
    if (ts.isCatchClause(n) && n.variableDeclaration && binds(n.variableDeclaration.name)) return n.variableDeclaration;
    const sts = ts.isSourceFile(n) || ts.isBlock(n) || ts.isCaseClause(n) || ts.isDefaultClause(n) ? n.statements : undefined;
    for (const st of sts ?? []) {
      if (ts.isVariableStatement(st)) {
        const d = inList(st.declarationList);
        if (d) return d;
      }
      if (ts.isFunctionDeclaration(st) && st.name?.text === name) return st;
      const nb = ts.isImportDeclaration(st) ? st.importClause?.namedBindings : undefined;
      const spec = nb && ts.isNamedImports(nb) ? nb.elements.find((e) => e.name.text === name) : undefined;
      if (spec) return spec;
    }
  }
  return undefined;
}

const kindsOf = (id: ts.Identifier, s: Summary): Kinds | undefined => {
  const d = declOf(id);
  return d ? s.decls.get(d) : undefined;
};
export const has = (e: ts.Expression, s: Summary, k: Kind): boolean => {
  const u = unwrap(e);
  return ts.isIdentifier(u) && kindsOf(u, s)?.has(k) === true;
};
/** `shell`, `x.shell`, `useShell()`, a const bound to useShell() or a parameter typed ShellContext. */
function isShell(e: ts.Expression, s: Summary): boolean {
  const u = unwrap(e);
  if (ts.isCallExpression(u)) return calleeName(u) === 'useShell';
  if (ts.isPropertyAccessExpression(u)) return u.name.text === 'shell';
  if (!ts.isIdentifier(u)) return false;
  const d = declOf(u);
  return u.text === 'shell' || (d !== undefined && (s.decls.get(d)?.has('shell') === true || (ts.isParameter(d) && /\bShellContext\b/.test(d.type?.getText() ?? ''))));
}
/** `shell.selection`, `ship.shell.selection`: the part hangs off the shell context. */
const isShellPart = (o: ts.Expression, s: Summary, part: string): boolean => {
  const u = unwrap(o);
  return ts.isPropertyAccessExpression(u) && u.name.text === part && isShell(u.expression, s);
};
const SHELL_READS: Record<string, readonly string[]> = { selection: ['get'], sidebar: ['open', 'width'], theme: ['pref', 'resolved'], projectFilter: ['value'], computerFilter: ['value'], listOrder: ['keys'], listSelect: ['on', 'keys'], taskCards: ['render'] };
/** ShellContext's methods that hand back a readable over the registry (`shell.harnesses()()`). */
const SHELL_MAKERS = new Set(['extensions', 'settingsPanes', 'harnesses', 'promptSources', 'sideTabs', 'previewAddresses']);

/** `e` is a readable (a getter that tracks alien reads) as far as the syntax shows. */
export function isReadable(e: ts.Expression, s: Summary): boolean {
  const u = ts.isNonNullExpression(unwrap(e)) ? unwrap((unwrap(e) as ts.NonNullExpression).expression) : unwrap(e);
  if (ts.isIdentifier(u)) return has(u, s, 'readable');
  if (ts.isPropertyAccessExpression(u)) {
    const p = u.name.text;
    const o = u.expression;
    if (p === 'nav' && (nameOf(o) === 'nav' || isShell(o, s))) return true;
    if (p === 'value' && has(o, s, 'store')) return true;
    if (shapeOf(o, s)?.has(p)) return true;
    if (Object.entries(SHELL_READS).some(([part, reads]) => reads.includes(p) && isShellPart(o, s, part))) return true;
    return s.props.has(p) && !s.otherProps.has(p) && !BUILTIN.has(p);
  }
  if (!ts.isCallExpression(u)) return false;
  const c = unwrap(u.expression);
  if (ts.isIdentifier(c)) return has(c, s, 'maker') || has(c, s, 'family');
  if (!ts.isPropertyAccessExpression(c)) return false;
  const m = c.name.text;
  const o = c.expression;
  if (ENTITY.has(m) && (nameOf(o) === 'entities' || has(o, s, 'entities'))) return true;
  if (SHELL_MAKERS.has(m) && isShell(o, s)) return true;
  if (PEERS.has(m) && (nameOf(o) === 'peers' || has(o, s, 'peers'))) return true;
  if (SETTINGS.has(m) && (nameOf(o) === 'settings' || has(o, s, 'settings'))) return true;
  return m === 'get' && nameOf(o) === 'drafts';
}

/** The readable props of what `e` holds: a name whose declaration has a shape, or a call of a function that has one. */
export function shapeOf(e: ts.Expression, s: Summary): Set<string> | undefined {
  const u = unwrap(ts.isNonNullExpression(unwrap(e)) ? (unwrap(e) as ts.NonNullExpression).expression : e);
  // `props.src` where props is typed `{ src: SettingSource<…> }`: the member's declared type
  if (ts.isPropertyAccessExpression(u) && ts.isIdentifier(unwrap(u.expression))) {
    const d = declOf(unwrap(u.expression) as ts.Identifier);
    const t = d && (ts.isParameter(d) || ts.isVariableDeclaration(d)) ? d.type : undefined;
    const m = t && ts.isTypeLiteralNode(t) ? t.members.find((x) => ts.isPropertySignature(x) && x.name && propName(x.name) === u.name.text) : undefined;
    return m && ts.isPropertySignature(m) ? typeShape(m.type, s) : undefined;
  }
  const id = ts.isIdentifier(u) ? u : ts.isCallExpression(u) && ts.isIdentifier(unwrap(u.expression)) ? (unwrap(u.expression) as ts.Identifier) : undefined;
  const d = id ? declOf(id) : undefined;
  return d ? s.shapes.get(d) : undefined;
}
/** The Readable props a type names (`SettingSource<boolean>`, `Projects | undefined`). */
function typeShape(t: ts.TypeNode | undefined, s: Summary): Set<string> | undefined {
  if (!t) return undefined;
  if (ts.isUnionTypeNode(t)) {
    for (const x of t.types) {
      const got = typeShape(x, s);
      if (got) return got;
    }
    return undefined;
  }
  if (ts.isTypeLiteralNode(t)) return membersShape(t.members);
  return ts.isTypeReferenceNode(t) && ts.isIdentifier(t.typeName) ? s.types.get(t.typeName.text) : undefined;
}
export function membersShape(ms: ts.NodeArray<ts.TypeElement>): Set<string> | undefined {
  const out = new Set<string>();
  for (const m of ms) if (ts.isPropertySignature(m) && isReadableType(m.type) && propName(m.name)) out.add(propName(m.name) as string);
  return out.size > 0 ? out : undefined;
}
/** The readable props of an object literal (`{ value: computed(…), set }`). */
function literalShape(o: ts.ObjectLiteralExpression, s: Summary): Set<string> | undefined {
  const out = new Set<string>();
  for (const p of o.properties) {
    const name = p.name && propName(p.name);
    if (!name) continue;
    if (ts.isPropertyAssignment(p) && isReadable(p.initializer, s)) out.add(name);
    if (ts.isShorthandPropertyAssignment(p) && isReadable(p.name, s)) out.add(name);
  }
  return out.size > 0 ? out : undefined;
}
/** What a declaration's value looks like: its declared type, or what it's made of. */
export function shapeOfDecl(node: ts.Node, init: ts.Node | undefined, s: Summary): Set<string> | undefined {
  if (ts.isParameter(node) || ts.isVariableDeclaration(node)) {
    const t = typeShape(node.type, s);
    if (t || !init) return t;
    if (ts.isObjectLiteralExpression(init)) return literalShape(init, s);
    return ts.isExpression(init) && !isFn(init) ? shapeOf(init, s) : init && isFn(init) ? fnShape(init, s) : undefined;
  }
  return ts.isFunctionDeclaration(node) ? fnShape(node, s) : undefined;
}
/** A function's result: its declared return type, or the object literals it returns. */
function fnShape(fn: ts.FunctionLikeDeclaration, s: Summary): Set<string> | undefined {
  const t = typeShape(fn.type, s);
  if (t || !fn.body) return t;
  const body = fn.body;
  if (!ts.isBlock(body)) {
    const e = unwrap(body as ts.Expression);
    return ts.isObjectLiteralExpression(e) ? literalShape(e, s) : undefined;
  }
  let out: Set<string> | undefined;
  const walk = (x: ts.Node): void => {
    if (ts.isFunctionLike(x)) return;
    if (ts.isReturnStatement(x) && x.expression && ts.isObjectLiteralExpression(unwrap(x.expression))) out ??= literalShape(unwrap(x.expression) as ts.ObjectLiteralExpression, s);
    ts.forEachChild(x, walk);
  };
  ts.forEachChild(body, walk);
  return out;
}

/** A read: a readable called with no arguments, or a reader called. */
export function isRead(n: ts.CallExpression, s: Summary): boolean {
  if (n.arguments.length === 0 && isReadable(n.expression, s)) return true;
  return has(n.expression, s, 'reader');
}
