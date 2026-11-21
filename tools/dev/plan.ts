// tools/dev.ts's pure parts: the arguments, which ports and home a dev run may use, the env each child gets, and
// which file changes rebuild the Rust server. No I/O here (tools/test/dev.test.ts).

import { join, posix, sep, win32 } from 'node:path';

export const DEFAULT_PORT = 4800;
export const DEFAULT_VITE_PORT = 5180;
/** An installed wisp (v1 and v2's default 4477, the grid/preview 4791): a dev run never listens there or talks to it. */
export const RESERVED_PORTS: readonly number[] = [4477, 4791];
/** Homes a dev run must never write to: the installed wisp's and the v2 preview's. */
export const RESERVED_HOMES: readonly string[] = ['.wisp', '.wisp-v2-preview'];
export const DEV_HOME = '.wisp-dev';
/** How many ports past the asked one a busy port falls back through. */
export const PORT_TRIES = 50;
/** The desktop dev build's bundle identifier: its own single-instance lock and data dir, never dev.wisp.app's. */
export const DESKTOP_DEV_IDENTIFIER = 'dev.wisp.dev';

export interface DevArgs {
  desktop: boolean;
  port: number;
  vitePort: number;
  /** The wisp home for the dev server; undefined = ~/.wisp-dev. */
  home: string | undefined;
  /** Rebuild and restart the server when crates/ or plugins/x/host change. */
  watch: boolean;
  help: boolean;
}

export class DevArgError extends Error {}

function portArg(name: string, v: string | undefined): number {
  const n = Number(v);
  if (!v || !Number.isInteger(n) || n < 1 || n > 65535) throw new DevArgError(`${name} needs a port (1-65535), got ${v ?? 'nothing'}`);
  if (RESERVED_PORTS.includes(n)) throw new DevArgError(`${name} ${n} is an installed wisp's port; pick another`);
  return n;
}

/** `node tools/dev.ts [--desktop] [--port N] [--vite-port N] [--home DIR] [--no-watch]`; env WISP_DEV_PORT / WISP_DEV_HOME too. */
export function parseArgs(argv: readonly string[], env: Readonly<Record<string, string | undefined>> = {}): DevArgs {
  const out: DevArgs = { desktop: false, port: DEFAULT_PORT, vitePort: DEFAULT_VITE_PORT, home: undefined, watch: true, help: false };
  if (env['WISP_DEV_PORT']) out.port = portArg('WISP_DEV_PORT', env['WISP_DEV_PORT']);
  if (env['WISP_DEV_HOME']) out.home = env['WISP_DEV_HOME'];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] ?? '';
    const [flag, inline] = a.startsWith('--') && a.includes('=') ? [a.slice(0, a.indexOf('=')), a.slice(a.indexOf('=') + 1)] : [a, undefined];
    const value = (): string | undefined => inline ?? argv[++i];
    switch (flag) {
      case '--desktop': out.desktop = true; break;
      case '--port': out.port = portArg('--port', value()); break;
      case '--vite-port': out.vitePort = portArg('--vite-port', value()); break;
      case '--home': {
        const h = value();
        if (!h) throw new DevArgError('--home needs a folder');
        out.home = h;
        break;
      }
      case '--no-watch': out.watch = false; break;
      case '-h': case '--help': out.help = true; break;
      default: throw new DevArgError(`unknown argument ${a} (try --help)`);
    }
  }
  return out;
}

export const USAGE = `npm run dev [-- options]          the wisp server from this checkout + Vite with HMR
npm run dev:desktop [-- options]  the same, plus the Tauri window on Vite's URL

  --port N        the dev server's port (default ${DEFAULT_PORT}, WISP_DEV_PORT; the next free one if busy)
  --vite-port N   Vite's port (default ${DEFAULT_VITE_PORT}; the next free one if busy)
  --home DIR      the dev server's wisp home (default ~/${DEV_HOME}, WISP_DEV_HOME; never ~/.wisp)
  --no-watch      don't rebuild on Rust changes (press r + Enter to rebuild and restart)

Without apps/web/dist it builds the web app once first, so pages that reach the server itself (the grid tunnel to
this computer) get a UI; run \`npm run build\` to refresh that copy. Vite's page always hot reloads.

Keys: r + Enter rebuilds and restarts the server, q + Enter (or Ctrl-C) stops everything.`;

/** The ports to try, in order: the asked one, then the next ones, never an installed wisp's. */
export function portCandidates(start: number, tries = PORT_TRIES): number[] {
  const out: number[] = [];
  for (let p = start; p <= 65535 && out.length < tries; p++) if (!RESERVED_PORTS.includes(p)) out.push(p);
  return out;
}

/** The first candidate `free` says yes to; `taken` holds ports this run already handed out. */
export async function choosePort(start: number, free: (port: number) => Promise<boolean>, taken: readonly number[] = []): Promise<number> {
  for (const p of portCandidates(start)) if (!taken.includes(p) && (await free(p))) return p;
  throw new DevArgError(`no free port from ${start} (${PORT_TRIES} tried)`);
}

/** The dev home, absolute; refuses an installed wisp's home (or anything inside it). Case-insensitive where the file
 * system usually is (macOS, Windows): ~/.WISP is ~/.wisp there. Paths are `platform`'s (win32: `C:\Users\u`, `~\x`
 * too), so every OS's rules are testable on any OS. */
export function resolveHome(asked: string | undefined, userHome: string, cwd: string, platform: string = process.platform): string {
  const path = platform === 'win32' ? win32 : posix;
  const tilde = platform === 'win32' ? /^~[\\/]/ : /^~\//;
  const expand = (a: string): string => (a === '~' ? userHome : tilde.test(a) ? path.join(userHome, a.slice(2)) : a);
  const home = asked === undefined ? path.join(userHome, DEV_HOME) : path.resolve(cwd, expand(asked));
  const fold = platform === 'darwin' || platform === 'win32' ? (p: string) => p.toLowerCase() : (p: string) => p;
  for (const r of RESERVED_HOMES) {
    const rel = path.relative(fold(path.join(userHome, r)), fold(home));
    const outside = rel === '..' || rel.startsWith(`..${path.sep}`) || path.isAbsolute(rel);
    if (!outside) throw new DevArgError(`${home} is an installed wisp's home (~/${r}); a dev run never touches it`);
  }
  return home;
}

type Env = Record<string, string | undefined>;

/** `wisp serve`'s env: its own port and home, the page from Vite (so no WISP_WEB_DIR), and it exits when dev.ts goes. */
export function serverEnv(base: Readonly<Env>, port: number, home: string): Env {
  const env: Env = { ...base, WISP_PORT: String(port), WISP_HOME: home, WISP_EXIT_WITH_STDIN: '1' };
  for (const k of ['WISP_WEB_DIR', 'WISP_DESKTOP', 'WISP_DESKTOP_DATA', 'WISP_BOX']) delete env[k];
  return env;
}

/** The web build a debug `wisp serve` falls back to (crates/wisp/src/serve.rs `web_dir()`: CARGO_MANIFEST_DIR/../../apps/web/dist).
 * Pages that skip Vite need it: the grid tunnel forwards `/` to the server's port, not Vite's. */
export function webIndex(root: string): string {
  return join(root, 'apps', 'web', 'dist', 'index.html');
}

export function apiUrl(port: number): string {
  return `http://127.0.0.1:${port}`;
}

export function viteUrl(port: number): string {
  return `http://localhost:${port}`;
}

/** Vite's env: the real server instead of the mock peer (apps/web/vite.config.ts reads WISP_DEV_API). */
export function viteEnv(base: Readonly<Env>, vitePort: number, apiPort: number): Env {
  return { ...base, PORT: String(vitePort), WISP_MOCK: '0', WISP_DEV_API: apiUrl(apiPort) };
}

/** The Tauri dev build's config overlay (desktop/src-tauri/tauri.dev.conf.json), as TAURI_CONFIG takes it. */
export interface DesktopOverlay {
  identifier: string;
  productName?: string;
}

/** The desktop shell's env: attach to the dev server's port (never 4477), load Vite's page, a dev identifier. */
export function desktopEnv(base: Readonly<Env>, apiPort: number, vitePort: number, home: string, overlay: DesktopOverlay, targetDir: string, serverBin: string): Env {
  if (overlay.identifier !== DESKTOP_DEV_IDENTIFIER) throw new DevArgError(`the desktop dev identifier must be ${DESKTOP_DEV_IDENTIFIER}, got ${overlay.identifier}`);
  const env: Env = {
    ...base,
    WISP_PORT: String(apiPort),
    WISP_HOME: home,
    WISP_DEV_URL: viteUrl(vitePort),
    // should it ever find the port empty (a restart just then), it starts this build's server, not a stale one
    WISP_BIN: serverBin,
    TAURI_CONFIG: JSON.stringify(overlay),
    // Always its own target dir: the shell's binary is also called `wisp` (it would overwrite the server's), and the
    // dev identifier is compiled in (tauri-build reads TAURI_CONFIG), so a shared one would rebuild both ways.
    CARGO_TARGET_DIR: targetDir,
  };
  return env;
}

/** Does a changed path (relative to the repo) need the server rebuilt? Rust sources and manifests under crates/ and
 * plugins/x/host, and the workspace's Cargo files; never target/ or node_modules. */
export function isRustChange(rel: string): boolean {
  const p = rel.split(sep).join('/');
  if (/(^|\/)(target|node_modules|\.git)\//.test(p)) return false;
  if (p === 'Cargo.toml' || p === 'Cargo.lock') return true;
  const inScope = p.startsWith('crates/') || /^plugins\/[^/]+\/host\//.test(p);
  return inScope && /(\.rs|Cargo\.toml|\.json|\.sql)$/.test(p);
}

/** The `wisp` binary out of `cargo build --message-format=json` lines; undefined when none was built. */
export function builtBinary(lines: readonly string[], name = 'wisp'): string | undefined {
  let found: string | undefined;
  for (const line of lines) {
    if (!line.startsWith('{')) continue;
    try {
      const m = JSON.parse(line) as { reason?: string; target?: { name?: string; kind?: string[] }; executable?: string | null };
      if (m.reason === 'compiler-artifact' && m.target?.name === name && m.target.kind?.includes('bin') && m.executable) found = m.executable;
    } catch {
      // a line that only looks like JSON: not cargo's
    }
  }
  return found;
}
