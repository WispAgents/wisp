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
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params, ...(sessionId && { sessionId }) }));
    return new Promise<T>((ok, no) => this.waiting.set(id, { ok: ok as (v: unknown) => void, no }));
  }
  close(): void {
    this.ws.close();
  }
}

/** Chrome headless, its DevTools socket and its profile dir; `stop` ends it (and only resolves once it's gone). */
async function launch(): Promise<{ proc: ChildProcess; ws: string; dir: string; stop: () => Promise<void> }> {
  const dir = mkdtempSync(join(tmpdir(), 'wisp-first-screen-'));
  // a CI Linux runner (Ubuntu 24.04's AppArmor) may not give Chrome its sandbox
  const sandbox = process.platform === 'linux' && process.env['CI'] ? ['--no-sandbox'] : [];
  const proc = spawn(chromePath(), ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${dir}`, '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--disable-extensions', ...sandbox, 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  const stop = async (): Promise<void> => {
    if (proc.exitCode === null && proc.signalCode === null) {
      const gone = new Promise<void>((ok) => proc.once('exit', () => ok()));
      proc.kill();
      const hard = setTimeout(() => proc.kill('SIGKILL'), 5_000);
      await gone;
      clearTimeout(hard);
    }
    rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  };
  try {
    const ws = await new Promise<string>((ok, no) => {
      let err = '';
      const t = setTimeout(() => no(new Error(`first-screen: Chrome didn't start\n${err}`)), 30_000);
      proc.stderr?.on('data', (b: Buffer) => {
        err += b.toString();
        const m = /DevTools listening on (ws:\/\/\S+)/.exec(err);
        if (m?.[1]) {
          clearTimeout(t);
          ok(m[1]);
        }
      });
      proc.on('exit', () => {
        clearTimeout(t);
        no(new Error(`first-screen: Chrome exited\n${err}`));
      });
    });
    return { proc, ws, dir, stop };
  } catch (err) {
    await stop();
    throw err;
  }
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

/** The built app at `dist`, served with the mock peer, opened in each look. */
export async function firstScreen(dist: string, root = join(import.meta.dirname, '..', 'apps', 'web')): Promise<Record<Look, Loaded>> {
  // vite is the web app's own dev dependency: resolved from there
  const vite = createRequire(join(root, 'package.json'))('vite') as typeof import('vite');
  const server = await vite.preview({ root, configFile: join(root, 'vite.config.ts'), logLevel: 'error', build: { outDir: dist }, preview: { port: 0, host: '127.0.0.1', strictPort: false, open: false } });
  let chrome: Awaited<ReturnType<typeof launch>> | undefined;
  let cdp: Cdp | undefined;
  try {
    const addr = server.httpServer.address();
    const url = `http://127.0.0.1:${typeof addr === 'object' && addr ? addr.port : 0}/`;
    chrome = await launch();
    cdp = await Cdp.open(chrome.ws);
    const out = {} as Record<Look, Loaded>;
    for (const look of LOOKS) out[look] = await openOnce(cdp, url, look);
    return out;
  } finally {
    cdp?.close();
    await chrome?.stop();
    await server.close();
  }
}

if (import.meta.filename === process.argv[1]) {
  const dist = process.argv.find((a) => a.startsWith('--dist='))?.slice(7) ?? join(import.meta.dirname, '..', 'apps', 'web', 'dist');
  const r = await firstScreen(dist);
  console.log(JSON.stringify(r, null, 1));
}
