// The service worker (src/platform/sw.ts) against a fake cache and network: a build's page is kept only with its
// modules (a failed /_pack left a new page over no modules: blank offline), old builds' files go once a new one is in
// (the cache and `?have=` grew forever), and a tab's `check` finds a new build without a navigation (an open or
// resumed app never learned of one).
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const ORIGIN = 'https://box.grid.example';
type Handler = (e: unknown) => void;

interface World {
  cache: Map<string, Response>;
  packs: string[];
  posted: unknown[];
  build: string;
  /** the build's page and modules */
  builds: Record<string, { page: string; files: Record<string, string> }>;
  packFails: boolean;
  on: Map<string, Handler>;
}

const page = (assets: readonly string[]): string =>
  `<!doctype html><html><head>${assets
    .map((a) => (a.endsWith('.css') ? `<link rel="stylesheet" href="${a}">` : `<link rel="modulepreload" href="${a}">`))
    .join('')}<script type="module" src="${assets[0]}"></script></head></html>`;

let w: World;

function fakeWorld(): World {
  const cache = new Map<string, Response>();
  const path = (r: RequestInfo | URL): string => {
    const u = new URL(typeof r === 'string' ? r : r instanceof URL ? r.href : r.url, ORIGIN);
    return u.pathname + u.search;
  };
  const world: World = { cache, packs: [], posted: [], build: 'A', builds: {}, packFails: false, on: new Map() };
  const c = {
    match: async (r: RequestInfo) => cache.get(path(r))?.clone(),
    put: async (r: RequestInfo, res: Response) => void cache.set(path(r), res),
    keys: async () => [...cache.keys()].map((k) => new Request(new URL(k, ORIGIN))),
    delete: async (r: RequestInfo) => cache.delete(path(r)),
  };
  vi.stubGlobal('caches', { open: async () => c, keys: async () => ['wisp-v2'], delete: async () => true });
  vi.stubGlobal('fetch', async (r: RequestInfo) => {
    const p = path(r);
    const b = world.builds[world.build];
    if (!b) throw new Error('no build');
    if (p === '/') return new Response(b.page, { headers: { 'content-type': 'text/html', 'x-wisp-build': world.build } });
    if (p.startsWith('/_pack')) {
      world.packs.push(p);
      if (world.packFails) return new Response('down', { status: 502 });
      const have = new Set(decodeURIComponent(new URL(p, ORIGIN).searchParams.get('have') ?? '').split(',').filter(Boolean));
      return Response.json(Object.fromEntries(Object.entries(b.files).filter(([u]) => !have.has(u))));
    }
    return new Response('', { status: 404 });
  });
  vi.stubGlobal('location', { origin: ORIGIN });
  vi.stubGlobal('clients', { claim: async () => {}, matchAll: async () => [{ postMessage: (m: unknown) => world.posted.push(m) }], get: async () => undefined });
  vi.stubGlobal('skipWaiting', async () => {});
  vi.stubGlobal('addEventListener', (t: string, fn: Handler) => void world.on.set(t, fn));
  return world;
}

/** Runs one of the worker's events and waits for what it waits on. */
async function fire(type: string, data?: unknown): Promise<void> {
  const waits: Promise<unknown>[] = [];
  w.on.get(type)?.({ data, waitUntil: (p: Promise<unknown>) => void waits.push(p) });
  await Promise.all(waits);
}

const cachedPage = async (): Promise<string> => (await w.cache.get('/')?.clone().text()) ?? '';
const assets = (): string[] => [...w.cache.keys()].filter((k) => k.startsWith('/assets/')).sort();

beforeEach(async () => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(1_000_000);
  vi.resetModules();
  w = fakeWorld();
  // build A: its entry, a module B keeps (shared), one only A has, a lazy module the page doesn't name, its stylesheet
  w.builds['A'] = {
    page: page(['/assets/index-a1.js', '/assets/shared-s1.js', '/assets/only-a.js', '/assets/index-a1.css']),
    files: { '/assets/index-a1.js': 'a', '/assets/shared-s1.js': 's', '/assets/only-a.js': 'oa', '/assets/lazy-l1.js': 'l' },
  };
  w.builds['B'] = {
    page: page(['/assets/index-b1.js', '/assets/shared-s1.js', '/assets/index-b1.css']),
    files: { '/assets/index-b1.js': 'b', '/assets/shared-s1.js': 's', '/assets/lazy-l1.js': 'l' },
  };
  // check:allow dynamic-import-in-test the worker registers its listeners on import: each test needs a fresh one (vi.resetModules)
  await import('../src/platform/sw.ts');
  await fire('install');
  // a stylesheet comes the usual way (file()), and so does a font, which only a stylesheet names
  w.cache.set('/assets/index-a1.css', new Response('css'));
  w.cache.set('/assets/font-f1.woff2', new Response('font'));
  w.posted.length = 0;
  w.packs.length = 0;
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test('a_failed_pack_keeps_the_old_page_and_tells_no_tab', async () => {
  expect(await cachedPage()).toBe(w.builds['A']?.page);
  w.build = 'B';
  w.packFails = true;
  await fire('message', { t: 'check' });
  expect(w.packs).toHaveLength(1);
  expect(await cachedPage(), 'the old build, whose modules are all here').toBe(w.builds['A']?.page);
  expect(w.posted).toEqual([]);
  // the next look tries again, and this time it's in
  w.packFails = false;
  vi.setSystemTime(1_000_000 + 31_000);
  await fire('message', { t: 'check' });
  expect(await cachedPage()).toBe(w.builds['B']?.page);
  expect(w.posted).toEqual([{ t: 'update' }]);
});

test('a_new_build_drops_the_files_of_the_builds_before_the_last_and_keeps_what_they_share', async () => {
  const before = assets();
  w.build = 'B';
  await fire('message', { t: 'check' });
  // the look asked only for what the new page names
  expect(w.packs).toEqual([`/_pack?have=${encodeURIComponent('/assets/shared-s1.js')}`]);
  // build A stays this once: the tab whose navigation found B still runs A, and loads A's lazy modules from here
  expect(assets()).toEqual([...new Set([...before, '/assets/index-b1.js'])].sort());
  // build C: build A's own files go (its entry, its only module, its stylesheet); B's and what they share stay, and
  // the font, which only a stylesheet names
  w.builds['C'] = { page: page(['/assets/index-c1.js', '/assets/index-b1.js', '/assets/shared-s1.js']), files: { '/assets/index-c1.js': 'c', '/assets/index-b1.js': 'b', '/assets/shared-s1.js': 's' } };
  w.build = 'C';
  vi.setSystemTime(1_000_000 + 31_000);
  await fire('message', { t: 'check' });
  // `have` is what build C's page names, never build A's
  const have = decodeURIComponent(new URL(w.packs[1] ?? '', ORIGIN).searchParams.get('have') ?? '').split(',').sort();
  expect(have).toEqual(['/assets/index-b1.js', '/assets/shared-s1.js']);
  expect(assets()).toEqual(['/assets/font-f1.woff2', '/assets/index-b1.js', '/assets/index-c1.js', '/assets/lazy-l1.js', '/assets/shared-s1.js']);
  // and D: B's lazy module goes (only B used it), C's files stay (index-b1 is one of them): about two builds long
  w.builds['D'] = { page: page(['/assets/index-d1.js', '/assets/shared-s1.js']), files: { '/assets/index-d1.js': 'd', '/assets/shared-s1.js': 's' } };
  w.build = 'D';
  vi.setSystemTime(1_000_000 + 62_000);
  await fire('message', { t: 'check' });
  expect(assets()).toEqual(['/assets/font-f1.woff2', '/assets/index-b1.js', '/assets/index-c1.js', '/assets/index-d1.js', '/assets/shared-s1.js']);
});

test('a_tabs_check_tells_the_tabs_of_a_new_build_and_says_nothing_of_the_same_one', async () => {
  await fire('message', { t: 'check' });
  expect(w.posted, 'same build').toEqual([]);
  expect(w.packs).toEqual([]);
  w.build = 'B';
  // within half a minute of the last check: none
  await fire('message', { t: 'check' });
  expect(w.posted).toEqual([]);
  vi.setSystemTime(1_000_000 + 31_000);
  await Promise.all([fire('message', { t: 'check' }), fire('message', { t: 'check' })]);
  expect(w.posted, 'one check at a time').toEqual([{ t: 'update' }]);
  expect(w.packs).toHaveLength(1);
  await fire('message', { t: 'nonsense' });
  expect(w.posted).toHaveLength(1);
});
