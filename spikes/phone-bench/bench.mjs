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
// work per event). Prints a table and writes spikes/phone-bench/out.json.
//
// The gate (AGENTS.md §Performance, tap → frame ≤ 50 ms p95 on a 4× slowed CPU): the bench's two taps, TAPS times
// each after a first one: a row → its task page, and the page's Back → the list; each p95, exit 1 when either is over.
// A tap is timed from the touch's own timestamp (not from when Playwright was asked: its actionability checks are no
// part of the tap) to the end of the layout of the first frame that shows the new page: found in that frame's rAF,
// then a ResizeObserver on it, whose first callback runs right after that frame's layout. That's the main thread's
// frame, the gate. Also reported: the browser's Event Timing entry for the tap (INP's measure: to the frame presented,
// which adds the compositor's and the display's ~10-20 ms in headless Chrome). The first open after load (code still
// cold) is reported, not gated, as is the browser's own Back (history). Part of the batched outer-loop pass.

import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const PW = process.env.PW || '/tmp/pw/node_modules/playwright-core/index.js';
const URL_ = process.argv[2] || 'http://127.0.0.1:5191/';
const { chromium } = (await import(pathToFileURL(PW).href)).default;
const SLOW_4G = { offline: false, latency: 562.5, downloadThroughput: (1.44e6 / 8) | 0, uploadThroughput: (675e3 / 8) | 0 };
const BUDGET = { tapMs: 50, longTasksWhileStreaming: 0 };
const TAPS = Number(process.env.TAPS ?? 20);
const p95 = (xs) => [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.ceil(xs.length * 0.95) - 1)] ?? 0;

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
  // the last touch's own time (its event timestamp: when the finger came down)
  window.__down = 0;
  addEventListener('pointerdown', (e) => (window.__down = e.timeStamp), true);
  // Event Timing: each input's time to the next frame presented after its handlers (≥ 16 ms are reported)
  window.__events = [];
  new PerformanceObserver((l) => {
    for (const e of l.getEntries()) if (e.interactionId) window.__events.push({ start: e.startTime, dur: e.duration });
  }).observe({ type: 'event', durationThreshold: 16, buffered: true });
  // on screen: rendered (not under display: none or content-visibility: hidden) and inside the viewport
  const shows = (el) => {
    if (!el?.checkVisibility()) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.right > 0 && r.left < innerWidth;
  };
  // from `t0` (default: the last touch) to the end of the layout of the frame where `sel` is on screen (and `gone`
  // isn't): found in its rAF, then timed by a ResizeObserver's first callback (after that frame's layout)
  window.__until = (sel, gone, t0) =>
    new Promise((done) => {
      const tick = () => {
        const el = document.querySelector(sel);
        if (shows(el) && !(gone && shows(document.querySelector(gone)))) {
          const ro = new ResizeObserver(() => {
            ro.disconnect();
            done(performance.now() - (t0 ?? window.__down));
          });
          return ro.observe(el);
        }
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
  // the last touch's Event Timing (to the frame presented; 16 when under the threshold), never under `frame`
  window.__presented = (frame) => Math.max(frame, 16, ...window.__events.filter((e) => e.start >= window.__down - 1).map((e) => e.dur));
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
  const PAGE = '[data-part="split.back"], [data-part="split.title"]';
  const LIST = '[data-kit="virtual-list"][data-part~="list.rows"]';
  // a tap on `loc`, timed to the frame that shows `sel` (and hides `gone`); its presented time goes in `presented`
  const tap = async (loc, sel, gone, presented) => {
    const shown = p.evaluate(([s, g]) => window.__until(s, g), [sel, gone]);
    await loc.tap();
    const ms = await shown;
    await p.waitForTimeout(500); // the Event Timing entries come after the frame, and the page settles
    presented?.push(Math.round(await p.evaluate((x) => window.__presented(x), ms)));
    return Math.round(ms);
  };
  const taps = [];
  const backs = [];
  const history = [];
  const tapShown = [];
  const backShown = [];
  const openLong = [];
  for (let i = 0; i <= TAPS; i++) {
    const t = await now(p);
    taps.push(await tap(row, PAGE, LIST, tapShown));
    openLong.push(sum(await longs(p, t)).worst);
    backs.push(await tap(p.locator('[data-part="split.back"]'), LIST, PAGE, backShown));
  }
  // the browser's Back (history.back(): no input, so to the frame that shows the list)
  for (let i = 0; i < 3; i++) {
    await tap(row, PAGE, LIST);
    history.push(Math.round(await p.evaluate(([l, g]) => { const t0 = performance.now(); const shown = window.__until(l, g, t0); window.history.back(); return shown; }, [LIST, PAGE])));
    await p.waitForTimeout(500);
  }
  const warm = taps.slice(1);
  const back = backs.slice(1);
  out.open = {
    task: id,
    firstTapMs: taps[0],
    tapToPageMs: warm,
    tapP95: p95(warm),
    backTapMs: back,
    backP95: p95(back),
    presented: { tapP95: p95(tapShown.slice(1)), backP95: p95(backShown.slice(1)), tapMs: tapShown.slice(1), backMs: backShown.slice(1) },
    historyBackMs: history,
    worstLongTaskAfterTap: openLong.slice(1),
  };
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
const over = [out.open.tapP95 > BUDGET.tapMs && 'tap', out.open.backP95 > BUDGET.tapMs && 'Back'].filter(Boolean);
console.log(`\ntap → task page p95 ${out.open.tapP95} ms (first tap ${out.open.firstTapMs} ms), Back → list p95 ${out.open.backP95} ms, budget ${BUDGET.tapMs} ms over ${TAPS} each`);
console.log(`presented (Event Timing, INP's measure): tap p95 ${out.open.presented.tapP95} ms, Back p95 ${out.open.presented.backP95} ms: reported, not gated`);
console.log(`long tasks while streaming ${out.streaming.count} (worst ${out.streaming.worst} ms), budget ${BUDGET.longTasksWhileStreaming}: reported, not gated here`);
if (over.length) {
  console.error(`FAIL: ${over.join(' and ')} over ${BUDGET.tapMs} ms p95`);
  process.exit(1);
}
console.log('ok: tap and Back within budget');
