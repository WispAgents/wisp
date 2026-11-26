// The service worker's two ways in from outside the page (src/platform/sw.ts): the installed app's share target (a POST
// /share answered nowhere: the worker returned on every non-GET, so a share opened a 405) and a tapped notification
// (no notificationclick: a note the worker showed did nothing when tapped).
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { CACHE as SHARED, MAX_FILES, take, type CacheStore } from '../src/platform/share-form.ts';

const ORIGIN = 'https://box.grid.example';
type Handler = (e: unknown) => void;

let on: Map<string, Handler>;
let stores: Map<string, Map<string, Response>>;
let tabs: { url: string; focused: number; posted: unknown[] }[];
let windows: string[];

const store: CacheStore = {
  open: async (name) => {
    const m = stores.get(name) ?? stores.set(name, new Map()).get(name)!;
    const key = (r: string | Request): string => new URL(typeof r === 'string' ? r : r.url, ORIGIN).pathname;
    return {
      put: async (k, res) => void m.set(key(k), res),
      match: async (k) => m.get(key(k))?.clone(),
      delete: async (k) => m.delete(key(k)),
      keys: async () => [...m.keys()].map((k) => new Request(new URL(k, ORIGIN))),
    };
  },
};

beforeEach(async () => {
  vi.resetModules();
  on = new Map();
  stores = new Map();
  tabs = [];
  windows = [];
  vi.stubGlobal('caches', { ...store, keys: async () => [...stores.keys()], delete: async (n: string) => stores.delete(n) });
  vi.stubGlobal('fetch', async () => new Response('', { status: 404 }));
  vi.stubGlobal('location', { origin: ORIGIN });
  vi.stubGlobal('clients', {
    claim: async () => {},
    matchAll: async () => tabs.map((t) => ({ url: t.url, postMessage: (m: unknown) => void t.posted.push(m), focus: async () => void t.focused++ })),
    get: async () => undefined,
    openWindow: async (url: string) => void windows.push(url),
  });
  vi.stubGlobal('skipWaiting', async () => {});
  vi.stubGlobal('addEventListener', (t: string, fn: Handler) => void on.set(t, fn));
  // check:allow dynamic-import-in-test the worker registers its listeners on import: each test needs a fresh one (vi.resetModules)
  await import('../src/platform/sw.ts');
});

afterEach(() => vi.unstubAllGlobals());

/** A fetch event; what the worker answered, or undefined when it let the request through. */
async function fetchEvent(req: Request): Promise<Response | undefined> {
  let answer: Promise<Response> | Response | undefined;
  on.get('fetch')?.({ request: req, respondWith: (r: Promise<Response> | Response) => void (answer = r), waitUntil: () => {} });
  return answer && (await answer);
}

test('a_post_to_share_is_kept_capped_and_answered_with_a_303_to_the_page', async () => {
  const form = new FormData();
  form.set('title', 'A page');
  form.set('text', 'Look at this');
  form.set('url', 'https://example.com/a');
  for (let i = 0; i <= MAX_FILES; i++) form.append('files', new File([`img ${i}`], `p${i}.png`, { type: 'image/png' }));
  form.append('files', new File(['MZ'], 'tool.exe', { type: 'application/octet-stream' }));
  const res = await fetchEvent(new Request(`${ORIGIN}/share`, { method: 'POST', body: form }));
  expect(res?.status).toBe(303);
  const to = new URL(res?.headers.get('location') ?? '');
  expect(to.origin + to.pathname).toBe(`${ORIGIN}/`);
  const id = to.searchParams.get('shared') ?? '';
  expect(id).not.toBe('none');
  expect([...(stores.get(SHARED)?.keys() ?? [])]).toEqual([`/shared/${id}`]);
  const got = await take(id, store);
  expect([got?.title, got?.text, got?.url]).toEqual(['A page', 'Look at this', 'https://example.com/a']);
  // form.ts's caps: MAX_FILES of the images, the one past them and the type it doesn't take left out
  expect(got?.files.map((f) => f.name)).toEqual([...Array(MAX_FILES).keys()].map((i) => `p${i}.png`));
  expect(got?.left).toBe(2);
});

test('every_other_non_get_request_goes_by_untouched', async () => {
  expect(await fetchEvent(new Request(`${ORIGIN}/api/command`, { method: 'POST', body: '{}' }))).toBeUndefined();
  expect(await fetchEvent(new Request(`${ORIGIN}/share`, { method: 'PUT', body: 'x' }))).toBeUndefined();
  expect(await fetchEvent(new Request('https://elsewhere.example/share', { method: 'POST', body: new FormData() }))).toBeUndefined();
  expect(stores.has(SHARED)).toBe(false);
});

async function tap(data: unknown): Promise<number> {
  let closed = 0;
  const waits: Promise<unknown>[] = [];
  on.get('notificationclick')?.({ notification: { data, close: () => void closed++ }, waitUntil: (p: Promise<unknown>) => void waits.push(p) });
  await Promise.all(waits);
  return closed;
}

test('a_tapped_note_focuses_an_open_tab_and_tells_it_where_to_go', async () => {
  tabs.push({ url: 'https://other.example/', focused: 0, posted: [] }, { url: `${ORIGIN}/task/p%3As1`, focused: 0, posted: [] });
  expect(await tap({ url: '/task/p%3As2' })).toBe(1);
  expect(tabs[0]).toMatchObject({ focused: 0, posted: [] });
  expect(tabs[1]).toMatchObject({ focused: 1, posted: [{ t: 'open', url: '/task/p%3As2' }] });
  expect(windows).toEqual([]);
});

test('a_tapped_note_with_no_tab_open_opens_a_window_on_its_link', async () => {
  expect(await tap({ url: '/task/p%3As2' })).toBe(1);
  expect(windows).toEqual(['/task/p%3As2']);
  // a link elsewhere never: the app's home
  await tap({ url: 'https://evil.example/x' });
  expect(windows.at(-1)).toBe('/');
});
