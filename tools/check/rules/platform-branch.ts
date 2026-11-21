// ADR-0065 §5: features don't know the platform. The phone apps' look comes from the kit's variants
// (`:root[data-platform=android|ios]` in @layer kit, `usePlatform()` for structure), so the only code that
// reads the platform's look is in packages/ui, the token sets in packages/tokens and the shell's platform
// module (which sets the attribute). Anywhere else, the attribute `data-platform` (a string, a template, a
// stylesheet, `dataset.platform`) or a comparison of `platform.kind` with 'android' / 'ios' (===, !==, ==,
// !=, either side, or a `case` in a switch on it) is an error. `platform.kind === 'desktop'` and a
// machine's own OS are not the phone's look and pass. The kit's own readers (`usePlatform`, `lookOf`,
// `documentLook`, `LOOK_ATTR`, `PlatformLookProvider`) can't be imported from @wisp/ui outside it either.
// Tests and tools/ are not features.

import ts from 'typescript';
import { finding } from '../core/report.ts';
import { isCode, isTest, lineOf, visit } from '../core/source.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';

const ID = 'platform-branch';
const ALLOWED = ['packages/ui/', 'packages/tokens/', 'packages/shell/src/platform/'];
const PHONES = new Set(['android', 'ios']);
const ATTR = /data-platform/;
const KIT_READERS = new Set(['usePlatform', 'lookOf', 'documentLook', 'LOOK_ATTR', 'PlatformLookProvider']);
const WHY = "features don't know the platform (ADR-0065 §5): use a kit component (its variant draws Material 3 or HIG), or the kit's usePlatform() inside packages/ui";

function inScope(ctx: Ctx, file: string): boolean {
  return ctx.pkgOf(file) !== undefined && !isTest(file) && !ALLOWED.some((p) => file.startsWith(p));
}

/** `x.platform.kind`, `platform.kind`, `p.platform?.kind`: a `.kind` read off something named platform; or a
 * bare `kind` (destructured from a platform). */
function isPlatformKind(e: ts.Expression): boolean {
  let n = e;
  while (ts.isParenthesizedExpression(n) || ts.isNonNullExpression(n)) n = n.expression;
  if (ts.isIdentifier(n)) return n.text === 'kind';
  if (!ts.isPropertyAccessExpression(n) || n.name.text !== 'kind') return false;
  const recv = n.expression;
  const name = ts.isPropertyAccessExpression(recv) ? recv.name.text : ts.isIdentifier(recv) ? recv.text : '';
  return /platform/i.test(name);
}

const phoneLiteral = (e: ts.Expression): boolean => ts.isStringLiteralLike(e) && PHONES.has(e.text);

const EQUALITY = new Set([
  ts.SyntaxKind.EqualsEqualsEqualsToken,
  ts.SyntaxKind.ExclamationEqualsEqualsToken,
  ts.SyntaxKind.EqualsEqualsToken,
  ts.SyntaxKind.ExclamationEqualsToken,
]);

function codeFindings(ctx: Ctx, file: string, out: Finding[]): void {
  const sf = ctx.source(file);
  const add = (n: ts.Node, what: string): void => void out.push(finding(file, lineOf(sf, n), ID, `${what}: ${WHY}`));
  visit(sf, (n) => {
    if ((ts.isStringLiteralLike(n) || ts.isTemplateHead(n) || ts.isTemplateMiddle(n) || ts.isTemplateTail(n)) && ATTR.test(n.text)) {
      add(n, 'reads data-platform');
    } else if (ts.isPropertyAccessExpression(n) && n.name.text === 'platform' && ts.isPropertyAccessExpression(n.expression) && n.expression.name.text === 'dataset') {
      add(n, 'reads dataset.platform');
    } else if (ts.isBinaryExpression(n) && EQUALITY.has(n.operatorToken.kind)) {
      if ((isPlatformKind(n.left) && phoneLiteral(n.right)) || (isPlatformKind(n.right) && phoneLiteral(n.left))) add(n, 'compares platform.kind with a phone');
    } else if (ts.isJsxAttribute(n) && ATTR.test(n.name.getText(sf))) {
      add(n, 'sets data-platform');
    } else if (ts.isImportDeclaration(n) && ts.isStringLiteral(n.moduleSpecifier) && /^@wisp\/ui(\/|$)/.test(n.moduleSpecifier.text)) {
      const b = n.importClause?.namedBindings;
      if (b && ts.isNamedImports(b)) for (const el of b.elements) if (KIT_READERS.has((el.propertyName ?? el.name).text)) add(el, `imports the kit's ${(el.propertyName ?? el.name).text}`);
    } else if (ts.isCallExpression(n) && ts.isPropertyAccessExpression(n.expression) && n.expression.name.text === 'includes' && n.arguments[0] && isPlatformKind(n.arguments[0]) && ts.isArrayLiteralExpression(n.expression.expression) && n.expression.expression.elements.some((x) => phoneLiteral(x))) {
      add(n, 'checks platform.kind against the phones');
    } else if (ts.isSwitchStatement(n) && isPlatformKind(n.expression)) {
      for (const c of n.caseBlock.clauses) if (ts.isCaseClause(c) && phoneLiteral(c.expression)) add(c, 'switches on platform.kind for a phone');
    }
  });
}

function textFindings(ctx: Ctx, file: string, out: Finding[]): void {
  const lines = ctx.read(file).split('\n');
  lines.forEach((l, i) => {
    if (ATTR.test(l)) out.push(finding(file, i + 1, ID, `a stylesheet picks a platform: ${WHY}`));
  });
}

export const platformBranch: Rule = {
  id: ID,
  description: 'only packages/ui, packages/tokens and packages/shell/src/platform read data-platform or compare platform.kind with android/ios',
  run(ctx) {
    const out: Finding[] = [];
    for (const file of ctx.files) {
      if (!inScope(ctx, file)) continue;
      if (isCode(file)) codeFindings(ctx, file, out);
      else if (/\.(css|html)$/.test(file)) textFindings(ctx, file, out);
    }
    return out;
  },
};
