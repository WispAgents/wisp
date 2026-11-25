// An answer's file on a page its peer doesn't serve (the phone app): the transport's URL, live, null while asked
// (src/frame/media-src.ts).
import { expect, test } from 'vitest';
import type { PeerHttp } from '@wisp/client';
import { FRESH_MS, peerMedia } from '../src/frame/media-src.ts';

function http(media?: (path: string) => Promise<string>): PeerHttp & { asked: string[] } {
  const asked: string[] = [];
  return {
    asked,
    send: () => Promise.resolve(null),
    events: () => ({ [Symbol.asyncIterator]: () => ({ next: () => Promise.resolve({ done: true as const, value: undefined }) }) }),
    ...(media && { media: (p: string) => (asked.push(p), media(p)) }),
  };
}

test('a transport with no media URLs: the page’s own relative URL', () => {
  expect(peerMedia({}, http(), 'home', '/api/sessions/s1/file?path=a.png', 0)).toBe('relative');
});

test('on the phone: null while asked, then the signed URL; asked once until it goes stale', async () => {
  const owner = {};
  const h = http((p) => Promise.resolve(`https://mo.grid.test${p}&_gs=x`));
  const path = '/api/sessions/s1/file?path=a.png';
  expect(peerMedia(owner, h, 'cloud', path, 0)).toBeNull();
  await Promise.resolve();
  expect(peerMedia(owner, h, 'cloud', path, 1)).toBe(`https://mo.grid.test${path}&_gs=x`);
  expect(h.asked).toEqual([path]);
  peerMedia(owner, h, 'cloud', path, FRESH_MS + 1);
  expect(h.asked).toEqual([path, path]);
});

test('a refusal: undefined (can’t be shown), and the next render asks again', async () => {
  const owner = {};
  const h = http(() => Promise.reject(new Error('HTTP 401')));
  const path = '/api/uploads/u1';
  expect(peerMedia(owner, h, 'cloud', path, 0)).toBeNull();
  await new Promise((r) => setTimeout(r, 0));
  peerMedia(owner, h, 'cloud', path, 1);
  expect(h.asked).toEqual([path, path]);
});
