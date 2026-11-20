// fetch on a connection of its own (connection: close), with the body read at once.
//
// Own connection: a pooled one can go stale while the install gate's execFileSync blocks the loop:
// wisp closes it after 65 idle seconds (KEEP_ALIVE_MS), this process can't see that until it runs
// again, and the next request goes out on it and gets a reset ("fetch failed") from a wisp that's
// still up. Slow npx on Windows took INST-04b to 04d past that (#770).
//
// Body read at once: on Node 22.23.4, undici asserts `!this.paused` and kills the process when the
// server closes a connection whose response body nobody has read yet (a 64-130 KB page is enough).
// A row that only looked at `.ok` of GET / crashed the gate on Mac and Linux (release 44085115543).
export async function closeFetch(url, opts = {}) {
  const r = await fetch(url, { ...opts, headers: { ...opts.headers, connection: 'close' } });
  const body = await r.arrayBuffer();
  const empty = [101, 204, 205, 304].includes(r.status);
  return new Response(empty ? null : body, { status: r.status, statusText: r.statusText, headers: r.headers });
}
