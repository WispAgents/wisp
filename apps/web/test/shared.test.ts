// The installed web app's share target (v1 web/manifest.webmanifest share_target, web/js/features/share-in.js): v2's
// manifest offered no share target, the worker answered no POST /share and nothing read `?shared=`, so wisp was never
// in the phone's share sheet. The manifest offers it again, and the page reads what the worker kept once (src/shared.ts;
// the navigation takes `?shared=` off the URL: packages/client nav-share.test.ts).
import { readFileSync } from 'node:fs';
import { beforeEach, expect, test, vi } from 'vitest';
import type { PluginClient } from '@wisp/client';
import { keep, take, type CacheStore } from '@wisp/shell/share-form';

const received = vi.hoisted(() => [] as unknown[]);
vi.mock('@wisp-plugin/stuff-ui/share-in', () => ({
  receiveShared: (_c: unknown, r: unknown) => (received.push(r), 'one file left out'),
}));
const { readShared } = await import('../src/shared.ts');

const ORIGIN = 'https://box.grid.example';
let stores: Map<string, Map<string, Response>>;
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

beforeEach(() => {
  stores = new Map();
  received.length = 0;
});

function page(id: string) {
  const attached: File[][] = [];
  const said: string[] = [];
  const o = {
    id,
    caches: store,
    stuff: () => ({}) as PluginClient,
    attach: (f: readonly File[]) => void attached.push([...f]),
    say: (t: string) => void said.push(t),
  };
  return { o, attached, said };
}

test('the_manifest_offers_wisp_in_the_share_sheet', () => {
  const m = JSON.parse(readFileSync(new URL('../public/manifest.webmanifest', import.meta.url), 'utf8')) as { share_target?: { action?: string; method?: string; enctype?: string } };
  expect(m.share_target).toMatchObject({ action: '/share', method: 'POST', enctype: 'multipart/form-data' });
});

test('a_page_opened_with_shared_reads_it_once_into_new_task', async () => {
  const form = new FormData();
  form.set('title', 'A page');
  form.set('text', 'Look');
  form.set('url', 'https://example.com/a');
  form.append('files', new File(['png'], 'shot.png', { type: 'image/png' }));
  const id = await keep(new Request(`${ORIGIN}/share`, { method: 'POST', body: form }), store);
  const p = page(id);
  await readShared(p.o);
  expect(received).toEqual([{ title: 'A page', text: 'Look', url: 'https://example.com/a', files: 1 }]);
  expect(p.attached.map((f) => f.map((x) => x.name))).toEqual([['shot.png']]);
  expect(p.said).toEqual(['one file left out']);
  expect(await take(id, store), 'read once: gone').toBeNull();
});

test('a_missing_or_none_id_does_nothing', async () => {
  for (const id of ['none', '', 'abc']) {
    const p = page(id);
    await readShared(p.o);
    expect([p.attached, p.said]).toEqual([[], []]);
  }
  expect(received).toEqual([]);
});
