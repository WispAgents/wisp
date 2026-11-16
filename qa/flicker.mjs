// The flicker gate (B12, design/feedback.md "nothing jumps"): an action never shows something that is gone again a moment
// later. For each action below, on apps/web's mock peer, at 1440 and 390 px, a MutationObserver in the page notes every
// element added; each frame marks the ones the user could see (connected, `checkVisibility()`, with text); an element
// seen and then removed within 1 s is a flicker: a node taken down and put back (its hover, focus and entry animation
// lost: B31, B32), or something that appeared and went (the owner's "something appear then disappear"). Hidden menus and
// positioners are never seen, so they never count. It also reports each action's first change after the input event
// (design/feedback.md: within 100 ms; a DOM change or focus moving) and fails one that changed nothing.
//
//   node qa/flicker.mjs                         starts apps/web's vite (the mock peer) on 4593; both widths
//   FLICKER_URL=http://127.0.0.1:5181 node qa/flicker.mjs   a vite already running (npm run dev:web's mock)
//   --widths=1440 --only=follow-up              a subset
// Results: qa/out/flicker/summary.json; exit 1 on a flicker or an action that changed nothing.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { launchBrowser, OUT, ROOT } from './lib.mjs';

const arg = (k) => process.argv.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3);
const WIDTHS = (arg('widths') ?? '1440,390').split(',').map(Number);
const ONLY = arg('only');
const DIR = path.join(OUT, 'flicker');
fs.mkdirSync(DIR, { recursive: true });

async function startWeb() {
  if (process.env.FLICKER_URL) return { url: process.env.FLICKER_URL.replace(/\/?$/, '/'), stop: () => {} };
  const port = Number(process.env.QA_FLICKER_PORT) || 4593;
  const url = `http://127.0.0.1:${port}/`;
  if (await fetch(url).then(() => true, () => false)) throw new Error(`port ${port} is taken: lsof -ti tcp:${port} -sTCP:LISTEN | xargs kill`);
  const log = fs.openSync(path.join(DIR, `vite-${port}.log`), 'w');
  const child = spawn(process.execPath, [path.join(ROOT, 'node_modules/vite/bin/vite.js'), '--port', String(port), '--strictPort'], {
    cwd: path.join(ROOT, 'apps/web'),
    env: { ...process.env, PORT: String(port) },
    stdio: ['ignore', log, log],
  });
  for (let i = 0; i < 60; i++) {
    if (await fetch(url).then((r) => r.ok, () => false)) return { url, stop: () => child.kill('SIGTERM') };
    await new Promise((r) => setTimeout(r, 500));
  }
  child.kill('SIGTERM');
  throw new Error(`apps/web did not start on ${port}`);
}

// In the page: watch additions, mark what was seen, report what was seen and gone within 1 s.
function watch() {
  const w = (window.__flicker = { born: new Map(), seen: new Set(), out: [], armed: 0, want: false, first: null });
  new MutationObserver((ms) => {
    const t = performance.now();
    if (w.armed && !w.want && w.first === null) w.first = t - w.armed;
    for (const m of ms) {
      for (const n of m.addedNodes) if (n.nodeType === 1) w.born.set(n, t);
      for (const n of m.removedNodes) {
        if (n.nodeType !== 1 || !w.seen.has(n)) continue;
        const life = t - (w.born.get(n) ?? t);
        const text = (n.textContent || '').replace(/\s+/g, ' ').trim();
        if (life > 17 && life < 1000 && text) w.out.push({ kit: n.getAttribute('data-kit') || n.tagName.toLowerCase(), part: n.getAttribute('data-part') || '', life: Math.round(life), text: text.slice(0, 70) });
        w.seen.delete(n);
        w.born.delete(n);
      }
    }
  }).observe(document.body, { childList: true, subtree: true, attributes: true, characterData: true });
  const frame = () => {
    for (const n of w.born.keys()) if (!w.seen.has(n) && n.isConnected && n.checkVisibility?.({ opacityProperty: true, visibilityProperty: true }) && (n.textContent || '').trim()) w.seen.add(n);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  for (const ev of ['pointerdown', 'keydown']) addEventListener(ev, () => { if (w.want) { w.armed = performance.now(); w.first = null; w.want = false; } }, true);
  // focus moving is a change too (N puts the caret in New task's box), as qa/feedback.mjs counts it
  addEventListener('focusin', () => { if (w.armed && !w.want && w.first === null) w.first = performance.now() - w.armed; }, true);
}

// Each action: what it does. Steps that don't apply at a width (no list beside a task on a phone) say so.
const ACTIONS = [
  ['open a task', async (p) => p.locator('[data-kit="feed-item"]').first().click()],
  ['open another task', async (p) => p.locator('[data-kit="feed-item"]').nth(1).click(), { desk: true }],
  ['Transcript (V)', async (p) => (await blur(p), p.keyboard.press('v'))],
  ['Transcript back (V)', async (p) => (await blur(p), p.keyboard.press('v'))],
  ['Mark done (E)', async (p) => (await blur(p), p.keyboard.press('e'))],
  ['open the next task', async (p) => p.locator('[data-kit="feed-item"]').nth(2).click(), { desk: true }],
  ['send a follow-up', async (p) => {
    const box = p.locator('[data-kit="reply"]').first();
    await box.click();
    await box.fill('QA flicker follow-up');
    await p.keyboard.press('Enter');
  }],
  ['back home (Esc twice)', async (p) => (await blur(p), p.keyboard.press('Escape'))],
  ['New task (N)', async (p) => (await blur(p), p.keyboard.press('n')), { desk: true }],
  ['send a New task', async (p) => {
    await p.locator('[data-kit="launcher"] textarea').first().fill('QA flicker new task');
    await p.keyboard.press('Enter');
  }, { desk: true }],
  ['palette (⌘K)', async (p) => p.keyboard.press('Meta+k')],
  ['palette closes', async (p) => p.keyboard.press('Escape')],
  ['Settings', async (p) => {
    await p.locator('[data-part="sidebar.foot"]').first().click();
    await p.locator('[role=menu]:visible [role=menuitem]').filter({ hasText: /Settings/ }).first().click();
  }, { desk: true }],
  ['a Settings pane', async (p) => p.locator('[data-part~="settings.nav"] *').filter({ hasText: /^Usage$/ }).first().click(), { desk: true }],
  ['Settings closes', async (p) => p.keyboard.press('Escape'), { desk: true }],
  ['a crew member', async (p) => p.locator('[data-part="crew.rail.face"]').first().click(), { desk: true }],
  ['All tasks', async (p) => p.locator('[data-part="crew.rail.all"]').first().click(), { desk: true }],
  ['Add a crew member', async (p) => p.locator('[data-part="crew.add"]').first().click()],
  ['the crew editor closes', async (p) => p.keyboard.press('Escape')],
  ['My Stuff', async (p) => p.locator('[data-part="stuff.button"]').first().click(), { desk: true }],
  ['home', async (p) => p.keyboard.press('Escape'), { desk: true }],
  ['archive from the row menu', async (p) => {
    await p.locator('[data-kit="feed-item"]').nth(1).click({ button: 'right' });
    await p.locator('[role=menu]:visible [role=menuitem]').filter({ hasText: /Archive/ }).first().click();
  }],
  ['the Done section', async (p) => p.getByText(/^Done$/).first().click()],
  ['the Done section folds', async (p) => p.getByText(/^Done$/).first().click()],
];
const blur = (p) => p.evaluate(() => document.activeElement?.blur?.());

const web = await startWeb();
const browser = await launchBrowser();
const results = [];
let failed = 0;
try {
  for (const width of WIDTHS) {
    await fetch(`${web.url}__mock/reset`, { method: 'POST' }).catch(() => undefined);
    const page = await browser.newPage({ viewport: { width, height: width < 600 ? 844 : 900 } });
    page.on('pageerror', (e) => console.log(`  page error: ${e.message.slice(0, 160)}`));
    await page.goto(web.url, { waitUntil: 'load' });
    await page.waitForSelector('[data-kit="feed-item"]', { timeout: 30_000 });
    await page.waitForTimeout(1000);
    await page.evaluate(watch);
    console.log(`\n${width} px`);
    for (const [name, act, o = {}] of ACTIONS) {
      if (ONLY && !name.includes(ONLY)) continue;
      if (o.desk && width < 600) continue;
      await page.evaluate(() => { const w = window.__flicker; w.out = []; w.first = null; w.armed = performance.now(); w.want = true; });
      const r = { width, name };
      try {
        await act(page);
      } catch (e) {
        r.error = String(e.message).split('\n')[0].slice(0, 160);
      }
      await page.waitForTimeout(1500);
      Object.assign(r, await page.evaluate(() => ({ first: window.__flicker.first, flicker: window.__flicker.out })));
      const uniq = [...new Map(r.flicker.map((f) => [`${f.kit}|${f.part}|${f.text}`, f])).values()];
      r.flicker = uniq;
      const bad = uniq.length > 0 || r.first === null || r.error;
      failed += bad ? 1 : 0;
      console.log(`${bad ? '✗' : '✓'} ${name}: ${r.error ?? (r.first === null ? 'nothing changed' : `first change ${Math.round(r.first)} ms`)}`);
      for (const f of uniq) console.log(`    seen ${f.life} ms then gone: [${f.kit}] ${f.part} “${f.text}”`);
      results.push(r);
    }
    await page.screenshot({ path: path.join(DIR, `end-${width}.png`) });
    await page.close();
  }
} finally {
  await browser.close();
  web.stop();
}
fs.writeFileSync(path.join(DIR, 'summary.json'), JSON.stringify(results, null, 2));
console.log(`\n${results.length - failed} of ${results.length} actions clean; ${path.join(DIR, 'summary.json')}`);
process.exit(failed ? 1 : 0);
