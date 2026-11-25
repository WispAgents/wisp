// A saved computer as a @wisp/client Transport (plan §7.2.2, §7.3.1). Every connect asks the native
// side how to reach it first: an SSH tunnel may have come back on another local port, a grid's
// device token may have been renewed. Then it is plain HTTP sync (httpTransport): SSE over fetch
// on GET /api/events, POSTs for sub/unsub/window/mutate, with `Authorization: Bearer <token>`.

import type { ClientMessage } from '@wisp/protocol';
import { httpTransport, PeerDown, type FetchLike, type Frame, type Transport } from '@wisp/client';
import type { Computer, NativeBridge, Reached } from './bridge.ts';

export interface BridgeTransportOptions {
  /** the WebView's fetch by default */
  fetch?: FetchLike;
}

const message = (err: unknown): string => (err instanceof Error ? err.message : String(err));

/** The token is the computer's own (server.json over SSH) or the grid's device token. */
const auth = (token: string): Record<string, string> => (token ? { authorization: `Bearer ${token}` } : {});

/**
 * A grid that put a box to sleep answers 503 with `{"asleep": true}` and doesn't start it for a
 * passive request. A 503 `{"waking": true}` is a box on its way up: retried, not asleep.
 */
export function gridAsleep(status: number, body: string): boolean {
  if (status !== 503) return false;
  try {
    const o: unknown = JSON.parse(body);
    return typeof o === 'object' && o !== null && (o as { asleep?: unknown }).asleep === true;
  } catch {
    return false;
  }
}

function httpFor(r: Reached, fetchFn: FetchLike | undefined): Transport {
  const base = r.kind === 'ssh' ? `http://127.0.0.1:${r.port}` : r.base;
  return httpTransport({ base, headers: () => auth(r.token), ...(fetchFn && { fetch: fetchFn }), ...(r.kind === 'grid' && { asleep: gridAsleep }) });
}

export function bridgeTransport(bridge: NativeBridge, computer: Pick<Computer, 'id' | 'kind'>, o: BridgeTransportOptions = {}): Transport {
  let open: Transport | undefined; // the connection's own: posts go where its stream went

  const reach = async (): Promise<Reached> => {
    try {
      return await bridge.connect(computer.id);
    } catch (err) {
      throw new PeerDown('unreachable', message(err));
    }
  };

  const transport: Transport = {
    kind: computer.kind,
    async *connect(params, signal): AsyncGenerator<Frame> {
      const r = await reach();
      if (signal.aborted) return;
      const http = httpFor(r, o.fetch);
      open = http;
      try {
        yield* http.connect(params, signal);
      } finally {
        if (open === http) open = undefined;
      }
    },
    post(m: ClientMessage, signal) {
      return open ? open.post(m, signal) : Promise.reject(new PeerDown('unreachable', 'not connected'));
    },
    // a plugin's own routes (an attached file's POST /uploads, a terminal's stream): where the stream went, or
    // reached afresh when it isn't open. Without these PeerHttp refused every route on the phone ("can't be reached")
    async request(r, signal) {
      const http = open ?? httpFor(await reach(), o.fetch);
      return http.request ? http.request(r, signal) : Promise.reject(new PeerDown('unreachable', 'no routes'));
    },
    async *events(path, signal) {
      const http = open ?? httpFor(await reach(), o.fetch);
      if (http.events) yield* http.events(path, signal);
    },
    // an answer's picture or video for a plain <img>/<video>, which sends no Bearer header and isn't on this page's
    // origin (https://localhost): through a tunnel, its local address (wisp trusts loopback); through the grid, a URL
    // the grid signed for this device (POST /_grid/media, an hour)
    async mediaUrl(path, signal) {
      const r = await reach();
      if (r.kind === 'ssh') return `http://127.0.0.1:${r.port}${path}`;
      return sign(r, path, signal);
    },
  };
  const sign = gridSigner(o.fetch);
  if (computer.kind !== 'grid') return transport;
  return { ...transport, wake: (signal) => wakeGrid(reach, o.fetch, signal) };
}

const SIGN_MAX = 100; // the grid's own limit per POST /_grid/media

/**
 * Asks the grid to sign paths, the ones asked for in the same tick in one POST (an answer's pictures all render
 * together). Resolves each with its absolute URL; a refusal rejects them all.
 */
export function gridSigner(fetchFn: FetchLike | undefined): (r: Extract<Reached, { kind: 'grid' }>, path: string, signal: AbortSignal) => Promise<string> {
  const f: FetchLike = fetchFn ?? ((url, init) => globalThis.fetch(url, init));
  interface Ask {
    readonly path: string;
    readonly ok: (url: string) => void;
    readonly fail: (err: unknown) => void;
  }
  let queue: { r: Extract<Reached, { kind: 'grid' }>; asks: Ask[] } | undefined;
  const flush = async (r: Extract<Reached, { kind: 'grid' }>, asks: readonly Ask[]): Promise<void> => {
    for (let i = 0; i < asks.length; i += SIGN_MAX) {
      const part = asks.slice(i, i + SIGN_MAX);
      try {
        const res = await f(`${r.base}/_grid/media`, {
          method: 'POST',
          headers: { 'content-type': 'application/json', accept: 'application/json', ...auth(r.token) },
          body: JSON.stringify({ paths: part.map((a) => a.path) }),
          signal: new AbortController().signal, // shared by every ask in it: one leaving doesn't cancel the rest
        });
        if (res.status >= 400) throw new Error(`${r.name}: HTTP ${res.status}`);
        const urls = (JSON.parse(await res.text()) as { urls?: unknown }).urls;
        if (!Array.isArray(urls) || urls.length !== part.length) throw new Error(`${r.name}: no signed links`);
        part.forEach((a, n) => a.ok(`${r.base}${String(urls[n])}`));
      } catch (err) {
        for (const a of part) a.fail(err);
      }
    }
  };
  return (r, path, signal) =>
    new Promise<string>((ok, fail) => {
      if (signal.aborted) return fail(new Error('aborted'));
      if (!queue || queue.r.base !== r.base) {
        const q = { r, asks: [] as Ask[] };
        queue = q;
        queueMicrotask(() => {
          if (queue === q) queue = undefined;
          void flush(q.r, q.asks);
        });
      }
      queue.asks.push({ path, ok, fail });
    });
}

/**
 * The grid starts a box on any request that isn't passive (v1 grid/server.js boxEdge): one GET of
 * its /api/health does it. A 503 means it's still starting, which the next connect rides out.
 */
async function wakeGrid(reach: () => Promise<Reached>, fetchFn: FetchLike | undefined, signal: AbortSignal): Promise<void> {
  const r = await reach();
  if (r.kind !== 'grid') return;
  const f: FetchLike = fetchFn ?? ((url, init) => globalThis.fetch(url, init));
  const res = await f(`${r.base}/api/health`, { method: 'GET', headers: { accept: 'application/json', ...auth(r.token) }, signal });
  if (res.status === 401 || res.status === 403) throw new Error(`Signed out of ${r.name}: sign in to the grid again`);
}
