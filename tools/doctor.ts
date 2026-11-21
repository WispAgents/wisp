// `npm run doctor`: is this checkout ready to work on, and what to run if not. Read-only and fast (no builds, no
// cargo or rustc calls, which could download a toolchain): Node, npm deps vs the lockfile, rustup and the pinned
// toolchain, CARGO_TARGET_DIR and free disk, monoflow, git hooks, the web build, the fake agents, the desktop and
// iOS prerequisites, and ports 4800 (npm run dev) and 4477 (an installed wisp, left alone).
//
//   node tools/doctor.ts          one line per check, the fix under it; exit 1 on a FAIL line
//   node tools/doctor.ts --json   {ok, ms, checks: [{id, title, status, detail, fix?}]} for agents
//
// The decisions live in tools/doctor/plan.ts (tools/test/doctor.test.ts).

import { execFile } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statfsSync, statSync } from 'node:fs';
import { connect } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';
import {
  type Check, depsCheck, desktopCheck, devPortCheck, diskCheck, exitCode, fakeAgentsCheck, format, hooksCheck, installedWispCheck,
  DEV_PORT, INSTALLED_PORT, type Lockfile, lockDrift, monoflowCheck, nodeCheck, type Os, parseToolchainToml, targetDirCheck,
  toolchainChecks, webBuildCheck, xcodeCheck,
} from './doctor/plan.ts';
import { DEST, strays, WRAPPER } from './setup-monoflow.ts';

const ROOT = join(import.meta.dirname, '..');
const run = promisify(execFile);
const EXE = process.platform === 'win32' ? '.exe' : '';

/**
 * rustup must never download a toolchain here: in the repo (or under RUSTUP_TOOLCHAIN) even `rustup --version` runs
 * the active rustc, which installs rust-toolchain.toml's channel. So rustup runs outside the repo, with no override
 * and auto-install off.
 */
const RUSTUP_ENV: NodeJS.ProcessEnv = { ...process.env, RUSTUP_AUTO_INSTALL: '0' };
delete RUSTUP_ENV['RUSTUP_TOOLCHAIN'];

/** stdout of a command, undefined when it's missing or fails; never longer than 2 s. */
async function out(cmd: string, args: readonly string[], opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {}): Promise<string | undefined> {
  try {
    const r = await run(cmd, args, { cwd: opts.cwd ?? ROOT, env: opts.env ?? process.env, timeout: 2000, encoding: 'utf8', windowsHide: true });
    return r.stdout.trim();
  } catch {
    return undefined;
  }
}

/** Like out(), but tells "not on PATH" from "ran and failed or timed out". */
async function present(cmd: string, args: readonly string[], opts: { cwd?: string; env?: NodeJS.ProcessEnv }): Promise<{ found: boolean; stdout: string | undefined }> {
  try {
    const r = await run(cmd, args, { cwd: opts.cwd ?? ROOT, env: opts.env ?? process.env, timeout: 2000, encoding: 'utf8', windowsHide: true });
    return { found: true, stdout: r.stdout.trim() };
  } catch (e) {
    return { found: (e as NodeJS.ErrnoException).code !== 'ENOENT', stdout: undefined };
  }
}

const rustupOut = (args: readonly string[]): Promise<string | undefined> => out('rustup', args, { cwd: tmpdir(), env: RUSTUP_ENV });

/** The override toolchain's rustc version, run from its own dir (not the proxy, so nothing downloads). */
async function overrideVersion(name: string | undefined): Promise<string | undefined> {
  if (!name) return undefined;
  const home = process.env['RUSTUP_HOME'] ?? join(homedir(), '.rustup');
  const dir = join(home, 'toolchains');
  const hit = existsSync(dir) ? readdirSync(dir).find((d) => d === name || d.startsWith(`${name}-`)) : undefined;
  if (!hit) return undefined;
  const v = await out(join(dir, hit, 'bin', `rustc${EXE}`), ['--version'], { cwd: tmpdir() });
  return v ? /\d+\.\d+\.\d+/.exec(v)?.[0] : undefined;
}

/** A path as a person would type it: ~ for the home dir. */
function tidy(p: string): string {
  const h = homedir();
  return p === h || p.startsWith(`${h}/`) || p.startsWith(`${h}\\`) ? `~${p.slice(h.length)}` : p;
}

function readJson<T>(path: string): T | undefined {
  try {
    return JSON.parse(readFileSync(path, 'utf8')) as T;
  } catch {
    return undefined;
  }
}

/** True when something accepts a connection on 127.0.0.1:port. Connects and closes: never sends a byte. */
function listening(port: number): Promise<boolean> {
  return new Promise((done) => {
    const s = connect({ host: '127.0.0.1', port });
    const end = (v: boolean): void => {
      s.destroy();
      done(v);
    };
    s.setTimeout(400, () => end(false));
    s.once('connect', () => end(true));
    s.once('error', () => end(false));
  });
}

function freeBytes(path: string): number | undefined {
  let p = resolve(path);
  while (!existsSync(p) && dirname(p) !== p) p = dirname(p);
  try {
    const s = statfsSync(p);
    return s.bavail * s.bsize;
  } catch {
    return undefined;
  }
}

function os(): Os {
  const p = process.platform;
  return p === 'darwin' || p === 'linux' || p === 'win32' ? p : 'other';
}

async function desktopPresent(o: Os, xcodeSelect: string | undefined): Promise<boolean | undefined> {
  if (o === 'darwin') return xcodeSelect !== undefined && existsSync(xcodeSelect);
  if (o === 'linux') {
    if ((await out('pkg-config', ['--version'])) === undefined) return undefined;
    return (await out('pkg-config', ['--exists', 'webkit2gtk-4.1'])) !== undefined;
  }
  if (o === 'win32') {
    const key = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
    const at = [`HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\${key}`, `HKCU\\Software\\Microsoft\\EdgeUpdate\\Clients\\${key}`];
    const found = await Promise.all(at.map((k) => out('reg', ['query', k, '/v', 'pv'])));
    return found.some((f) => f !== undefined && /\d+\.\d+/.test(f));
  }
  return undefined;
}

async function rust(): Promise<Check[]> {
  const toml = parseToolchainToml(existsSync(join(ROOT, 'rust-toolchain.toml')) ? readFileSync(join(ROOT, 'rust-toolchain.toml'), 'utf8') : '');
  const pinned = toml.channel ?? 'stable';
  const override = process.env['RUSTUP_TOOLCHAIN'];
  const [version, list, ov] = await Promise.all([
    present('rustup', ['--version'], { cwd: tmpdir(), env: RUSTUP_ENV }), rustupOut(['toolchain', 'list']), overrideVersion(override),
  ]);
  const installed = (list ?? '').split('\n').map((l) => l.trim().split(/\s+/)[0] ?? '').filter(Boolean);
  const has = installed.some((n) => n === pinned || n.startsWith(`${pinned}-`));
  const comps = has ? await rustupOut(['component', 'list', '--installed', '--toolchain', pinned]) : undefined;
  return toolchainChecks({
    // found but failing (slow, or a broken default toolchain) still counts as installed
    rustup: version.found ? (version.stdout?.split('\n')[0] ?? 'rustup (its --version failed)') : undefined,
    pinned,
    installed,
    override,
    overrideVersion: ov,
    components: comps?.split('\n').map((l) => l.trim()).filter(Boolean),
    wantComponents: toml.components,
  });
}

async function hooks(): Promise<Check> {
  const [path, configured] = await Promise.all([out('git', ['rev-parse', '--git-path', 'hooks']), out('git', ['config', 'core.hooksPath'])]);
  const dir = path ? resolve(ROOT, path) : undefined;
  const left = strays(ROOT);
  const isFile = (f: string): boolean => {
    try {
      return dir !== undefined && statSync(join(dir, f)).isFile();
    } catch {
      return false;
    }
  };
  const other = dir && existsSync(dir)
    ? readdirSync(dir).filter((f) => !f.endsWith('.sample') && isFile(f) && !left.some((s) => s.startsWith(`${join(path ?? '', f)} `)))
    : [];
  return hooksCheck(left, other, configured);
}

async function main(): Promise<void> {
  const t0 = performance.now();
  const json = process.argv.includes('--json');
  const o = os();
  const pkg = readJson<{ engines?: { node?: string } }>(join(ROOT, 'package.json'));
  const lock = readJson<Lockfile>(join(ROOT, 'package-lock.json')) ?? {};
  const hasModules = existsSync(join(ROOT, 'node_modules'));
  const hidden = hasModules ? readJson<Lockfile>(join(ROOT, 'node_modules', '.package-lock.json')) : undefined;
  const targetEnv = process.env['CARGO_TARGET_DIR'];
  const target = targetEnv ? resolve(ROOT, targetEnv) : join(ROOT, 'target');
  const fakes = ['wisp-fake-agent', 'wisp-fake-gh'];
  const fakeDir = join(target, 'debug');

  const xcodeSelect = o === 'darwin' ? await out('xcode-select', ['-p']) : undefined;
  const [rustChecks, hookCheck, desktop, devBusy, installedBusy] = await Promise.all([
    rust(), hooks(), desktopPresent(o, xcodeSelect), listening(DEV_PORT), listening(INSTALLED_PORT),
  ]);
  const xcode = existsSync('/Applications/Xcode.app') || /Xcode[^/]*\.app/.test(xcodeSelect ?? '');

  const checks: Check[] = [
    nodeCheck(process.version, pkg?.engines?.node),
    depsCheck(hasModules, hidden ? lockDrift(lock, hidden) : undefined),
    ...rustChecks,
    targetDirCheck(targetEnv && tidy(target), tidy(join(ROOT, 'target'))),
    diskCheck(freeBytes(target), tidy(target)),
    monoflowCheck(existsSync(join(ROOT, DEST)), existsSync(join(ROOT, WRAPPER))),
    hookCheck,
    webBuildCheck(existsSync(join(ROOT, 'apps', 'web', 'dist', 'index.html'))),
    fakeAgentsCheck(fakes.filter((f) => existsSync(join(fakeDir, `${f}${EXE}`))), fakes, tidy(fakeDir)),
    desktopCheck(o, desktop),
    ...[xcodeCheck(o, xcode)].filter((c): c is Check => c !== undefined),
    devPortCheck(devBusy),
    installedWispCheck(installedBusy),
  ];
  const ms = Math.round(performance.now() - t0);
  const code = exitCode(checks);
  process.stdout.write(json ? `${JSON.stringify({ ok: code === 0, ms, checks }, null, 2)}\n` : format(checks, ms));
  process.exitCode = code;
}

main().catch((e: unknown) => {
  process.stderr.write(`[doctor] ${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exitCode = 1;
});
