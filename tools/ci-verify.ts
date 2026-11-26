// CI's pass over every package but the Cargo workspace's members (suite.yml, ADR-0069, ADR-0072). The workspace's
// crates are checked once by the job's own cargo steps (`cargo clippy --workspace --all-targets`, `cargo test
// --workspace --no-fail-fast`); `verify --all` re-ran them crate by crate. What's left (the npm workspaces,
// sdk/python, the crates outside the workspace such as spikes/*) gets every check monoflow's `verify --no-cache`
// runs for it, each done once (ADR-0072):
// - typecheck: a package's `tsc -p <config>` whose files and compiler options one of `npm run typecheck`'s programs
//   already holds (tools/typecheck.ts PROGRAMS: the job's typecheck step, its output in `--typecheck-log`) is not
//   run again; that step's errors are credited to the package whose config holds the file. Without a log of a
//   finished typecheck (the step didn't run), ci-verify runs tools/typecheck.ts itself. Any other typecheck runs
//   through monoflow (`--typecheck`).
// - tests: every package whose `test` script is plain `vitest run` runs in one vitest process
//   (tools/vitest.ci.config.ts: each package's own vitest.config.ts is a project). One vitest start per package, and
//   one tsc per package over the whole kit, were most of the pass: 26+ min on Windows. Any other test script runs
//   through monoflow (`--test`).
// - a package with a lint script (or a root eslint config), sdk/python and the spikes: monoflow, every check, as
//   before. The spikes start first, beside the vitest run: their builds are the longest.
// Failures of the cargo test step (its log, `--cargo-log`) join the result as failed crates, so the job summary and
// gate G14 (tools/g14.ts) see every failing package as they did.
//
//   node tools/ci-verify.ts --out=verify-all-1.json [--cargo-log=<file>] [--typecheck-log=<file>] [--jobs=N] [--no-cache] [dir…]
//   node tools/ci-verify.ts --out=- --list        the packages it would verify, how, and the folders it skips
//
// The output has monoflow's `--json` shape (status, counts, `failed`, `errors`, `results` with each target's time and
// what ran it), read by tools/monoflow-report.ts. Exit 1 when a package failed (the cargo step fails the job itself).

import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { availableParallelism, tmpdir } from 'node:os';
import { dirname, join, posix, relative, resolve, sep } from 'node:path';
import { cargoFailures, slash, stripAnsi, tscFailures, vitestByPackage } from './ci-logs.ts';
import { covers, plan, readProgram, ROOT_LINT, type How, type Program } from './ci-plan.ts';
import { stopStrayRuns } from './ci-strays.ts';
import { DEST } from './setup-monoflow.ts';
import { PROGRAMS } from './typecheck.ts';

const ROOT = join(import.meta.dirname, '..');

/** The manifests monoflow discovers packages from (2a86d14: graph/discover_*.go) and the folders it never looks in. */
const MANIFESTS = new Set(['package.json', 'Cargo.toml', 'pyproject.toml', 'setup.py', 'go.mod']);
const SKIPPED = new Set(['node_modules', 'vendor', 'testdata', '__fixtures__', 'fixtures', '.agents', '.git', 'target', 'dist', 'build', '.venv', 'venv', '__pycache__', 'third_party']);

export interface Target {
  target: string;
  path: string;
  status: 'passed' | 'failed';
  duration_ms: number;
  /** what ran it: `monoflow`, `vitest (one run)`, `npm run typecheck`, joined by ` + ` */
  via: string;
}

export interface Part {
  target: string;
  path: string;
  raw: unknown;
  ms: number;
  via?: string;
}

/** One check's result for a package (a monoflow call, its share of the vitest run or of `npm run typecheck`). */
export interface Piece {
  via: string;
  ok: boolean;
  ms: number;
  errors: unknown[];
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Folders holding a manifest, as monoflow lists files (git, tracked and untracked-not-ignored, skipped folders out). */
export function manifestDirs(files: readonly string[]): string[] {
  const out = new Set<string>();
  for (const f of files) {
    const parts = f.split('/');
    const base = parts.at(-1) ?? '';
    if (!MANIFESTS.has(base) || parts.slice(0, -1).some((p) => SKIPPED.has(p))) continue;
    out.add(parts.length === 1 ? '.' : parts.slice(0, -1).join('/'));
  }
  return [...out].sort();
}

/** The folders to ask monoflow about: every manifest folder except a Cargo workspace member's with no other manifest. */
export function candidates(files: readonly string[], members: ReadonlySet<string>): string[] {
  const others = new Set(files.filter((f) => MANIFESTS.has(posix.basename(f)) && posix.basename(f) !== 'Cargo.toml').map((f) => posix.dirname(f)));
  return manifestDirs(files).filter((d) => d !== '.' && (!members.has(d) || others.has(d)));
}

/** A monoflow call's `--json` result as one piece. */
export function monoflowPiece(raw: unknown, ms: number, via = 'monoflow'): Piece {
  const j = isObj(raw) ? raw : {};
  const ok = j['status'] === 'passed';
  const errors = Array.isArray(j['errors']) ? [...j['errors']] : [];
  if (!ok && errors.length === 0) errors.push({ code: 'execution_failure', message: typeof j['error'] === 'string' ? j['error'] : `${via}: ${String(j['status'] ?? 'no result')}`, targets: [] });
  return { via, ok, ms: num(j['duration_ms']) || ms, errors };
}

/** A package's pieces as one monoflow-shaped result (errors keyed to the package). */
export function combine(target: string, path: string, pieces: readonly Piece[]): Part {
  const ok = pieces.every((p) => p.ok);
  const errors = pieces.flatMap((p) => p.errors.map((e) => (isObj(e) ? { ...e, targets: [target] } : e)));
  const ms = pieces.reduce((a, p) => a + p.ms, 0);
  return { target, path, ms, via: pieces.map((p) => p.via).join(' + '), raw: { status: ok ? 'passed' : 'failed', duration_ms: ms, ...(ok ? {} : { failed: { [target]: 1 } }), errors } };
}

/** Folds the per-package results and the cargo step into one `monoflow verify --json`-shaped result. */
export function merge(parts: readonly Part[], cargo: { ran: boolean; failures: Map<string, string[]>; crates: number; dirs?: ReadonlyMap<string, string> }, wallMs: number): Record<string, unknown> {
  const results: Target[] = [];
  const errors: unknown[] = [];
  const failed: Record<string, number> = {};
  let cached = 0;
  for (const p of parts) {
    const j = isObj(p.raw) ? p.raw : {};
    const ok = j['status'] === 'passed';
    cached += num(j['cached_targets']);
    if (Array.isArray(j['errors'])) errors.push(...j['errors']);
    if (isObj(j['failed'])) for (const [k, v] of Object.entries(j['failed'])) failed[k] = (failed[k] ?? 0) + (num(v) || 1);
    if (!ok && !failed[p.target]) {
      failed[p.target] = 1;
      if (!Array.isArray(j['errors']) || j['errors'].length === 0) errors.push({ code: 'execution_failure', message: typeof j['error'] === 'string' ? j['error'] : `monoflow verify --package=${p.target}: ${String(j['status'] ?? 'no result')}`, targets: [p.target] });
    }
    results.push({ target: p.target, path: p.path, status: ok ? 'passed' : 'failed', duration_ms: num(j['duration_ms']) || p.ms, via: p.via ?? 'monoflow' });
  }
  // a crate named like a monoflow package (crates/wisp and sdk/python are both `wisp`) is keyed by its folder, so its
  // failure is never credited to the other package (G14); no `results` row: the cargo step's time is the step's
  const taken = new Set(parts.map((p) => p.target));
  for (const [crate, what] of cargo.failures) {
    const key = taken.has(crate) ? (cargo.dirs?.get(crate) ?? `cargo:${crate}`) : crate;
    failed[key] = (failed[key] ?? 0) + 1;
    errors.push({ code: 'test-failure', message: `cargo test --workspace (${crate}): ${what.join('; ')} (see the cargo test step's log)`, targets: [key] });
  }
  const failedTargets = Object.keys(failed).length;
  // crates with no cargo test result count as neither passed nor failed
  const passed = parts.length - (failedTargets - cargo.failures.size) + (cargo.ran ? cargo.crates - cargo.failures.size : 0);
  return {
    status: failedTargets ? 'failed' : 'passed',
    duration_ms: wallMs,
    total_targets: parts.length + cargo.crates,
    passed_targets: passed,
    failed_targets: failedTargets,
    cached_targets: cached,
    failed,
    errors,
    results: results.sort((a, b) => b.duration_ms - a.duration_ms),
    rust_workspace: cargo.ran
      ? `${cargo.crates} crates checked by the cargo steps (clippy --workspace --all-targets, test --workspace --no-fail-fast), not again here`
      : `${cargo.crates} crates left to the cargo steps; cargo test didn't run (an earlier step failed), so their tests have no result here`,
  };
}

function git(args: string[]): string {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20 });
}

/** The Cargo workspace's members: their folders and names. */
function workspace(): { dirs: Set<string>; names: Set<string>; byName: Map<string, string> } {
  const meta: unknown = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--locked', '--format-version', '1'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20 }));
  const pkgs = isObj(meta) && Array.isArray(meta['packages']) ? meta['packages'] : [];
  const dirs = new Set<string>();
  const names = new Set<string>();
  const byName = new Map<string, string>();
  for (const p of pkgs) {
    if (!isObj(p) || typeof p['name'] !== 'string' || typeof p['manifest_path'] !== 'string') continue;
    const dir = relative(ROOT, dirname(p['manifest_path'])).split(sep).join('/');
    dirs.add(dir);
    names.add(p['name']);
    byName.set(p['name'], dir);
  }
  return { dirs, names, byName };
}

/** What the .agents/bin/monoflow wrapper sets (tools/setup-monoflow.ts WRAPPER_SCRIPT; a test keeps them alike). */
export const WRAPPER_ENV: Readonly<Record<string, string>> = { MONOFLOW_NO_SPECULATE: '1', MONOFLOW_NO_ROOT_SPECULATE: '1' };

/** One child process, stdout captured (or passed through), stderr passed through. */
function run(cmd: string, args: string[], env: NodeJS.ProcessEnv, capture = true): Promise<{ code: number; out: string }> {
  return new Promise((done) => {
    const child = spawn(cmd, args, { cwd: ROOT, env, stdio: ['ignore', capture ? 'pipe' : 'inherit', 'inherit'] });
    let out = '';
    child.stdout?.setEncoding('utf8').on('data', (d: string) => (out += d));
    child.on('error', (e) => done({ code: -1, out: JSON.stringify({ status: 'failed', error: e.message }) }));
    child.on('close', (code) => done({ code: code ?? -1, out }));
  });
}

/** One monoflow call, as the wrapper makes it (no background speculation). */
function monoflow(args: string[], json = true): Promise<{ code: number; out: string }> {
  return run(join(ROOT, DEST), args, { ...process.env, ...WRAPPER_ENV, ...(json ? { MONOFLOW_FORMAT: 'json' } : {}) });
}

const parse = (s: string): unknown => {
  const at = s.indexOf('{');
  try {
    return JSON.parse(at > 0 ? s.slice(at) : s);
  } catch {
    return { status: 'failed', error: `monoflow printed no JSON: ${s.trim().slice(-400)}` };
  }
};

async function pool<T, R>(items: readonly T[], jobs: number, work: (t: T) => Promise<R>): Promise<R[]> {
  const out: R[] = new Array<R>(items.length);
  let next = 0;
  const worker = async (): Promise<void> => {
    for (let i = next++; i < items.length; i = next++) out[i] = await work(items[i] as T);
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(jobs, items.length)) }, worker));
  return out;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (n: string): string | undefined => argv.find((a) => a.startsWith(`--${n}=`))?.slice(n.length + 3);
  const out = flag('out');
  if (!out) {
    process.stderr.write('usage: node tools/ci-verify.ts --out=<verify.json> [--cargo-log=<file>] [--typecheck-log=<file>] [--jobs=N] [--no-cache] [dir…]\n');
    process.exit(2);
  }
  const start = performance.now();
  const startedAt = Date.now();
  const ws = workspace();
  const only = argv.filter((a) => !a.startsWith('--'));
  const files = git(['ls-files', '--cached', '--others', '--exclude-standard']).split('\n').filter(Boolean);
  const dirs = only.length ? only.map((d) => d.replace(/\/$/, '')) : candidates(files, ws.dirs);
  // monoflow names each package; a folder it doesn't know, or knows under a name another folder took (three
  // packages are called `wisp`: monoflow keeps one), is skipped as `verify --all` skipped it
  const owners = await pool(dirs, 8, async (d) => ({ d, r: parse((await monoflow(['graph', 'query', d])).out) }));
  const targets: { name: string; path: string; how: How }[] = [];
  const skipped: string[] = [];
  const rootLint = files.some((f) => ROOT_LINT.test(f));
  let programs: (Program | undefined)[] | undefined;
  const pkgPrograms = new Map<string, Program>();
  for (const { d, r } of owners) {
    const name = isObj(r) && typeof r['package'] === 'string' ? r['package'] : undefined;
    const path = isObj(r) && typeof r['path'] === 'string' ? slash(r['path']) : undefined;
    if (!name || path !== d || (ws.dirs.has(d) && ws.names.has(name))) {
      skipped.push(d);
      continue;
    }
    const manifest = join(ROOT, d, 'package.json');
    const pj: unknown = existsSync(manifest) ? JSON.parse(readFileSync(manifest, 'utf8')) : undefined;
    const scripts = isObj(pj) && isObj(pj['scripts']) ? (Object.fromEntries(Object.entries(pj['scripts']).filter(([, v]) => typeof v === 'string')) as Record<string, string>) : undefined;
    const how = plan(scripts, {
      vitestConfig: existsSync(join(ROOT, d, 'vitest.config.ts')),
      rootLint,
      covered: (config) => {
        programs ??= PROGRAMS.map(readProgram);
        const p = readProgram(posix.join(d, config));
        if (p) pkgPrograms.set(posix.join(d, config), p);
        return covers(p, programs);
      },
    });
    targets.push({ name, path: d, how });
  }
  if (skipped.length) process.stdout.write(`ci-verify: not monoflow packages of their own (as in verify --all): ${skipped.join(', ')}\n`);
  const describe = (h: How): string => (h.kind === 'monoflow' ? 'monoflow' : `typecheck: ${h.typecheck === 'covered' ? 'npm run typecheck' : 'monoflow'}, test: ${h.test === 'vitest' ? 'vitest (one run)' : 'monoflow'}`);
  if (argv.includes('--list')) {
    for (const t of targets) process.stdout.write(`${t.name}\t${t.path}\t${describe(t.how)}\n`);
    process.exit(0);
  }
  const noCache = argv.includes('--no-cache') ? ['--no-cache'] : [];
  const split = targets.filter((t): t is typeof t & { how: Extract<How, { kind: 'split' }> } => t.how.kind === 'split');
  const vitestDirs = split.filter((t) => t.how.test === 'vitest').map((t) => t.path);
  const covered = split.filter((t) => t.how.typecheck === 'covered');

  // the monoflow calls: whole packages (crates outside the workspace first: the longest builds), then single checks
  type Call = { name: string; path: string; args: string[]; via: string };
  const isCrate = (p: string): boolean => existsSync(join(ROOT, p, 'Cargo.toml')) && !existsSync(join(ROOT, p, 'package.json'));
  const calls: Call[] = [
    ...targets.filter((t) => t.how.kind === 'monoflow').sort((a, b) => Number(isCrate(b.path)) - Number(isCrate(a.path))).map((t) => ({ name: t.name, path: t.path, args: [], via: 'monoflow' })),
    ...split.filter((t) => t.how.typecheck === 'monoflow').map((t) => ({ name: t.name, path: t.path, args: ['--typecheck'], via: 'monoflow --typecheck' })),
    ...split.filter((t) => t.how.test === 'monoflow').map((t) => ({ name: t.name, path: t.path, args: ['--test'], via: 'monoflow --test' })),
  ];
  const jobs = Number(flag('jobs')) || Math.max(1, Math.ceil(availableParallelism() / 2));
  process.stdout.write(
    `ci-verify: ${targets.length} packages: ${calls.length} monoflow calls (${jobs} at a time), ${vitestDirs.length} packages' tests in one vitest run, ${covered.length} typechecks already in npm run typecheck; ${ws.names.size} workspace crates left to the cargo steps\n`,
  );

  const monoflowRuns = pool(calls, jobs, async (c) => {
    const t0 = performance.now();
    // --no-fix: CI never rewrites the checkout (a reformatted file is uncommitted work, which starts a background run)
    const r = await monoflow(['verify', `--package=${c.name}`, '--json', '--no-fix', ...c.args, ...noCache], false);
    const piece = monoflowPiece(parse(r.out), performance.now() - t0, c.via);
    process.stdout.write(`  ${piece.ok ? 'ok  ' : 'FAIL'} ${c.name} (${c.via}, ${(piece.ms / 1000).toFixed(1)} s)\n`);
    return { call: c, piece };
  });

  const vitestRun = (async (): Promise<{ report: unknown; code: number; ms: number } | undefined> => {
    if (!vitestDirs.length) return undefined;
    const tmp = mkdtempSync(join(tmpdir(), 'wisp-ci-vitest-'));
    const file = join(tmp, 'report.json');
    const t0 = performance.now();
    try {
      // 42 packages share one worker pool here, beside the cargo builds: a UI test that takes 0.3 s alone took over 5 s
      // (vitest's default) on a busy Windows runner and failed main, which holds the release (2026-11-26). The flag
      // reaches every project, unlike a testTimeout in the root config; a test that really hangs still fails.
      const r = await run(
        process.execPath,
        [join(ROOT, 'node_modules', 'vitest', 'vitest.mjs'), 'run', '--config', join(ROOT, 'tools', 'vitest.ci.config.ts'), '--testTimeout=20000', '--reporter=dot', '--reporter=json', `--outputFile.json=${file}`],
        { ...process.env, WISP_VITEST_PROJECTS: JSON.stringify(vitestDirs) },
        false,
      );
      const report: unknown = existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : undefined;
      const ms = performance.now() - t0;
      process.stdout.write(`  ${r.code === 0 ? 'ok  ' : 'FAIL'} vitest, ${vitestDirs.length} packages in one run (${(ms / 1000).toFixed(1)} s)\n`);
      return { report, code: r.code, ms };
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  })();

  const typecheckRun = (async (): Promise<{ via: string; result: ReturnType<typeof tscFailures> } | undefined> => {
    if (!covered.length) return undefined;
    const log = flag('typecheck-log');
    const fromStep = log !== undefined && existsSync(log) ? tscFailures(readFileSync(log, 'utf8')) : undefined;
    if (fromStep?.done) return { via: 'npm run typecheck', result: fromStep };
    // the typecheck step didn't run (or didn't finish): run it here, so those packages are still type-checked
    process.stdout.write('ci-verify: no finished npm run typecheck log; running tools/typecheck.ts\n');
    const r = await run(process.execPath, [join(ROOT, 'tools', 'typecheck.ts')], process.env);
    const result = tscFailures(r.out);
    if (!result.done) result.other.push(`tools/typecheck.ts ended (exit ${r.code}) before its summary: ${stripAnsi(r.out).trim().slice(-400)}`);
    return { via: 'npm run typecheck (here)', result };
  })();

  const [mf, vt, tc] = await Promise.all([monoflowRuns, vitestRun, typecheckRun]);
  // what the calls started in the background ends here, before the server budgets step times anything
  await stopStrayRuns(ROOT, startedAt, (l) => process.stdout.write(l));

  // the packages' pieces
  const pieces = new Map<string, Piece[]>(targets.map((t) => [t.name, []]));
  for (const { call, piece } of mf) pieces.get(call.name)?.push(piece);
  const extra: Part[] = [];
  if (vt) {
    const by = vt.report === undefined ? undefined : vitestByPackage(vt.report, vitestDirs, [ROOT, realpathSync.native(ROOT)], vt.code);
    const share = vitestDirs.length ? vt.ms / vitestDirs.length : 0;
    for (const t of split.filter((s) => s.how.test === 'vitest')) {
      const at = by?.per.get(t.path);
      const errors = at ? [...at.errors] : [{ code: 'execution_failure', message: `vitest wrote no report (exit ${vt.code}): see the vitest output above`, targets: [] }];
      if (at && at.files === 0 && t.how.needsTests) errors.push({ code: 'test-failure', message: `vitest found no test files in ${t.path} (its test script has no --passWithNoTests)`, targets: [] });
      pieces.get(t.name)?.push({ via: 'vitest (one run)', ok: errors.length === 0, ms: at?.ms || share, errors });
    }
    // a failure vitest can't place in a package fails a target of its own, so neither the job nor G14 loses it
    if (by?.other.length) extra.push({ target: 'vitest', path: '.', ms: vt.ms, via: 'vitest (one run)', raw: { status: 'failed', errors: by.other.map((m) => ({ code: 'test-failure', message: m, targets: ['vitest'] })) } });
  }
  if (tc) {
    // an error is the package's whose config holds the file, else the package whose folder it is in
    const owner = new Map<string, string[]>();
    for (const t of covered) {
      const mine = new Set(t.how.configs.flatMap((c) => pkgPrograms.get(posix.join(t.path, c))?.files ?? []));
      for (const file of tc.result.files.keys()) if (mine.has(resolve(ROOT, file))) owner.set(file, [...(owner.get(file) ?? []), t.name]);
    }
    const byFolder = [...covered].sort((a, b) => b.path.length - a.path.length);
    for (const file of tc.result.files.keys()) {
      const t = owner.has(file) ? undefined : byFolder.find((c) => file.startsWith(`${c.path}/`));
      if (t) owner.set(file, [t.name]);
    }
    for (const t of covered) {
      const errs = [...tc.result.files].filter(([file]) => owner.get(file)?.includes(t.name)).map(([file, what]) => ({ code: 'typecheck', file, message: what.join('\n'), targets: [] }));
      pieces.get(t.name)?.unshift({ via: tc.via, ok: errs.length === 0, ms: 0, errors: errs });
    }
    // a file in no package (tools/, apps/server): no monoflow package ever type-checked it; the typecheck step fails
    // the job for it, so it is named here, not credited (G14)
    const loose = [...tc.result.files.keys()].filter((f) => !owner.has(f));
    if (loose.length) process.stdout.write(`ci-verify: typecheck errors in no package: ${loose.join(', ')}\n`);
    // an error with no file, or a typecheck that never finished: no package's result can be trusted, so a target of
    // its own fails (as every package's own tsc -p would have)
    if (tc.result.other.length) extra.push({ target: 'typecheck', path: '.', ms: 0, via: tc.via, raw: { status: 'failed', errors: tc.result.other.map((m) => ({ code: 'typecheck', message: m, targets: ['typecheck'] })) } });
  }
  const parts: Part[] = [...targets.map((t) => combine(t.name, t.path, pieces.get(t.name) ?? [])), ...extra];
  for (const p of parts) {
    const j = isObj(p.raw) ? p.raw : {};
    if (j['status'] !== 'passed') process.stdout.write(`  FAIL ${p.target} (${p.via ?? 'monoflow'})\n`);
  }
  const log = flag('cargo-log');
  const ran = log !== undefined && existsSync(log);
  const result = merge(parts, { ran, failures: ran ? cargoFailures(readFileSync(log, 'utf8')) : new Map(), crates: ws.names.size, dirs: ws.byName }, performance.now() - start);
  writeFileSync(out, `${JSON.stringify(result, null, 2)}\n`);
  process.stdout.write(`ci-verify: ${((performance.now() - start) / 1000).toFixed(1)} s\n`);
  process.exitCode = parts.some((p) => !(isObj(p.raw) && p.raw['status'] === 'passed')) ? 1 : 0;
}
