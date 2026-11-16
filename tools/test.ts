// `npm test`: the tests of tools/ itself, then every workspace package's tests. With monoflow installed
// (.agents/bin/monoflow, see tools/setup-monoflow.ts) the packages run through `monoflow verify --all
// --test` (cached, dependency-aware); without it, each workspace's `test` script runs in dependency
// order. Every step runs even after a failure, so one run shows everything that is broken. Output is
// one line per step; a failing step prints the tail of its output.
//
//   node tools/test.ts               everything
//   node tools/test.ts --no-monoflow the plain npm fallback, even when monoflow is installed
//   node tools/test.ts --verbose     stream every step's output

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { delimiter, join } from 'node:path';
import { discover } from './check/core/workspace.ts';
import type { Pkg } from './check/core/types.ts';

const ROOT = join(import.meta.dirname, '..');
const TAIL = 60;

export interface Step {
  name: string;
  cmd: string;
  args: string[];
}

export function monoflowBin(root: string, platform: string = process.platform): string | undefined {
  // the wrapper (a sh script) everywhere but Windows, where Node can't spawn it: the binary there
  const names = platform === 'win32' ? ['monoflow-bin.exe'] : ['monoflow'];
  return names.map((b) => join(root, '.agents', 'bin', b)).find((p) => existsSync(p));
}

/** Kahn's algorithm, ties by name; packages in a cycle go last (the cycles check reports them). */
export function dependencyOrder(pkgs: readonly Pkg[]): Pkg[] {
  const left = new Map(pkgs.map((p) => [p.name, p]));
  const out: Pkg[] = [];
  while (left.size) {
    const ready = [...left.values()].filter((p) => [...p.declared].every((d) => !left.has(d))).sort((a, b) => a.name.localeCompare(b.name));
    const next = ready.length ? ready : [...left.values()].sort((a, b) => a.name.localeCompare(b.name));
    for (const p of next) {
      out.push(p);
      left.delete(p.name);
    }
  }
  return out;
}

function hasTest(p: Pkg): boolean {
  const s = p.manifest['scripts'];
  return typeof s === 'object' && s !== null && 'test' in s;
}

export function plan(root: string, opts: { monoflow: boolean; platform?: string }): Step[] {
  const platform = opts.platform ?? process.platform;
  const npm = platform === 'win32' ? 'npm.cmd' : 'npm';
  const steps: Step[] = [{ name: 'tools', cmd: process.execPath, args: ['--test', '--test-reporter=dot', 'tools/**/*.test.ts'] }];
  const mf = opts.monoflow ? monoflowBin(root, platform) : undefined;
  if (mf) return [...steps, { name: 'monoflow verify --all --test', cmd: mf, args: ['verify', '--all', '--test'] }];
  for (const p of dependencyOrder(discover(root)).filter(hasTest)) steps.push({ name: p.name, cmd: npm, args: ['run', 'test', `--workspace=${p.name}`] });
  return steps;
}

/** The pytest CI pins (.github/workflows/suite.yml, pr.yml). */
export const PYTEST = 'pytest==8.3.5';

/** The interpreter sdk/python's tests run with: `python` with pytest on PATH (CI's setup-python), else a venv this
 *  checkout keeps in target/py from `python3` (macOS ships python3 only, so `python -m pytest` failed every local
 *  run), pytest pinned as in CI. The directory to put first on PATH, '' when `python` already works, or the reason
 *  it can't (no python3; pip couldn't reach the index). */
export function pythonPath(root: string, run: typeof spawnSync = spawnSync): { dir: string } | { error: string } {
  const has = (cmd: string): boolean => run(cmd, ['-m', 'pytest', '--version'], { encoding: 'utf8' }).status === 0;
  if (has('python')) return { dir: '' };
  const venv = join(root, 'target', 'py');
  const bin = join(venv, process.platform === 'win32' ? 'Scripts' : 'bin');
  const py = join(bin, process.platform === 'win32' ? 'python.exe' : 'python');
  if (existsSync(py) && has(py)) return { dir: bin };
  if (run('python3', ['--version'], { encoding: 'utf8' }).status !== 0) return { error: 'sdk/python needs python3 (with pytest), and there is none on PATH' };
  if (!existsSync(py) && run('python3', ['-m', 'venv', venv], { encoding: 'utf8' }).status !== 0) return { error: `python3 -m venv ${venv} failed` };
  const pip = run(py, ['-m', 'pip', 'install', '--disable-pip-version-check', '--quiet', PYTEST], { encoding: 'utf8' });
  return pip.status === 0 ? { dir: bin } : { error: `pip install ${PYTEST} into ${venv} failed: ${(pip.stderr ?? '').trim().split('\n').at(-1) ?? ''}` };
}

function runStep(step: Step, verbose: boolean, env?: NodeJS.ProcessEnv): boolean {
  const t0 = performance.now();
  const r = spawnSync(step.cmd, step.args, { cwd: ROOT, stdio: verbose ? 'inherit' : 'pipe', encoding: 'utf8', shell: process.platform === 'win32' && step.cmd.endsWith('.cmd'), ...(env && { env }) });
  const secs = ((performance.now() - t0) / 1000).toFixed(1);
  const ok = r.status === 0;
  process.stdout.write(`${ok ? '✔' : '✖'} ${step.name} (${secs}s)\n`);
  if (!ok && !verbose) {
    const out = `${r.stdout ?? ''}${r.stderr ?? ''}${r.error ? `\n${r.error.message}` : ''}`.trimEnd().split('\n');
    process.stdout.write(`${out.slice(-TAIL).map((l) => `  ${l}`).join('\n')}\n`);
  }
  return ok;
}

if (import.meta.main) {
  const verbose = process.argv.includes('--verbose');
  const steps = plan(ROOT, { monoflow: !process.argv.includes('--no-monoflow') });
  const py = existsSync(join(ROOT, 'sdk', 'python')) ? pythonPath(ROOT) : { dir: '' };
  if ('error' in py) process.stdout.write(`test: ${py.error}; sdk/python's tests will fail\n`);
  const env = 'dir' in py && py.dir ? { ...process.env, PATH: `${py.dir}${delimiter}${process.env['PATH'] ?? ''}` } : undefined;
  const failed = steps.filter((s) => !runStep(s, verbose, env));
  process.stdout.write(failed.length ? `test: ${failed.length} of ${steps.length} failed: ${failed.map((s) => s.name).join(', ')}\n` : `test: ${steps.length} passed\n`);
  process.exitCode = failed.length ? 1 : 0;
}
