// A UI test waits on what it then asserts, not on a fixed count of timer hops. A loop that only awaits a real timer
// (`for (let i = 0; i < 30; i++) await new Promise((r) => setTimeout(r, 0))`, or a `tick()`/`settle()` helper that
// is one) costs its count times the timer's grain: on Windows a timer hop is a ~15.6 ms system tick, so the 30-hop
// flushes and the `i < 300 && !ok()` caps of 10 ms waits ran 0.5 s and ~4.7 s there and timed out under the one-run
// load (round 322). So in the `test/` folder of a workspace package, a `for`/`while`/`do` loop whose body does nothing
// but await a timer hop is an error unless its condition reads the clock (`performance.now()`/`Date.now()`: a deadline,
// as `until(ok)` keeps one) or is a plain condition with no count in it (`while (!ok())`). Wait with `until(ok)`, or
// put the code under test on fake timers (an injected `Timers`, vi.useFakeTimers) and move them. A loop with a real
// reason says so on its line or the line above: `// check:allow timer-loop-in-test <reason>`.
//
// A timer hop is `new Promise(…setTimeout…)`, or a call to a function with one and no loop of its own, declared in the
// file or imported from a relative module. setImmediate and microtask loops (`await Promise.resolve()`) are not timer
// hops: their grain is not the system tick. A loop that steps something each turn (advances a fake clock, writes a
// frame) and then awaits is stepping, not waiting, and passes.

import ts from 'typescript';
import { finding } from '../core/report.ts';
import { commentsOf, lineOf, visit } from '../core/source.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';

const ID = 'timer-loop-in-test';
const ALLOW = new RegExp(`check:allow\\s+${ID}\\s+\\S`);
const RELATIONAL = new Set([ts.SyntaxKind.LessThanToken, ts.SyntaxKind.LessThanEqualsToken, ts.SyntaxKind.GreaterThanToken, ts.SyntaxKind.GreaterThanEqualsToken]);

type Loop = ts.ForStatement | ts.WhileStatement | ts.DoStatement | ts.ForOfStatement;
const isLoop = (n: ts.Node): n is Loop => ts.isForStatement(n) || ts.isWhileStatement(n) || ts.isDoStatement(n) || ts.isForOfStatement(n);

/** `setTimeout(…)` or `globalThis.setTimeout(…)` / `window.setTimeout(…)` (not vi's). */
function isSetTimeout(n: ts.Node): boolean {
  if (!ts.isCallExpression(n)) return false;
  const c = n.expression;
  if (ts.isIdentifier(c)) return c.text === 'setTimeout';
  return ts.isPropertyAccessExpression(c) && c.name.text === 'setTimeout' && ts.isIdentifier(c.expression) && (c.expression.text === 'globalThis' || c.expression.text === 'window');
}

/** `new Promise(…)` with a setTimeout inside its executor. */
function isTimerPromise(n: ts.Node): boolean {
  if (!ts.isNewExpression(n) || !ts.isIdentifier(n.expression) || n.expression.text !== 'Promise') return false;
  let found = false;
  for (const a of n.arguments ?? []) visit(a, (x) => void (found ||= isSetTimeout(x)));
  return found;
}

/** A function body that is one timer hop: a timer promise in it, and no loop (a loop in it is a waiter or a flush,
 * judged where it is written). */
function isHopBody(body: ts.Node): boolean {
  let timer = false;
  let loop = false;
  visit(body, (x) => {
    timer ||= isTimerPromise(x);
    loop ||= isLoop(x);
  });
  return timer && !loop;
}

/** The names a file declares at its top level (functions, consts bound to a function) that are one timer hop. */
function hopsDeclared(sf: ts.SourceFile): Set<string> {
  const out = new Set<string>();
  for (const st of sf.statements) {
    if (ts.isFunctionDeclaration(st) && st.name && st.body && isHopBody(st.body)) out.add(st.name.text);
    if (ts.isVariableStatement(st))
      for (const d of st.declarationList.declarations) {
        const init = d.initializer;
        if (ts.isIdentifier(d.name) && init && (ts.isArrowFunction(init) || ts.isFunctionExpression(init)) && isHopBody(init.body)) out.add(d.name.text);
      }
  }
  return out;
}

/** Timer-hop helpers this file can call by name: its own, those it imports by name from a relative module, and
 * node's promise `setTimeout` (`import { setTimeout as sleep } from 'node:timers/promises'`). */
function hopNames(ctx: Ctx, file: string, sf: ts.SourceFile, memo: Map<string, Set<string>>): Set<string> {
  const own = new Set(hopsDeclared(sf));
  for (const st of sf.statements) {
    if (!ts.isImportDeclaration(st) || !ts.isStringLiteral(st.moduleSpecifier) || st.importClause?.isTypeOnly) continue;
    const nb = st.importClause?.namedBindings;
    if (!nb || !ts.isNamedImports(nb)) continue;
    if (st.moduleSpecifier.text === 'node:timers/promises' || st.moduleSpecifier.text === 'timers/promises') {
      for (const el of nb.elements) if ((el.propertyName ?? el.name).text === 'setTimeout') own.add(el.name.text);
      continue;
    }
    const r = ctx.resolve(file, st.moduleSpecifier.text);
    if (r.kind !== 'relative' || !r.file) continue;
    const target = r.file;
    let theirs = memo.get(target);
    if (!theirs) {
      theirs = hopsDeclared(ctx.source(target));
      memo.set(target, theirs);
    }
    for (const el of nb.elements) if (theirs.has((el.propertyName ?? el.name).text)) own.add(el.name.text);
  }
  return own;
}

/** `await <timer hop>`: a timer promise, or a call to a hop helper. */
function isAwaitedHop(e: ts.Expression, hops: ReadonlySet<string>): boolean {
  let cur = e;
  while (ts.isParenthesizedExpression(cur)) cur = cur.expression;
  if (!ts.isAwaitExpression(cur)) return false;
  const x = cur.expression;
  if (isTimerPromise(x)) return true;
  return ts.isCallExpression(x) && ts.isIdentifier(x.expression) && hops.has(x.expression.text);
}

/** `if (…) break;` (or `{ break; }`): a guard that ends the wait early, still a wait. */
function isGuard(s: ts.Statement): boolean {
  if (!ts.isIfStatement(s) || s.elseStatement) return false;
  const t = s.thenStatement;
  return ts.isBreakStatement(t) || (ts.isBlock(t) && t.statements.length === 1 && t.statements[0] !== undefined && ts.isBreakStatement(t.statements[0]));
}

/** The loop's body does nothing but await timer hops (one or several), with at most guards that break out. */
function onlyWaits(body: ts.Statement, hops: ReadonlySet<string>): boolean {
  const sts = ts.isBlock(body) ? [...body.statements] : [body];
  const waits = sts.filter((s) => ts.isExpressionStatement(s) && isAwaitedHop(s.expression, hops));
  return waits.length > 0 && sts.every((s) => waits.includes(s) || isGuard(s));
}

/** The condition keeps a deadline by the clock (`performance.now()`, `Date.now()`). */
function readsClock(cond: ts.Node): boolean {
  let found = false;
  visit(cond, (x) => {
    if (!ts.isCallExpression(x) || !ts.isPropertyAccessExpression(x.expression) || x.expression.name.text !== 'now') return;
    const o = x.expression.expression;
    found ||= ts.isIdentifier(o) && (o.text === 'performance' || o.text === 'Date');
  });
  return found;
}

const NOT_EQUAL = new Set([ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsToken]);
const STEP = new Set([ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken]);

/** A count in the condition: a comparison (`i < 30`, `n-- > 0`), a step (`while (n--)`), or `i !== 30`. */
function counts(cond: ts.Node | undefined): boolean {
  if (!cond) return false;
  let found = false;
  visit(cond, (x) => {
    if (ts.isBinaryExpression(x)) found ||= RELATIONAL.has(x.operatorToken.kind) || (NOT_EQUAL.has(x.operatorToken.kind) && (ts.isNumericLiteral(x.left) || ts.isNumericLiteral(x.right)));
    if (ts.isPrefixUnaryExpression(x) || ts.isPostfixUnaryExpression(x)) found ||= STEP.has(x.operator);
  });
  return found;
}

/** `for (… of Array(30))`, `new Array(30)`, `Array.from({ length: 30 })`: a count spelled as a list. */
function countedList(e: ts.Expression): boolean {
  if ((ts.isCallExpression(e) || ts.isNewExpression(e)) && ts.isIdentifier(e.expression) && e.expression.text === 'Array') return true;
  if (!ts.isCallExpression(e) || !ts.isPropertyAccessExpression(e.expression) || e.expression.name.text !== 'from') return false;
  const o = e.expression.expression;
  const arg = e.arguments[0];
  return ts.isIdentifier(o) && o.text === 'Array' && arg !== undefined && ts.isObjectLiteralExpression(arg) && arg.properties.some((p) => p.name !== undefined && ts.isIdentifier(p.name) && p.name.text === 'length');
}

/** The loop counts its turns and keeps no deadline by the clock. */
function counted(n: Loop): boolean {
  if (ts.isForOfStatement(n)) return countedList(n.expression);
  const cond = ts.isForStatement(n) ? n.condition : n.expression;
  return !(cond && readsClock(cond)) && counts(cond);
}

export const timerLoopInTest: Rule = {
  id: ID,
  description: 'a UI test waits on its condition (until, a clock deadline) or fake timers, not on a fixed count of timer hops (~15.6 ms each on Windows)',
  run(ctx) {
    const out: Finding[] = [];
    const memo = new Map<string, Set<string>>();
    for (const file of ctx.files) {
      if (!/(^|\/)test\/(.+\/)?[^/]+\.tsx?$/.test(file) || file.endsWith('.d.ts') || !ctx.pkgOf(file)) continue;
      const text = ctx.read(file);
      if (!text.includes('setTimeout') && !/\bimport\b[^;]*from\s+['"]\.\.?\//.test(text)) continue;
      const sf = ctx.source(file);
      let hops: Set<string> | undefined;
      let allowed: Set<number> | undefined;
      visit(sf, (n) => {
        if (!isLoop(n)) return;
        hops ??= hopNames(ctx, file, sf, memo);
        if (!onlyWaits(n.statement, hops)) return;
        if (!counted(n)) return;
        allowed ??= new Set(commentsOf(sf).filter((c) => ALLOW.test(c.text)).map((c) => lineOf(sf, c.pos)));
        const line = lineOf(sf, n);
        if (allowed.has(line) || allowed.has(line - 1)) return;
        out.push(finding(file, line, ID, `a loop that only awaits timer hops waits a fixed count, not on what the test asserts (each hop is a ~15.6 ms system tick on Windows): wait with \`until(ok)\` (a clock deadline) or put the code on fake timers (a real reason: \`// check:allow ${ID} <reason>\`)`));
      });
    }
    return out;
  },
};
