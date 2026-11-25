// An answer's file on a page its peer doesn't serve (the phone app, at https://localhost): a relative
// `/api/sessions/<id>/file` would hit the app's own bundle, and a plain <img> can't send the device's token. The peer's
// transport gives an absolute URL instead (Transport.mediaUrl: a tunnel's local address, or a grid-signed URL, an
// hour). Kept per client, live: null while it's asked, so the block shows its place, not "can't be shown".
import { createSignal, type Accessor } from 'solid-js';
import type { PeerHttp } from '@wisp/client';

/** the grid signs for an hour: ask again before that, never with an old link */
export const FRESH_MS = 50 * 60_000;

type State = string | null | undefined; // the URL, on its way, or failed
interface Entry {
  readonly url: Accessor<State>;
  readonly until: number;
}

const caches = new WeakMap<object, Map<string, Entry | 'relative'>>();

/**
 * `path` (`/api/sessions/<id>/file?path=…`, `/api/uploads/<id>`) on `peer` → the URL to load (string), null (asked,
 * not answered yet), undefined (it failed). `relative` when the transport needs none: the page's own URL works.
 */
export function peerMedia(owner: object, http: PeerHttp, peer: string, path: string, now: number): State | 'relative' {
  let cache = caches.get(owner);
  if (!cache) caches.set(owner, (cache = new Map()));
  const key = `${peer}\n${path}`;
  const hit = cache.get(key);
  if (hit === 'relative') return hit;
  if (hit && hit.until > now) return hit.url();
  const asked = http.media?.(path);
  if (!asked) {
    cache.set(key, 'relative');
    return 'relative';
  }
  const [url, setUrl] = createSignal<State>(null);
  cache.set(key, { url, until: now + FRESH_MS });
  asked.then(
    (u) => setUrl(u),
    () => {
      setUrl(undefined);
      cache.delete(key); // the next render asks again (a tunnel back, a box awake)
    },
  );
  return url();
}
