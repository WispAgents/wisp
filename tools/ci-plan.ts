// Which checks of a package tools/ci-verify.ts runs which way (ADR-0072): `plan` reads a package's scripts as
// monoflow does (2a86d14 graph/discover_node.go), `covers` says whether `npm run typecheck` (tools/typecheck.ts
// PROGRAMS) already type-checks a package's own `tsc -p <config>`: same compiler options, every one of its files.

import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';

const ROOT = join(import.meta.dirname, '..');
const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** monoflow's lint script names (2a86d14 discover_node.go lintScripts): a package with one keeps the whole monoflow verify. */
const LINT_SCRIPTS = ['lint', 'test:lint', 'test:eslint', 'test:oxlint', 'eslint'];
/** The root files that give every npm package a lint command in monoflow (discover_node.go: eslint config). */
export const ROOT_LINT = /^(eslint\.config\.[cm]?[jt]s|\.eslintrc(\.[a-z]+)?)$/;

/** How one package is checked: all through monoflow, or its typecheck and tests each the cheapest way. */
export type How =
  | { kind: 'monoflow' }
  | { kind: 'split'; typecheck: 'covered' | 'monoflow'; test: 'vitest' | 'monoflow'; needsTests: boolean; configs: string[] };

/**
 * How a package is checked, from its package.json scripts (undefined: not an npm package). monoflow runs the
 * `typecheck` script, the lint script and the `test` script (2a86d14, first of each list); a package splits only when
 * it has `typecheck` and `test` and no lint, so nothing monoflow would run is left out. `covered` says whether
 * `npm run typecheck` already checks a config (`tsc -p <config>`) with the same files and options.
 */
export function plan(scripts: Readonly<Record<string, string>> | undefined, o: { vitestConfig: boolean; rootLint: boolean; covered: (config: string) => boolean }): How {
  const tc = scripts?.['typecheck'];
  const test = scripts?.['test'];
  if (!scripts || tc === undefined || test === undefined || o.rootLint || LINT_SCRIPTS.some((s) => s in scripts)) return { kind: 'monoflow' };
  const configs = tc.split('&&').map((c) => /^tsc (?:-p|--project) (\S+)$/.exec(c.trim())?.[1]);
  const covered = configs.every((c) => c !== undefined && o.covered(c));
  const vitest = o.vitestConfig && /^vitest run(?: --passWithNoTests)?$/.test(test.trim());
  if (!covered && !vitest) return { kind: 'monoflow' };
  return { kind: 'split', typecheck: covered ? 'covered' : 'monoflow', test: vitest ? 'vitest' : 'monoflow', needsTests: !test.includes('--passWithNoTests'), configs: configs.filter((c): c is string => c !== undefined) };
}

/** A tsconfig as tsc reads it: its root files (absolute) and its compiler options (as one comparable string). */
export interface Program {
  files: readonly string[];
  options: string;
}

/** True when one program holds every file of `pkg` with exactly its options: `tsc -p` of the package checks nothing more. */
export function covers(pkg: Program | undefined, programs: readonly (Program | undefined)[]): boolean {
  if (!pkg || pkg.files.length === 0) return false; // no inputs is tsc's own error: let monoflow run it and fail
  return programs.some((p) => {
    if (!p || p.options !== pkg.options) return false;
    const all = new Set(p.files);
    return pkg.files.every((f) => all.has(f));
  });
}

/** JSON with sorted keys, so two option objects compare by value. */
const stable = (v: unknown): string =>
  JSON.stringify(v, (_k, x: unknown) => (isObj(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => a.localeCompare(b))) : x));

let tsApi: typeof import('typescript') | undefined;
/** Reads a tsconfig with TypeScript's own parser (extends, include, exclude), without type-checking anything. */
export function readProgram(config: string): Program | undefined {
  tsApi ??= createRequire(import.meta.url)('typescript') as typeof import('typescript');
  const ts = tsApi;
  const path = resolve(ROOT, config);
  if (!existsSync(path)) return undefined;
  const read = ts.readConfigFile(path, (f) => ts.sys.readFile(f));
  if (read.error) return undefined;
  const parsed = ts.parseJsonConfigFileContent(read.config, ts.sys, dirname(path), undefined, path);
  if (parsed.errors.length) return undefined;
  const options: Record<string, unknown> = { ...parsed.options };
  delete options['configFilePath'];
  return { files: parsed.fileNames.map((f) => resolve(f)), options: stable(options) };
}
