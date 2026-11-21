// size-ok: one measure's whole lifecycle (start, the DevTools client, a cleanup that must never hang) in one place
// What the first screen really loads (ADR-0011's initial budgets, measured as ADR-0105 defines them): the built app,
// served by `vite preview` with the mock peer (apps/web/dev/vite-mock.ts), opened cold in headless Chrome at a desktop
// width and as the phone app (the Capacitor look) at `/?first-screen`, a mode in which the shell starts none of its
// deferred work (the plugins' later parts, `afterFirstScreen`): once it marks `wisp:first-screen` (drawn, the home list
// here, painted: packages/shell/src/shell.tsx whenFirstScreen) and no JS or CSS has been in flight for QUIET_MS, every JS
// and CSS file it fetched is the first screen's, its lazy chains followed to the end.
//
// No dependency: Chrome's DevTools protocol over Node's own WebSocket. Chrome is found at $CHROME_PATH, the macOS app,
// or google-chrome / chromium on the PATH (GitHub's Linux runners have google-chrome).

import { spawn, execFileSync, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export type Look = 'desktop' | 'phone';
export const LOOKS: readonly Look[] = ['desktop', 'phone'];
export interface Loaded {
  readonly js: readonly string[];
  readonly css: readonly string[];
}

/** The JS and CSS among `resources` (paths, in fetch order, with when each started) that started before `mark`. */
export function beforeMark(resources: readonly { readonly path: string; readonly start: number }[], mark: number): Loaded {
  const early = resources.filter((r) => r.start < mark).map((r) => r.path);
  const uniq = (ext: string): string[] => [...new Set(early.filter((p) => p.endsWith(ext)))];
  return { js: uniq('.js'), css: uniq('.css') };
}

function chromePath(): string {
  const env = process.env['CHROME_PATH'];
  if (env && existsSync(env)) return env;
  const mac = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  if (existsSync(mac)) return mac;
  for (const name of ['google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser']) {
    try {
      const p = execFileSync('which', [name], { encoding: 'utf8' }).trim();
      if (p) return p;
    } catch {
      // not this one
    }
  }
  throw new Error('first-screen: no Chrome found (set CHROME_PATH)');
}

/** A minimal DevTools client: one browser WebSocket, flat sessions. */
class Cdp {
  private id = 0;
  private readonly waiting = new Map<number, { ok: (v: unknown) => void; no: (e: Error) => void }>();
  private readonly ws: WebSocket;
  readonly listeners = new Set<(method: string, params: Record<string, unknown>, sessionId?: string) => void>();
  private constructor(ws: WebSocket) {
    this.ws = ws;
    ws.addEventListener('message', (ev) => {
      const msg = JSON.parse(String(ev.data)) as { id?: number; result?: unknown; error?: { message: string }; method?: string; params?: Record<string, unknown>; sessionId?: string };
      if (msg.id === undefined) {
        if (msg.method) for (const f of this.listeners) f(msg.method, msg.params ?? {}, msg.sessionId);
        return;
      }
      const w = this.waiting.get(msg.id);
      if (!w) return;
      this.waiting.delete(msg.id);
      if (msg.error) w.no(new Error(msg.error.message));
      else w.ok(msg.result);
    });
    // a closed socket answers nothing more: whatever waits on it fails at once instead of forever
    ws.addEventListener('close', () => {
      for (const w of this.waiting.values()) w.no(new Error('first-screen: the DevTools socket closed'));
      this.waiting.clear();
    });
  }
  static async open(url: string): Promise<Cdp> {
    const ws = new WebSocket(url);
    await new Promise<void>((ok, no) => {
      ws.addEventListener('open', () => ok(), { once: true });
      ws.addEventListener('error', () => no(new Error('first-screen: DevTools socket failed')), { once: true });
    });
    return new Cdp(ws);
  }
  send<T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}, sessionId?: string): Promise<T> {
    if (this.ws.readyState !== WebSocket.OPEN) return Promise.reject(new Error('first-screen: the DevTools socket closed'));
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params, ...(sessionId && { sessionId }) }));
    return new Promise<T>((ok, no) => this.waiting.set(id, { ok: ok as (v: unknown) => void, no }));
  }
  close(): void {
    this.ws.close();
  }
}

/** A browser started for one measure: its DevTools socket once it's up, and `stop`. */
export interface Browser {
  /** the main process's pid (its process group's id on Unix), when it started at all */
  readonly pid: number | undefined;
  /** the profile folder (removed by `stop`) */
  readonly dir: string;
  /** the DevTools WebSocket URL, once Chrome says it's listening; rejects if it exits or doesn't start in time */
  readonly ready: Promise<string>;
  /** Ends every process of it and only then removes its profile: resolves once all are gone and the folder with them. */
  stop(): Promise<void>;
}

const sleep = (ms: number): Promise<void> => new Promise((ok) => setTimeout(ok, ms));

/** `p`, or a rejection naming `what` after `ms`: no cleanup step may hang the script. */
async function bounded<T>(p: Promise<T>, ms: number, what: string): Promise<T> {
  let t: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([p, new Promise<never>((_, no) => (t = setTimeout(() => no(new Error(`first-screen: ${what} took over ${ms / 1000} s`)), ms)))]);
  } finally {
    clearTimeout(t);
  }
}

/** Whether any process of group `pgid` is left (a zombie not yet reaped counts: it's not gone). */
function groupAlive(pgid: number): boolean {
  try {
    process.kill(-pgid, 0);
    return true;
  } catch (err) {
    return (err as NodeJS.ErrnoException).code === 'EPERM';
  }
}

function signalGroup(pgid: number, sig: NodeJS.Signals): void {
  try {
    process.kill(-pgid, sig);
  } catch {
    // already gone
  }
}

/** SIGTERM to group `pgid`, SIGKILL to what's left after 5 s; resolves once no process of it is left. */
async function endGroup(pgid: number, dir: string): Promise<void> {
  signalGroup(pgid, 'SIGTERM');
  for (const sig of ['SIGKILL', undefined] as const) {
    const by = Date.now() + 5_000;
    while (groupAlive(pgid) && Date.now() < by) await sleep(25);
    if (!groupAlive(pgid)) return;
    if (!sig) throw new Error(`first-screen: Chrome's processes (group ${pgid}) didn't exit; its profile is left at ${dir}`);
    signalGroup(pgid, sig);
  }
}

/** The flags of a headless measuring Chrome: no crash reporter, sync, component updates or background networking, so
 *  nothing else writes into its profile while it shuts down. */
function chromeArgs(dir: string): string[] {
  // a CI Linux runner (Ubuntu 24.04's AppArmor) may not give Chrome its sandbox
  const sandbox = process.platform === 'linux' && process.env['CI'] ? ['--no-sandbox'] : [];
  return [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${dir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-gpu',
    '--disable-extensions',
    '--disable-crash-reporter',
    '--disable-breakpad',
    '--disable-background-networking',
    '--disable-component-update',
    '--disable-sync',
    ...sandbox,
    'about:blank',
  ];
}

/** Starts Chrome headless on a fresh profile (`bin`/`prefix`: another program in its place, the tests' fake browser).
 *
 *  CI run 37dca4f8 hung 60 minutes here: the old `stop` waited for Chrome's main process only, then removed the profile,
 *  while its helpers (same process group, still shutting down) were writing into `Default/`: ENOTEMPTY, thrown from the
 *  `finally` before the preview server was closed, which then kept Node alive until the job was cancelled. Now Chrome
 *  gets a process group of its own, `stop` ends the group and waits until no process of it is left, and only then
 *  removes the folder; a script that exits any other way takes the group down with it (`process.on('exit')`). */
export function startBrowser(opts: { bin?: string; prefix?: readonly string[]; startMs?: number } = {}): Browser {
  const dir = mkdtempSync(join(tmpdir(), 'wisp-first-screen-'));
  const unix = process.platform !== 'win32';
  let proc: ChildProcess;
  try {
    proc = spawn(opts.bin ?? chromePath(), [...(opts.prefix ?? []), ...chromeArgs(dir)], { stdio: ['ignore', 'ignore', 'pipe'], detached: unix });
  } catch (err) {
    rmSync(dir, { recursive: true, force: true });
    throw err;
  }
  const pid = proc.pid;
  const exited = new Promise<void>((ok) => {
    proc.once('exit', () => ok());
    proc.once('error', () => ok());
  });
  // the script ending any other way (an uncaught error, process.exit) leaves no Chrome behind
  const killNow = (): void => {
    if (pid === undefined) return;
    if (unix) signalGroup(pid, 'SIGKILL');
    else proc.kill('SIGKILL');
  };
  process.on('exit', killNow);
  const ready = new Promise<string>((ok, no) => {
    let err = '';
    const t = setTimeout(() => no(new Error(`first-screen: Chrome didn't start in ${(opts.startMs ?? 30_000) / 1000} s\n${err}`)), opts.startMs ?? 30_000);
    proc.stderr?.on('data', (b: Buffer) => {
      err += b.toString();
      const m = /DevTools listening on (ws:\/\/\S+)/.exec(err);
      if (m?.[1]) {
        clearTimeout(t);
        ok(m[1]);
      }
    });
    proc.once('error', (e) => {
      clearTimeout(t);
      no(new Error(`first-screen: Chrome couldn't start: ${e.message}`));
    });
    proc.once('exit', (code, sig) => {
      clearTimeout(t);
      no(new Error(`first-screen: Chrome exited (${sig ?? `code ${code}`})\n${err}`));
    });
  });
  // a rejection nobody awaited (stopped before it was up) is not an unhandled one
  ready.catch(() => undefined);
  let stopping: Promise<void> | undefined;
  const stop = (): Promise<void> =>
    (stopping ??= (async () => {
      if (pid !== undefined && unix) {
        await endGroup(pid, dir);
      } else if (pid !== undefined && proc.exitCode === null && proc.signalCode === null) {
        // Windows: the whole tree, then its exit
        spawn('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore' }).once('error', () => proc.kill('SIGKILL'));
      }
      await bounded(exited, 10_000, "Chrome's exit");
      process.off('exit', killNow);
      // every process of it is gone: nothing writes into the folder any more
      await rm(dir, { recursive: true, force: true });
    })());
  return { pid, dir, ready, stop };
}

/** no JS or CSS in flight this long after the first-screen mark: its lazy chains are done (a chunk's next import
 *  comes a render after it lands; a loaded CI runner gets the margin) */
const QUIET_MS = 1_000;

const PHONE_BRIDGE = `window.Capacitor = { isNativePlatform: () => true, getPlatform: () => 'android', Plugins: {}, registerPlugin: () => new Proxy({}, { get: () => async () => ({}) }) };`;

/** One cold open of `url` (with `?first-screen`: the shell starts no later part) in `look`: every JS and CSS file it
 *  fetches, once it has marked its first screen and no JS or CSS has been in flight for QUIET_MS (its lazy chains run
 *  to the end). */
async function openOnce(cdp: Cdp, url: string, look: Look): Promise<Loaded> {
  const { browserContextId } = await cdp.send<{ browserContextId: string }>('Target.createBrowserContext');
  const { targetId } = await cdp.send<{ targetId: string }>('Target.createTarget', { url: 'about:blank', browserContextId });
  const { sessionId } = await cdp.send<{ sessionId: string }>('Target.attachToTarget', { targetId, flatten: true });
  const s = (m: string, p: Record<string, unknown> = {}): Promise<Record<string, unknown>> => cdp.send(m, p, sessionId);
  // the JS and CSS requests in flight, and when the last one ended
  const asset = (u: unknown): boolean => typeof u === 'string' && /\.(?:js|css)(?:\?|$)/.test(new URL(u).pathname);
  const flying = new Set<string>();
  let quietSince = Date.now();
  const on = (method: string, p: Record<string, unknown>, sid?: string): void => {
    if (sid !== sessionId) return;
    const id = String(p['requestId'] ?? '');
    if (method === 'Network.requestWillBeSent' && asset((p['request'] as { url?: string } | undefined)?.url)) flying.add(id);
    else if ((method === 'Network.loadingFinished' || method === 'Network.loadingFailed') && flying.delete(id) && flying.size === 0) quietSince = Date.now();
  };
  cdp.listeners.add(on);
  try {
    await s('Network.enable');
    await s('Page.enable');
    await s('Runtime.enable');
    await s('Page.addScriptToEvaluateOnNewDocument', { source: `performance.setResourceTimingBufferSize(2000);${look === 'phone' ? PHONE_BRIDGE : ''}` });
    const phone = look === 'phone';
    await s('Emulation.setDeviceMetricsOverride', phone ? { width: 390, height: 844, deviceScaleFactor: 3, mobile: true } : { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
    if (phone) await s('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
    await s('Page.navigate', { url: `${url}?first-screen` });
    const probe = `(() => { const m = performance.getEntriesByName('wisp:first-screen')[0]; if (!m) return null; return JSON.stringify(performance.getEntriesByType('resource').map((r) => ({ path: new URL(r.name).pathname, start: r.startTime }))); })()`;
    const end = Date.now() + 60_000;
    while (Date.now() < end) {
      const marked = (await s('Runtime.evaluate', { expression: `performance.getEntriesByName('wisp:first-screen').length > 0`, returnByValue: true })) as { result?: { value?: boolean } };
      if (marked.result?.value && flying.size === 0 && Date.now() - quietSince >= QUIET_MS) {
        const r = (await s('Runtime.evaluate', { expression: probe, returnByValue: true })) as { result?: { value?: string | null } };
        const v = r.result?.value;
        // in `?first-screen` nothing deferred starts: everything fetched is the first screen's, to the end of its chains
        if (v) return beforeMark(JSON.parse(v) as { path: string; start: number }[], Number.POSITIVE_INFINITY);
      }
      await new Promise((ok) => setTimeout(ok, 50));
    }
    throw new Error(`first-screen: ${look} never marked wisp:first-screen at ${url}`);
  } finally {
    cdp.listeners.delete(on);
    await cdp.send('Target.closeTarget', { targetId }).catch(() => undefined);
    await cdp.send('Target.disposeBrowserContext', { browserContextId }).catch(() => undefined);
  }
}

/** The whole measure's limit (CI's job is 60 minutes; a measure takes ~10 s): past it, the browser and the preview
 *  server are stopped and the measure fails, saying so. */
export const FIRST_SCREEN_DEADLINE_MS = 5 * 60_000;

/** What serves the built app: its URL, and how to stop it. */
export interface Served {
  readonly url: string;
  close(): Promise<void>;
}

export interface FirstScreenOpts {
  /** FIRST_SCREEN_DEADLINE_MS by default */
  readonly deadlineMs?: number;
  /** another program in Chrome's place (the tests' fake browser) */
  readonly browser?: { readonly bin: string; readonly prefix?: readonly string[] };
  /** another server in vite preview's place (the tests') */
  readonly serve?: () => Promise<Served>;
}

/** `vite preview` of `dist` with the mock peer. */
async function preview(dist: string, root: string): Promise<Served> {
  // vite is the web app's own dev dependency: resolved from there
  const vite = createRequire(join(root, 'package.json'))('vite') as typeof import('vite');
  const server = await vite.preview({ root, configFile: join(root, 'vite.config.ts'), logLevel: 'error', build: { outDir: dist }, preview: { port: 0, host: '127.0.0.1', strictPort: false, open: false } });
  const addr = server.httpServer.address();
  return {
    url: `http://127.0.0.1:${typeof addr === 'object' && addr ? addr.port : 0}/`,
    close: async () => {
      // a socket Chrome left open must not hold the close
      (server.httpServer as { closeAllConnections?: () => void }).closeAllConnections?.();
      await server.close();
    },
  };
}

/** The cleanup, in order: Chrome asked to close (it ends its helpers itself), then every process of it waited out and
 *  its profile removed, then the server. Each step runs whatever the one before did, within its own limit; what went
 *  wrong, in words. */
async function cleanUp(cdp: Cdp | undefined, chrome: Browser | undefined, served: Promise<Served>): Promise<string[]> {
  const problems: string[] = [];
  const step = async (what: string, f: () => Promise<unknown>, ms: number): Promise<void> => {
    try {
      await bounded(f(), ms, what);
    } catch (err) {
      problems.push(`${what}: ${err instanceof Error ? err.message : String(err)}`);
    }
  };
  if (cdp) {
    // Browser.close's answer may never come (the browser is gone): its own short wait, and no problem if it doesn't
    await bounded(cdp.send('Browser.close'), 2_000, 'Browser.close').catch(() => undefined);
    cdp.close();
  }
  if (chrome) await step('stopping Chrome', () => chrome.stop(), 25_000);
  await step('closing the preview server', () => served.then((s) => s.close()), 5_000);
  return problems;
}

/** The built app at `dist`, served with the mock peer, opened in each look. Whatever happens (a look that never marks,
 *  a socket that fails, the deadline), the browser, its processes and profile, and the server are all stopped before
 *  this settles, each on its own (one failing to stop doesn't skip the next), each within seconds. */
export async function firstScreen(dist: string, root = join(import.meta.dirname, '..', 'apps', 'web'), opts: FirstScreenOpts = {}): Promise<Record<Look, Loaded>> {
  const deadlineMs = opts.deadlineMs ?? FIRST_SCREEN_DEADLINE_MS;
  const served = (opts.serve ?? (() => preview(dist, root)))();
  served.catch(() => undefined);
  let chrome: Browser | undefined;
  let cdp: Cdp | undefined;
  let stopped = false;
  const work = async (): Promise<Record<Look, Loaded>> => {
    const { url } = await served;
    if (stopped) throw new Error('first-screen: stopped');
    chrome = startBrowser(opts.browser ?? {});
    const c = await Cdp.open(await chrome.ready);
    // the deadline passed while it opened: the cleanup already ran without it
    if (stopped) {
      c.close();
      throw new Error('first-screen: stopped');
    }
    cdp = c;
    const out = {} as Record<Look, Loaded>;
    for (const look of LOOKS) out[look] = await openOnce(c, url, look);
    return out;
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, no) => {
    timer = setTimeout(() => no(new Error(`first-screen: not measured in ${deadlineMs / 1000} s (the browser and the preview server were stopped)`)), deadlineMs);
  });
  let failed: unknown;
  let out: Record<Look, Loaded> | undefined;
  const working = work();
  // past the deadline it may still fail (its socket closed under it): nobody's waiting for that any more
  working.catch(() => undefined);
  try {
    out = await Promise.race([working, deadline]);
  } catch (err) {
    failed = err;
  } finally {
    clearTimeout(timer);
    stopped = true;
  }
  const problems = await cleanUp(cdp, chrome, served);
  if (failed !== undefined) {
    const why = failed instanceof Error ? failed.message : String(failed);
    throw new Error(problems.length ? `${why}\n(and ${problems.join('; ')})` : why);
  }
  if (problems.length || !out) throw new Error(problems.join('; ') || 'first-screen: nothing measured');
  return out;
}

if (import.meta.filename === process.argv[1]) {
  const dist = process.argv.find((a) => a.startsWith('--dist='))?.slice(7) ?? join(import.meta.dirname, '..', 'apps', 'web', 'dist');
  const r = await firstScreen(dist);
  console.log(JSON.stringify(r, null, 1));
}
