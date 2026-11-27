/** until `ok`, checked after each macrotask; gives up after `ms` by the clock (the assertion after it then fails) */
export async function until(ok: () => boolean, ms = 15_000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}
