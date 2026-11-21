// The web app's tests wait on what they then assert, never on a fixed count of timer hops (round 322: on Windows a
// timer hop is a ~15.6 ms system tick, so a 30-hop flush cost ~0.5 s there and a 300 × 10 ms cap ran ~4.7 s, past
// vitest's 5 s under CI load). Plain code only (test-imports: no .tsx through here).

/** until `ok`, checked after each macrotask; gives up after `ms` by the clock (the assertion after it then fails with
 * its own message). 4 s by default: a cold lazy chunk (the frame, a phone screen, a pane) lands a beat after the
 * navigation. */
export async function until(ok: () => boolean, ms = 4000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

/** One macrotask: what the mock peer answers lands, and the client applies it. For a check that something did NOT
 * happen, after the thing that would have made it happen already ran (a timer the test fired itself). */
export const hop = (): Promise<void> => new Promise((r) => setTimeout(r, 0));
