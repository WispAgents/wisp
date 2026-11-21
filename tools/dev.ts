// `npm run dev` / `npm run dev:desktop`: wisp from this checkout, in dev mode.
//   1. `cargo build -p wisp`, then `wisp serve` on its own port (4800, else the next free one) and its own home
//      (~/.wisp-dev): never 4477/4791, never ~/.wisp or ~/.wisp-v2-preview (tools/dev/plan.ts).
//   1b. no apps/web/dist yet: `vite build` once, alongside the cargo build, so the server has a page of its own for
//      whatever reaches it without Vite (the grid tunnel to this computer forwards `/` to the server's port).
//   2. Vite with HMR (apps/web), proxying /api to that server instead of the mock peer (WISP_DEV_API).
//   3. --desktop: the Tauri shell (desktop/src-tauri) under dev.wisp.dev (tauri.dev.conf.json via TAURI_CONFIG),
//      attached to that server (WISP_PORT) and loading Vite's page (WISP_DEV_URL).
// UI edits hot reload; a change under crates/ or plugins/*/host rebuilds the server and restarts it on the same port
// and home (the page reconnects); a failed build keeps the old one running. `r` + Enter rebuilds by hand.
// Ctrl-C (or `q` + Enter) stops everything this started, by the pids it started; nothing is killed by name.

import { type ChildProcess, spawn } from 'node:child_process';
import { watch, type FSWatcher } from 'node:fs';
import { access, mkdir, readdir, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { createInterface } from 'node:readline';
import {
  apiUrl, builtBinary, choosePort, desktopEnv, DevArgError, type DesktopOverlay, isRustChange, parseArgs, resolveHome,
  serverEnv, USAGE, viteEnv, viteUrl, webIndex,
} from './dev/plan.ts';

const ROOT = join(import.meta.dirname, '..');
const WEB = join(ROOT, 'apps', 'web');
const DESKTOP = join(ROOT, 'desktop', 'src-tauri');
const STOP_GRACE_MS = 5000;
const READY_LIMIT_MS = 60_000;

const say = (s: string): void => void process.stdout.write(`\x1b[35m[dev]\x1b[0m ${s}\n`);

function freeOn(port: number, host: string): Promise<boolean> {
  return new Promise((done) => {
    const s = createServer();
    // no IPv6 here at all: nothing can be listening on it
    s.once('error', (e: NodeJS.ErrnoException) => done(e.code === 'EADDRNOTAVAIL' || e.code === 'EAFNOSUPPORT'));
    s.listen({ port, host, exclusive: true }, () => s.close(() => done(true)));
  });
}

/** Free on both loopbacks: `localhost` may reach either. */
async function isFree(port: number): Promise<boolean> {
  return (await freeOn(port, '127.0.0.1')) && (await freeOn(port, '::1'));
}

/** Cargo's env: AGENTS.md says cargo runs without RUSTUP_TOOLCHAIN (rust-toolchain.toml picks it). */
function cargoEnv(base: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const env = { ...base };
  delete env['RUSTUP_TOOLCHAIN'];
  return env;
}

const children = new Set<ChildProcess>();
let stopping = false;

function track(child: ChildProcess): ChildProcess {
  if (stopping) {
    void stopChild(child); // started while we were stopping: it goes too
    return child;
  }
  children.add(child);
  child.once('exit', () => children.delete(child));
  return child;
}

/** SIGTERM to that pid, SIGKILL after the grace period; resolves once it's gone. */
function stopChild(child: ChildProcess, signal: NodeJS.Signals = 'SIGTERM'): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((done) => {
    const hard = setTimeout(() => child.kill('SIGKILL'), STOP_GRACE_MS);
    child.once('exit', () => (clearTimeout(hard), done()));
    child.stdin?.end(); // wisp serve (WISP_EXIT_WITH_STDIN) stops its agents and exits on this alone
    child.kill(signal);
  });
}

/** `cargo build -p wisp`; the binary's path, or undefined when the build failed. */
function buildServer(): Promise<string | undefined> {
  say('cargo build -p wisp …');
  return new Promise((done) => {
    const cargo = track(spawn('cargo', ['build', '-p', 'wisp', '--bin', 'wisp', '--message-format=json-render-diagnostics'], {
      cwd: ROOT, env: cargoEnv(), stdio: ['ignore', 'pipe', 'inherit'],
    }));
    const lines: string[] = [];
    createInterface({ input: cargo.stdout! }).on('line', (l) => lines.push(l));
    cargo.once('error', (e) => (say(`cargo didn't start: ${e.message} (https://rustup.rs)`), done(undefined)));
    cargo.once('exit', (code) => done(code === 0 ? builtBinary(lines) : undefined));
  });
}

/** `vite build` once when apps/web/dist has no index.html; a failed build only means the server serves the API alone. */
async function buildWebIfMissing(): Promise<void> {
  if (await access(webIndex(ROOT)).then(() => true, () => false)) return;
  say('no web build yet: vite build (once, for pages that reach wisp serve without Vite, like the grid tunnel) …');
  const ok = await new Promise<boolean>((done) => {
    const vite = track(spawn(process.execPath, [viteBin(), 'build', '--logLevel', 'warn'], { cwd: WEB, stdio: ['ignore', 'inherit', 'inherit'] }));
    vite.once('error', () => done(false));
    vite.once('exit', (code) => done(code === 0));
  });
  say(ok ? 'web build done (apps/web/dist; npm run build refreshes it)' : 'web build failed: wisp serve serves the API only, Vite is unaffected');
}

async function waitFor(url: string, alive: () => boolean): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < READY_LIMIT_MS && alive()) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(1000) });
      await r.body?.cancel();
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 150));
    }
  }
  return false;
}

interface Run {
  port: number;
  home: string;
  bin: string | undefined;
  server: ChildProcess | undefined;
}

async function startServer(run: Run): Promise<boolean> {
  if (!run.bin || stopping) return false;
  const child = track(spawn(run.bin, ['serve'], { cwd: run.home, env: serverEnv(process.env, run.port, run.home), stdio: ['pipe', 'inherit', 'inherit'] }));
  run.server = child;
  child.once('exit', (code, sig) => {
    if (run.server === child && !stopping) say(`wisp serve exited (${sig ?? code}); r + Enter rebuilds and starts it`);
  });
  const up = await waitFor(`${apiUrl(run.port)}/api/health`, () => child.exitCode === null && child.signalCode === null);
  if (up) say(`wisp serve up on ${apiUrl(run.port)} (pid ${child.pid}, home ${run.home})`);
  return up;
}

let rebuilding: Promise<void> | undefined;
let again = false;

/** Rebuild; on success restart the server on the same port and home. One at a time; changes meanwhile queue one more. */
function rebuild(run: Run): void {
  if (rebuilding) {
    again = true;
    return;
  }
  rebuilding = (async () => {
    do {
      again = false;
      const bin = await buildServer();
      if (stopping) return;
      if (!bin) {
        say('build failed: the running server stays as it was');
        continue;
      }
      run.bin = bin;
      const old = run.server;
      run.server = undefined; // a planned stop: its exit isn't news
      if (old) await stopChild(old);
      if (stopping) return;
      await startServer(run);
    } while (again && !stopping);
  })().finally(() => (rebuilding = undefined));
}

async function rustDirs(): Promise<string[]> {
  const plugins = await readdir(join(ROOT, 'plugins'), { withFileTypes: true });
  const hosts = plugins.filter((d) => d.isDirectory()).map((d) => join(ROOT, 'plugins', d.name, 'host'));
  const exists = await Promise.all(hosts.map((h) => readdir(h).then(() => true, () => false)));
  return [join(ROOT, 'crates'), ...hosts.filter((_, i) => exists[i])];
}

async function watchRust(run: Run): Promise<FSWatcher[]> {
  let timer: NodeJS.Timeout | undefined;
  const changed = (dir: string) => (_: string, file: string | null) => {
    if (!file || !isRustChange(relative(ROOT, join(dir, file)))) return;
    clearTimeout(timer);
    timer = setTimeout(() => (say(`changed: ${relative(ROOT, join(dir, file))}`), rebuild(run)), 300);
  };
  const dirs = await rustDirs();
  const watchers = [...dirs.map((d) => watch(d, { recursive: true }, changed(d))), watch(ROOT, { recursive: false }, changed(ROOT))];
  // a watcher that fails (EMFILE, its folder gone) stops watching; the run goes on (r + Enter still rebuilds)
  for (const w of watchers) w.on('error', (e) => (say(`file watch stopped: ${e.message}; r + Enter rebuilds`), w.close()));
  return watchers;
}

function viteBin(): string {
  return join(createRequire(join(WEB, 'package.json')).resolve('vite/package.json'), '..', 'bin', 'vite.js');
}

function startVite(vitePort: number, apiPort: number): ChildProcess {
  return track(spawn(process.execPath, [viteBin(), '--port', String(vitePort), '--strictPort'], {
    cwd: WEB, env: viteEnv(process.env, vitePort, apiPort), stdio: ['ignore', 'inherit', 'inherit'],
  }));
}

async function startDesktop(run: Run, vitePort: number): Promise<ChildProcess> {
  const overlay = JSON.parse(await readFile(join(DESKTOP, 'tauri.dev.conf.json'), 'utf8')) as DesktopOverlay;
  const target = process.env['WISP_DEV_DESKTOP_TARGET'] ?? join(tmpdir(), 'wisp-target-desktop-dev');
  const env = desktopEnv(cargoEnv(), run.port, vitePort, run.home, overlay, target, run.bin ?? '');
  say(`cargo run (desktop, ${overlay.identifier}) …`);
  return track(spawn('cargo', ['run', '--manifest-path', join(DESKTOP, 'Cargo.toml')], { cwd: ROOT, env, stdio: ['ignore', 'inherit', 'inherit'] }));
}


/** Once: a second Ctrl-C while children are still stopping changes nothing (each still gets its grace period). */
async function shutdown(code: number, watchers: readonly FSWatcher[]): Promise<void> {
  if (stopping) return;
  stopping = true;
  say('stopping …');
  for (const w of watchers) w.close();
  await Promise.all([...children].map((c) => stopChild(c)));
  process.exit(code);
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2), process.env);
  if (args.help) return void process.stdout.write(`${USAGE}\n`);
  const home = resolveHome(args.home, homedir(), process.cwd());
  await mkdir(home, { recursive: true });
  const port = await choosePort(args.port, isFree);
  const vitePort = await choosePort(args.vitePort, isFree, [port]);
  if (port !== args.port) say(`port ${args.port} is busy: the server takes ${port}`);
  const run: Run = { port, home, bin: undefined, server: undefined };
  const watchers: FSWatcher[] = [];
  for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP'] as const) process.on(sig, () => void shutdown(0, watchers));

  const [bin] = await Promise.all([buildServer(), buildWebIfMissing()]);
  run.bin = bin;
  if (!run.bin) return void (await shutdown(1, watchers));
  const up = await startServer(run);
  if (stopping) return;
  if (!up) say("wisp serve didn't answer /api/health; Vite still starts (r + Enter retries)");
  const vite = startVite(vitePort, port);
  vite.once('exit', (code) => void (stopping || (say(`vite exited (${code})`), shutdown(1, watchers))));
  if (args.watch && !stopping) watchers.push(...(await watchRust(run)));
  if (!(await waitFor(viteUrl(vitePort), () => vite.exitCode === null && !stopping))) say("vite didn't answer");
  if (stopping) return;
  say(`\x1b[1mopen ${viteUrl(vitePort)}\x1b[0m  (API ${apiUrl(port)}, home ${home}${args.watch ? ', Rust changes rebuild' : ''}; r + Enter rebuilds, Ctrl-C stops)`);

  if (args.desktop) {
    // the shell attaches only to a server that answers; with none it would start its own on our port
    if (!(await waitFor(`${apiUrl(port)}/api/health`, () => !stopping))) return void (await shutdown(1, watchers));
    const app = await startDesktop(run, vitePort);
    app.once('exit', (code) => void (stopping || (say(`desktop exited (${code})`), shutdown(code ?? 0, watchers))));
  }
  if (process.stdin.isTTY) {
    createInterface({ input: process.stdin, terminal: false }).on('line', (l) => {
      const k = l.trim().toLowerCase();
      if (k === 'r') rebuild(run);
      else if (k === 'q') void shutdown(0, watchers);
    });
  }
}

main().catch((e: unknown) => {
  process.stderr.write(`[dev] ${e instanceof DevArgError ? e.message : e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  void shutdown(1, []);
});
