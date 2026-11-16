// A UI test loads its modules at the top of the file, not inside a test. vitest's 5 s timeout starts when the test
// (or a beforeEach/beforeAll hook) starts, so an `import(...)` in its body compiles the module on the clock; under CI
// load that has failed runs several times (pane-host, plugins/machine/ui labels). So in a `test/*.test.ts(x)` of a
// workspace package, an `import(...)` inside the callback of test/it (and .only/.skip/.each/…) or of a
// before*/after* hook is an error: import it at the top. A top-level `await import(...)` passes (it loads during
// collection, off the clock), so does one inside a `vi.mock`/`vi.doMock` factory. A test that needs a fresh module
// (vi.resetModules) says so on the import's line or the line above: `// check:allow dynamic-import-in-test <reason>`.
// `vi.importActual(…)` / `vi.importMock(…)` in a test body load on the same clock and count too. Only callbacks
// written inline are read: `test('x', run)` with `run` declared elsewhere is not followed.

import ts from 'typescript';
import { finding } from '../core/report.ts';
import { commentsOf, lineOf, visit } from '../core/source.ts';
import type { Finding, Rule } from '../core/types.ts';

const ID = 'dynamic-import-in-test';
const TIMED = new Set(['test', 'it', 'beforeEach', 'beforeAll', 'afterEach', 'afterAll']);
const MOCKS = new Set(['mock', 'doMock']);
const VI_IMPORTS = new Set(['importActual', 'importMock']);
const ALLOW = new RegExp(`check:allow\\s+${ID}\\s+\\S`);

/** The identifier a call chain starts from: `test`, `it.only`, `test.each(rows)`, `vi.mock` → test, it, test, vi. */
function rootName(e: ts.Expression): string | undefined {
  let cur = e;
  while (ts.isPropertyAccessExpression(cur) || ts.isCallExpression(cur)) cur = cur.expression;
  return ts.isIdentifier(cur) ? cur.text : undefined;
}

/** What the nearest enclosing call that takes `fn` as an argument is: a timed test/hook, a vi.mock factory, or neither. */
function ownerOf(fn: ts.Node): 'timed' | 'mock' | undefined {
  const call = fn.parent;
  if (!call || !ts.isCallExpression(call) || !call.arguments.some((a) => a === fn)) return undefined;
  const callee = call.expression;
  if (ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression) && callee.expression.text === 'vi' && MOCKS.has(callee.name.text)) return 'mock';
  const name = rootName(callee);
  return name && TIMED.has(name) ? 'timed' : undefined;
}

/** `import(…)`, `vi.importActual(…)`, `vi.importMock(…)`: what it calls, or undefined. */
function loader(n: ts.CallExpression): string | undefined {
  if (n.expression.kind === ts.SyntaxKind.ImportKeyword) return 'import';
  const c = n.expression;
  return ts.isPropertyAccessExpression(c) && ts.isIdentifier(c.expression) && c.expression.text === 'vi' && VI_IMPORTS.has(c.name.text) ? `vi.${c.name.text}` : undefined;
}

/** Inside a test or hook body (and not inside a vi.mock factory nearer to it). */
function inTimedBody(n: ts.Node): boolean {
  for (let cur = n.parent; cur; cur = cur.parent) {
    if (ts.isArrowFunction(cur) || ts.isFunctionExpression(cur)) {
      const owner = ownerOf(cur);
      if (owner === 'mock') return false;
      if (owner === 'timed') return true;
    }
  }
  return false;
}

export const dynamicImportInTest: Rule = {
  id: ID,
  description: 'a UI test imports its modules at the top, not with import() inside a test or hook (it counts against the 5 s timeout)',
  run(ctx) {
    const out: Finding[] = [];
    for (const file of ctx.files) {
      if (!/(^|\/)test\/(.+\/)?[^/]+\.test\.tsx?$/.test(file) || !ctx.pkgOf(file)) continue;
      const sf = ctx.source(file);
      let allowed: Set<number> | undefined;
      visit(sf, (n) => {
        if (!ts.isCallExpression(n)) return;
        const how = loader(n);
        if (!how || !inTimedBody(n)) return;
        allowed ??= new Set(commentsOf(sf).filter((c) => ALLOW.test(c.text)).map((c) => lineOf(sf, c.pos)));
        const line = lineOf(sf, n);
        if (allowed.has(line) || allowed.has(line - 1)) return;
        const arg = n.arguments[0];
        const spec = arg && ts.isStringLiteralLike(arg) ? arg.text : arg?.getText(sf) ?? '';
        out.push(finding(file, line, ID, `${how}(${JSON.stringify(spec)}) inside a test body loads the module on the test's 5 s clock: import it at the top of the file (a test that needs a fresh module after vi.resetModules: \`// check:allow ${ID} <reason>\`)`));
      });
    }
    return out;
  },
};
