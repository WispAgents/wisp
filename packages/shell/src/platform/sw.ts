// The service worker (v1 web/sw.js; built by apps/web into dist/sw.js, for the phone's browser and the installed web app). The page and
// its content-named files come from this cache at once; the network is asked behind them.
//   - install: every module in one request (`/_pack` → {url: text}, crates/wisp-api statics/pack.rs),
//     not ~40 round trips over a slow tunnel, then the page itself (`x-wisp-sw: 1`: the normal page,
//     not the phone's boot page that waits for this worker);
//   - navigations: the cached page, then a check behind it: a page naming another build (x-wisp-build)
//     brings that build's modules (one /_pack, minus what's here), replaces the page and tells the
//     tabs, which offer Reload. Two builds' files never mix: their names carry their hashes. The new page is kept
//     only once its modules are (a failed /_pack keeps the old page, and the next look tries again: a page whose
//     modules aren't here starts blank offline). Files of the builds before the last are dropped then: the last
//     one stays, as a tab may still run it (the navigation that found the new build was answered with it);
//   - a tab's `{t:'check'}` (worker.ts: back in view, resumed, its stream live again) runs that same look without a
//     navigation, at most one at a time and one per half minute: an open or resumed app learns of a new build;
//   - a computer that isn't connected (its grid's 503 names its waiting page, `x-wisp-offline`): the tabs are sent
//     to that page, never kept on the cached app (QA round 10 Grid #2);
//   - the installed app's share target (manifest.webmanifest share_target): a POST /share is kept, capped
//     (share-form.ts), in its own cache under a random id, and answered with /?shared=<id>, which the page reads
//     once (apps/web shared.ts). Every other non-GET request goes by untouched;
//   - a tapped notification (browser.ts shows them through this worker): an open tab is focused and told where
//     to go (`{t:'open', url}`), else a window opens there;
//   - never cached: /api/ (the sync stream, commands), /_grid/ (the grid's sign-in, waking and waiting pages),
//     /__mock/, /sw.js itself.

import { CACHE as SHARED, keep } from './share-form.ts';
import { offlineWaiting, waitingFor } from './sw-offline.ts';

interface FetchEventLike extends Event {
  readonly request: Request;
  readonly resultingClientId?: string;
  respondWith(r: Promise<Response> | Response): void;
  waitUntil(p: Promise<unknown>): void;
}
interface ExtendableMessageLike extends Event {
  readonly data: unknown;
  waitUntil(p: Promise<unknown>): void;
}
interface ClientLike {
  readonly url?: string;
  postMessage(m: unknown): void;
  navigate?(url: string): Promise<unknown>;
  focus?(): Promise<unknown>;
}
interface NotificationEventLike extends Event {
  readonly notification: { readonly data?: unknown; close(): void };
  waitUntil(p: Promise<unknown>): void;
}
interface WorkerScope {
  readonly location: { readonly origin: string };
  readonly clients: {
    claim(): Promise<void>;
    matchAll(o?: { type?: string; includeUncontrolled?: boolean }): Promise<readonly ClientLike[]>;
    get(id: string): Promise<ClientLike | undefined>;
    openWindow?(url: string): Promise<unknown>;
  };
  skipWaiting(): Promise<void>;
  addEventListener(type: 'install' | 'activate' | 'fetch', fn: (e: FetchEventLike) => void): void;
  addEventListener(type: 'message', fn: (e: ExtendableMessageLike) => void): void;
  addEventListener(type: 'notificationclick', fn: (e: NotificationEventLike) => void): void;
}

const sw = globalThis as unknown as WorkerScope;
const CACHE = 'wisp-v2';
const BUILD = 'x-wisp-build';
/** the files the cached page's build uses, as a JSON list (under /_pack, which is never served from here) */
const KEPT = '/_pack?kept';
const MIME: Record<string, string> = { js: 'text/javascript; charset=utf-8', css: 'text/css; charset=utf-8', json: 'application/json' };

const skip = (url: URL): boolean =>
  url.origin !== sw.location.origin || ['/api/', '/_grid/', '/__mock/', '/@'].some((p) => url.pathname.startsWith(p)) || url.pathname === '/sw.js' || url.pathname === '/_pack';

async function put(cache: Cache, files: Record<string, string>): Promise<void> {
  await Promise.all(
    Object.entries(files).map(([url, text]) => {
      const ext = url.split('?')[0]?.split('.').pop() ?? '';
      return cache.put(url, new Response(text, { headers: { 'content-type': MIME[ext] ?? 'text/plain' } }));
    }),
  );
}

/** The /assets/ files a page names (its entry script, modulepreloads, stylesheets). Not exported: sw.js is a classic
 *  worker script, where an `export` is a syntax error. */
function namedAssets(html: string): Set<string> {
  const out = new Set<string>();
  for (const m of html.matchAll(/\b(?:src|href)=["']?((?:https?:\/\/[^/"'\s>]+)?\/assets\/[^"'\s>]+)/g)) {
    const url = m[1] ?? '';
    out.add(url.startsWith('/') ? url : new URL(url).pathname);
  }
  return out;
}

/** The files the pack and the page tag manage: a build's modules and stylesheets (fonts and images, which a
 *  stylesheet names, stay). */
const built = (path: string): boolean => /\.(?:m?js|css)$/.test(path);

/** Brings the page (and, when its build is new, its modules) into the cache; `{ offline }`: the computer isn't
 *  connected, and that is its waiting page. */
async function refresh(cache: Cache): Promise<'same' | 'new' | 'failed' | { readonly offline: string }> {
  const old = await cache.match('/');
  const page = await fetch('/', { headers: { 'x-wisp-sw': '1' }, cache: 'no-store' }).catch(() => undefined);
  if (!page?.ok) {
    const offline = offlineWaiting(page);
    return offline === null ? 'failed' : { offline };
  }
  const build = page.headers.get(BUILD);
  if (old && build !== null && old.headers.get(BUILD) === build) return 'same';
  const named = namedAssets(await page.clone().text().catch(() => ''));
  const before = await cache
    .match(KEPT)
    .then((r) => (r ? (r.json() as Promise<string[]>) : undefined))
    .catch(() => undefined);
  // only what the new page names counts as had: the rest is the old build's, about to go, so `have` (and the
  // query string) stays about one build long, and a module both builds share but the page doesn't name comes again
  const cached = (await cache.keys()).map((r) => new URL(r.url).pathname).filter((p) => p.startsWith('/assets/'));
  const have = cached.filter((p) => named.has(p) && /\.m?js$/.test(p));
  let files: Record<string, string>;
  try {
    const pack = await fetch(`/_pack${have.length ? `?have=${encodeURIComponent(have.join(','))}` : ''}`);
    if (!pack.ok) return 'failed';
    files = (await pack.json()) as Record<string, string>;
    await put(cache, files);
  } catch {
    return 'failed';
  }
  await cache.put('/', page);
  const keep = new Set([...Object.keys(files).map((u) => new URL(u, sw.location.origin).pathname), ...have, ...named]);
  await cache.put(KEPT, new Response(JSON.stringify([...keep]), { headers: { 'content-type': MIME['json'] ?? '' } })).catch(() => {});
  // what neither this build nor the one before uses goes (a cache from before this list: nothing yet, next time)
  if (before) {
    const last = new Set(before);
    await Promise.all(cached.filter((p) => built(p) && !keep.has(p) && !last.has(p)).map((p) => cache.delete(p))).catch(() => {});
  }
  return old ? 'new' : 'same';
}

async function tell(message: unknown): Promise<void> {
  for (const c of await sw.clients.matchAll({ type: 'window' })) c.postMessage(message);
}

/** Until when navigations go to the network, not the cache: a computer found offline, so the grid sends them on to its
 *  waiting page (a tab told so reloads). */
const offline = { until: 0 };

/** The tab this navigation opens, sent to `to` once it exists (it doesn't yet while its page is answered), else every
 *  tab is told to reload (worker.ts), which the network answers for the next few seconds. */
async function sendTo(id: string | undefined, to: string): Promise<void> {
  const url = new URL(to, sw.location.origin).href;
  for (let n = 0; id && n < 20; n++) {
    const c = await sw.clients.get(id);
    if (c?.navigate) {
      await c.navigate(url).catch(() => {});
      return;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  offline.until = Date.now() + 10_000;
  await tell({ t: 'offline' });
}

async function navigate(e: FetchEventLike): Promise<Response> {
  if (Date.now() < offline.until) return fetch(e.request);
  const cache = await caches.open(CACHE);
  const cached = await cache.match('/');
  const check = refresh(cache);
  if (cached) {
    e.waitUntil(
      check
        .then((r) => (r === 'new' ? tell({ t: 'update' }) : typeof r === 'object' ? sendTo(e.resultingClientId, waitingFor(e.request.url)) : undefined))
        .catch(() => {}),
    );
    return cached;
  }
  const r = await check.catch(() => 'failed' as const);
  // offline: the page itself asked for, which the grid sends on to its waiting page (for this very page)
  if (typeof r === 'object') return fetch(e.request);
  return (await cache.match('/')) ?? fetch('/', { headers: { 'x-wisp-sw': '1' } });
}

async function file(req: Request): Promise<Response> {
  const cache = await caches.open(CACHE);
  const hit = await cache.match(req);
  if (hit) return hit;
  const r = await fetch(req);
  if (r.ok && r.type === 'basic' && new URL(req.url).pathname.startsWith('/assets/')) cache.put(req, r.clone()).catch(() => {});
  return r;
}

sw.addEventListener('install', (e) => {
  void sw.skipWaiting();
  e.waitUntil(caches.open(CACHE).then((c) => refresh(c)));
});

sw.addEventListener('activate', (e) => {
  e.waitUntil(
    caches
      .keys()
      // a share not read yet stays: it is no build's (share-form.ts)
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE && k !== SHARED).map((k) => caches.delete(k))))
      .then(() => sw.clients.claim()),
  );
});

/** A tab's check (worker.ts): one at a time, none within `GAP` of the last. */
const GAP = 30_000;
const checking: { at: number; running: Promise<unknown> | undefined } = { at: -Infinity, running: undefined };

function check(): Promise<unknown> {
  if (checking.running || Date.now() - checking.at < GAP) return checking.running ?? Promise.resolve();
  checking.at = Date.now();
  checking.running = caches
    .open(CACHE)
    .then((c) => refresh(c))
    .then((r) => (r === 'new' ? tell({ t: 'update' }) : undefined))
    .catch(() => {})
    .finally(() => {
      checking.running = undefined;
    });
  return checking.running;
}

sw.addEventListener('message', (e) => {
  const d = e.data as { t?: unknown } | null;
  if (d && typeof d === 'object' && d.t === 'check') e.waitUntil(check());
});

/** The share sheet's form, kept for the page: it opens on `/?shared=<id>` (a 303, so a reload doesn't post again).
 *  One that can't be read still opens the app, with nothing to take. */
async function shared(req: Request): Promise<Response> {
  const id = await keep(req, caches).catch(() => 'none');
  return Response.redirect(new URL(`/?shared=${encodeURIComponent(id)}`, sw.location.origin).href, 303);
}

/** Where a tapped note goes: a tab of this site, focused and told (browser.ts reads `{t:'open'}`), else a new window
 *  there. */
async function opened(url: string): Promise<void> {
  const tabs = await sw.clients.matchAll({ type: 'window', includeUncontrolled: true });
  const tab = tabs.find((c) => !c.url || new URL(c.url).origin === sw.location.origin);
  if (tab) {
    await tab.focus?.().catch(() => {});
    tab.postMessage({ t: 'open', url });
    return;
  }
  await sw.clients.openWindow?.(url);
}

sw.addEventListener('notificationclick', (e) => {
  e.notification.close();
  const data = e.notification.data as { url?: unknown } | null | undefined;
  const raw = typeof data?.url === 'string' ? data.url : '/';
  // only a link of this site (a note's data is the page's own, but the worker never opens elsewhere)
  const to = new URL(raw, sw.location.origin);
  const url = to.origin === sw.location.origin ? to.pathname + to.search : '/';
  e.waitUntil(opened(url).catch(() => {}));
});

sw.addEventListener('fetch', (e) => {
  const req = e.request;
  const url = new URL(req.url);
  if (req.method === 'POST' && url.origin === sw.location.origin && url.pathname === '/share') return e.respondWith(shared(req));
  if (req.method !== 'GET' || skip(url)) return;
  e.respondWith(req.mode === 'navigate' ? navigate(e) : file(req));
});
