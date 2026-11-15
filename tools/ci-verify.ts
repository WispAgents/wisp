// CI's monoflow pass (suite.yml, ADR-0069): `monoflow verify --no-cache` for every package monoflow knows except the
// Cargo workspace's members, which the job's own cargo steps already check once (`cargo clippy --workspace
// --all-targets`, `cargo test --workspace --no-fail-fast`). `verify --all` re-ran them crate by crate (`cargo check
// -p`, `cargo clippy -p`, `cargo test -p`, each with that crate's own feature set, so the workspace was rebuilt and
// every test ran twice): 23 of the 33 min on Linux, and past the 60-minute limit on macOS. What's left (the npm
// workspaces, sdk/python, the crates outside the workspace such as spikes/*) runs as before, one package per
// monoflow call, `--jobs` at a time. Failures of the cargo test step (its log, `--cargo-log`) join the result as
// failed crates, so the job summary and gate G14 (tools/g14.ts) see every failing package as they did.
//
//   node tools/ci-verify.ts --out=verify-all-1.json [--cargo-log=<cargo test output>] [--jobs=N] [--no-cache] [dir…]
//   node tools/ci-verify.ts --out=- --list        the packages it would verify, and the folders it skips
//
// The output has monoflow's `--json` shape (status, counts, `failed`, `errors`, `results` with each target's time),
// read by tools/monoflow-report.ts. Exit 1 when a monoflow package failed (the cargo step fails the job itself).

import { execFileSync, spawn } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { dirname, join, posix, relative, sep } from 'node:path';
import { DEST } from './setup-monoflow.ts';

const ROOT = join(import.meta.dirname, '..');

/** The manifests monoflow discovers packages from (2a86d14: graph/discover_*.go) and the folders it never looks in. */
const MANIFESTS = new Set(['package.json', 'Cargo.toml', 'pyproject.toml', 'setup.py', 'go.mod']);
const SKIPPED = new Set(['node_modules', 'vendor', 'testdata', '__fixtures__', 'fixtures', '.agents', '.git', 'target', 'dist', 'build', '.venv', 'venv', '__pycache__', 'third_party']);

export interface Target {
  target: string;
  path: string;
  status: 'passed' | 'failed';
  duration_ms: number;
  /** what ran it: `monoflow` or `cargo test --workspace` */
  via: string;
}

export interface Part {
  target: string;
  path: string;
  raw: unknown;
  ms: number;
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

/** The crates whose tests failed or didn't build, from `cargo test --no-fail-fast`'s output (colours and all). */
export function cargoFailures(log: string): Map<string, string[]> {
  const out = new Map<string, string[]>();
  const add = (crate: string, what: string): void => {
    const l = out.get(crate) ?? [];
    if (!l.includes(what)) l.push(what);
    out.set(crate, l);
  };
  const lines = log.replace(/\x1b\[[0-9;]*[A-Za-z]/g, '').split(/\r?\n/);
  let inList = false;
  for (const raw of lines) {
    const line = raw.replace(/^\d{4}-\d\d-\d\dT[\d:.]+Z /, ''); // a GitHub log's timestamps
    const built = /error: could not compile `([^`]+)`(?: \(([^)]+)\))?/.exec(line);
    if (built?.[1]) add(built[1], `doesn't compile${built[2] ? ` (${built[2]})` : ''}`);
    if (/^error: \d+ targets? failed:/.test(line.trim())) {
      inList = true;
      continue;
    }
    if (inList) {
      const t = /^\s+`-p (\S+) ([^`]*)`/.exec(line);
      if (t?.[1]) add(t[1], `${t[2]?.trim() ?? ''} failed`.trim());
      else inList = false;
    }
  }
  return out;
}

/** Folds the monoflow calls and the cargo step into one `monoflow verify --json`-shaped result. */
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
    results.push({ target: p.target, path: p.path, status: ok ? 'passed' : 'failed', duration_ms: num(j['duration_ms']) || p.ms, via: 'monoflow' });
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

/** One monoflow call, as the wrapper makes it (no background speculation). */
function monoflow(args: string[], json = true): Promise<{ code: number; out: string }> {
  return new Promise((done) => {
    const child = spawn(join(ROOT, DEST), args, {
      cwd: ROOT,
      env: { ...process.env, ...WRAPPER_ENV, ...(json ? { MONOFLOW_FORMAT: 'json' } : {}) },
      stdio: ['ignore', 'pipe', 'inherit'],
    });
    let out = '';
    child.stdout.setEncoding('utf8').on('data', (d: string) => (out += d));
    child.on('error', (e) => done({ code: -1, out: JSON.stringify({ status: 'failed', error: e.message }) }));
    child.on('close', (code) => done({ code: code ?? -1, out }));
  });
}

const parse = (s: string): unknown => {
  const at = s.indexOf('{');
  try {
    return JSON.parse(at > 0 ? s.slice(at) : s);
  } catch {
    return { status: 'failed', error: `monoflow printed no JSON: ${s.trim().slice(-400)}` };
  }
};

async function pool<T, R>(items: readonly T[], jobs: number, run: (t: T) => Promise<R>): Promise<R[]> {
  const out: R[] = new Array<R>(items.length);
  let next = 0;
  const worker = async (): Promise<void> => {
    for (let i = next++; i < items.length; i = next++) out[i] = await run(items[i] as T);
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(jobs, items.length)) }, worker));
  return out;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (n: string): string | undefined => argv.find((a) => a.startsWith(`--${n}=`))?.slice(n.length + 3);
  const out = flag('out');
  if (!out) {
    process.stderr.write('usage: node tools/ci-verify.ts --out=<verify.json> [--cargo-log=<file>] [--jobs=N] [--no-cache] [dir…]\n');
    process.exit(2);
  }
  const start = performance.now();
  const ws = workspace();
  const only = argv.filter((a) => !a.startsWith('--'));
  const files = git(['ls-files', '--cached', '--others', '--exclude-standard']).split('\n').filter(Boolean);
  const dirs = only.length ? only.map((d) => d.replace(/\/$/, '')) : candidates(files, ws.dirs);
  // monoflow names each package; a folder it doesn't know, or knows under a name another folder took (three
  // packages are called `wisp`: monoflow keeps one), is skipped as `verify --all` skipped it
  const owners = await pool(dirs, 8, async (d) => ({ d, r: parse((await monoflow(['graph', 'query', d])).out) }));
  const targets: { name: string; path: string }[] = [];
  const skipped: string[] = [];
  for (const { d, r } of owners) {
    const name = isObj(r) && typeof r['package'] === 'string' ? r['package'] : undefined;
    const path = isObj(r) && typeof r['path'] === 'string' ? r['path'].replaceAll('\\', '/') : undefined;
    if (name && path === d && !(ws.dirs.has(d) && ws.names.has(name))) targets.push({ name, path });
    else skipped.push(d);
  }
  if (skipped.length) process.stdout.write(`ci-verify: not monoflow packages of their own (as in verify --all): ${skipped.join(', ')}\n`);
  if (argv.includes('--list')) {
    for (const t of targets) process.stdout.write(`${t.name}\t${t.path}\n`);
    process.exit(0);
  }
  const jobs = Number(flag('jobs')) || Math.max(1, Math.ceil(availableParallelism() / 2));
  process.stdout.write(`ci-verify: ${targets.length} packages through monoflow, ${jobs} at a time; ${ws.names.size} workspace crates left to the cargo steps\n`);
  const parts = await pool(targets, jobs, async (t): Promise<Part> => {
    const t0 = performance.now();
    const r = await monoflow(['verify', `--package=${t.name}`, '--json', ...(argv.includes('--no-cache') ? ['--no-cache'] : [])], false);
    const ms = performance.now() - t0;
    const raw = parse(r.out);
    process.stdout.write(`  ${isObj(raw) && raw['status'] === 'passed' ? 'ok  ' : 'FAIL'} ${t.name} (${(ms / 1000).toFixed(1)} s)\n`);
    return { target: t.name, path: t.path, raw, ms };
  });
  const log = flag('cargo-log');
  const ran = log !== undefined && existsSync(log);
  const result = merge(parts, { ran, failures: ran ? cargoFailures(readFileSync(log, 'utf8')) : new Map(), crates: ws.names.size, dirs: ws.byName }, performance.now() - start);
  writeFileSync(out, `${JSON.stringify(result, null, 2)}\n`);
  process.exitCode = parts.some((p) => !(isObj(p.raw) && p.raw['status'] === 'passed')) ? 1 : 0;
}
