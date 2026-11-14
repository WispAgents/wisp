// F-phone-8: the web app on a phone with 4,000 tasks on a bad connection. A 390 px touch page in
// Chrome, CPU slowed 4× and DevTools' Slow 4G (562.5 ms RTT, 1.44 Mbit/s down, 675 kbit/s up),
// against the built bundle and the mock peer:
//
//   cd apps/web && npx vite build && MOCK_SESSIONS=4000 PORT=5191 npx vite preview --strictPort &
//   PW=/tmp/pw/node_modules/playwright-core/index.js node spikes/phone-bench/bench.mjs [url]
//
// Measures (ADR-0011): first list paint cold (nothing cached) and warm (HTTP cache + IndexedDB),
// long tasks while the list is flung top to bottom, tap → task page frame, Back → list frame, and
// long tasks while a turn streams into a running task with 4,000 rows in the store (no full-list
// work per event). Prints a table and writes spikes/phone-bench/out.json. Not a CI gate: a probe,
// run before and after a change to the list, the stack layout or the boot path.

import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const PW = process.env.PW || '/tmp/pw/node_modules/playwright-core/index.js';
const URL_ = process.argv[2] || 'http://127.0.0.1:5191/';
const { chromium } = (await import(pathToFileURL(PW).href)).default;
const SLOW_4G = { offline: false, latency: 562.5, downloadThroughput: (1.44e6 / 8) | 0, uploadThroughput: (675e3 / 8) | 0 };
const BUDGET = { tapMs: 50, longTasksWhileStreaming: 0 };

// in the page, before any module: long tasks, and the first frame a selector shows up in
const init = () => {
  window.__long = [];
  new PerformanceObserver((l) => {
    for (const e of l.getEntries()) window.__long.push({ at: Math.round(e.startTime), dur: Math.round(e.duration) });
  }).observe({ type: 'longtask', buffered: true });
  window.__firstRow = 0;
  const look = () => {
    if (document.querySelector('[data-kit="feed-item"]')) return void (window.__firstRow = performance.now());
    requestAnimationFrame(look);
  };
  requestAnimationFrame(look);
  // tap → the frame where `sel` is on screen (and `gone` isn't)
  window.__until = (sel, gone) =>
    new Promise((done) => {
      const t0 = performance.now();
      const tick = () => {
        const el = document.querySelector(sel);
        if (el && el.getBoundingClientRect().width > 0 && !(gone && document.querySelector(gone)?.getBoundingClientRect().width)) return done(performance.now() - t0);
        requestAnimationFrame(tick);
      };
      tick();
    });
};

async function page(ctx, net) {
  const p = await ctx.newPage();
  const cdp = await ctx.newCDPSession(p);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
  await cdp.send('Network.enable');
  if (net) await cdp.send('Network.emulateNetworkConditions', SLOW_4G);
  return { p, cdp };
}

const longs = (p, from) => p.evaluate((f) => window.__long.filter((l) => l.at >= f), from);
const now = (p) => p.evaluate(() => performance.now());
const sum = (ls) => ({ count: ls.length, worst: Math.max(0, ...ls.map((l) => l.dur)), total: ls.reduce((a, l) => a + l.dur, 0) });

const browser = await chromium.launch({ channel: 'chrome', headless: true });
const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 3 });
await ctx.addInitScript(init);
const out = {};
await fetch(new URL('/__mock/reset', URL_), { method: 'POST' });

// 1. cold: nothing cached, Slow 4G
{
  const { p, cdp } = await page(ctx, true);
  await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
  await p.goto(URL_, { waitUntil: 'load', timeout: 180_000 });
  await p.waitForFunction(() => window.__firstRow > 0, null, { timeout: 180_000 });
  const fcp = await p.evaluate(() => performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? 0);
  out.cold = { fcpMs: Math.round(fcp), firstListMs: Math.round(await p.evaluate(() => window.__firstRow)), boot: sum(await longs(p, 0)) };
  await p.waitForTimeout(3000); // let the client write its IndexedDB snapshot
  await p.close();
}

// 2. warm: the same phone opens wisp again (HTTP cache, IndexedDB), Slow 4G
const { p, cdp } = await page(ctx, true);
await p.goto(URL_, { waitUntil: 'load', timeout: 180_000 });
await p.waitForFunction(() => window.__firstRow > 0, null, { timeout: 180_000 });
out.warm = { fcpMs: Math.round(await p.evaluate(() => performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? 0)), firstListMs: Math.round(await p.evaluate(() => window.__firstRow)), boot: sum(await longs(p, 0)) };
// all 4,000 there (synced over Slow 4G)
await p.waitForFunction(() => document.querySelectorAll('[data-kit="feed-item"]').length > 5 && /\d{3,}/.test(document.body.innerText), null, { timeout: 180_000 }).catch(() => {});
await p.waitForTimeout(2000);
out.rowsInDom = await p.evaluate(() => document.querySelectorAll('[data-kit="virtual-row"]').length);

// 3. fling the list: 60 scroll steps to the end, one per frame, then back to the top
{
  const t = await now(p);
  const frames = await p.evaluate(async () => {
    const box = document.querySelector('[data-kit="virtual-list"][data-part~="list.rows"]');
    const sc = (() => {
      for (let e = box; e; e = e.parentElement) if (e.scrollHeight > e.clientHeight + 10 && /auto|scroll/.test(getComputedStyle(e).overflowY)) return e;
      return document.scrollingElement;
    })();
    const ft = [];
    let last = performance.now();
    const step = (to) => new Promise((r) => requestAnimationFrame(() => { sc.scrollTop = to; const n = performance.now(); ft.push(n - last); last = n; r(); }));
    const max = sc.scrollHeight - sc.clientHeight;
    for (let i = 1; i <= 60; i++) await step((max * i) / 60);
    for (let i = 59; i >= 0; i--) await step((max * i) / 60);
    return { height: sc.scrollHeight, over50: ft.filter((f) => f > 50).length, worst: Math.round(Math.max(...ft)), p95: Math.round(ft.sort((a, b) => a - b)[Math.floor(ft.length * 0.95)]) };
  });
  out.scroll = { ...frames, longTasks: sum(await longs(p, t)) };
}

// 3b. a thumb's fling: touch scroll gestures at 3,000 px/s (a hard flick), 12,000 px down and back up
{
  const t = await now(p);
  const box = await p.locator('[data-kit="virtual-list"][data-part~="list.rows"]').boundingBox();
  const x = Math.round(box.x + box.width / 2);
  const y = Math.round(box.y + box.height / 2);
  await p.evaluate(() => {
    window.__ft = [];
    let last = performance.now();
    const f = () => { const n = performance.now(); window.__ft.push(n - last); last = n; if (window.__ft.length < 100000) window.__raf = requestAnimationFrame(f); };
    window.__raf = requestAnimationFrame(f);
  });
  for (const dy of [-12000, 12000]) await cdp.send('Input.synthesizeScrollGesture', { x, y, yDistance: dy, speed: 3000, gestureSourceType: 'touch', preventFling: false });
  const ft = await p.evaluate(() => (cancelAnimationFrame(window.__raf), window.__ft.slice(1)));
  ft.sort((a, b) => a - b);
  out.fling = { frames: ft.length, over50: ft.filter((f) => f > 50).length, p95: Math.round(ft[Math.floor(ft.length * 0.95)] ?? 0), worst: Math.round(ft.at(-1) ?? 0), longTasks: sum(await longs(p, t)) };
}

// 4. tap a task → its page; Back → the list (keyboard Escape on a phone does nothing; the back button is browser history)
{
  const row = p.locator('[data-kit="feed-item"] [data-kit="list-row"]').first();
  const id = await p.locator('[data-kit="feed-item"]').first().getAttribute('data-id');
  const taps = [];
  const backs = [];
  for (let i = 0; i < 3; i++) {
    const t = await now(p);
    const wait = p.evaluate(() => window.__until('[data-part="split.back"], [data-part="split.title"]', '[data-kit="virtual-list"][data-part~="list.rows"]'));
    await row.tap();
    taps.push(await wait);
    await p.waitForTimeout(500);
    out[`openLong${i}`] = sum(await longs(p, t));
    const wb = p.evaluate(() => window.__until('[data-kit="virtual-list"][data-part~="list.rows"]'));
    await p.goBack();
    backs.push(await wb);
    await p.waitForTimeout(500);
  }
  out.open = { task: id, tapToPageMs: taps.map(Math.round), backToListMs: backs.map(Math.round) };
}

// 5. a turn streams into a running task (16 ms a token) with 4,000 rows in the store, the list on screen
{
  await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
  const t = await now(p);
  await fetch(new URL('/__mock/stream?tokens=200&every=16', URL_), { method: 'POST' });
  await p.waitForTimeout(5000);
  out.streaming = sum(await longs(p, t));
}

await browser.close();
writeFileSync(new URL('./out.json', import.meta.url), JSON.stringify(out, null, 2));
console.log(JSON.stringify(out, null, 2));
const tapP = Math.max(...out.open.tapToPageMs.slice(1));
console.log(`\ntap → task page (warm) ${tapP} ms, budget ${BUDGET.tapMs}; long tasks while streaming ${out.streaming.count}, budget ${BUDGET.longTasksWhileStreaming}`);
