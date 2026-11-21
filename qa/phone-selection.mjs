// Selection mode doesn't move the list (AGENTS.md "Nothing jumps", design/mobile.md §A.4 Selection mode): on the phone
// apps' Tasks list, entering selection (the bar's Select) and leaving it (iOS's Done, Android's ×) keep the first row
// at the same y. iOS's edit mode keeps the large title (or the folded title, scrolled) and turns the bar's buttons into
// Select All / Done; Android's contextual bar replaces the top app bar at its height.
//
//   APP_URL=http://127.0.0.1:5180 node qa/phone-selection.mjs     against `npm run dev:web` (the mock peer)
//   SHOTS=target/r294 …                                            also writes before/after screenshots there
//
// Needs playwright-core in PW_DIR (default /tmp/wisp-qa-pw) and the system Chrome (qa/lib.mjs).

import fs from 'node:fs';
import path from 'node:path';
import { launchBrowser, ok, runner } from './lib.mjs';
import { openPhone, settlePhone } from './phone.mjs';

const APP = process.env.APP_URL || 'http://127.0.0.1:5180';
const SHOTS = process.env.SHOTS;
if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true });
const SIZES = [
  { look: 'ios', width: 390, height: 844, insets: { top: 47, right: 0, bottom: 34, left: 0 } },
  { look: 'android', width: 390, height: 844, insets: { top: 24, right: 0, bottom: 24, left: 0 } },
];
const ROW = '[data-pane="list"] [data-part="card"][data-id]';

const { check, done } = runner('phone-selection');
const browser = await launchBrowser({ headless: true });

/** the first visible row's top, rounded to the device pixel */
const firstRowY = (page) =>
  page.evaluate((sel) => {
    const r = [...document.querySelectorAll(sel)].map((e) => e.getBoundingClientRect()).find((b) => b.height > 0 && b.bottom > 0);
    return r ? Math.round(r.top * 2) / 2 : NaN;
  }, ROW);

/** the bar's Select: a visible button, or the bar's More menu's item */
async function enterSelection(page) {
  const byName = page.locator('[data-part="phone.tasks.bar"] button[aria-label="Select"]:visible').first();
  const byText = page.locator('[data-part="phone.tasks.bar"] button:visible', { hasText: /^Select$/ }).first();
  if (await byName.count()) await byName.tap();
  else if (await byText.count()) await byText.tap();
  else {
    await page.locator('[data-part="phone.tasks.bar"] [aria-label="More"]:visible').first().tap();
    await page.locator('[data-kit="menu-item"]:visible, [data-kit="sheet-item"]:visible', { hasText: /^Select$/ }).first().tap();
  }
  await page.waitForSelector('[data-kit="list-row"][data-picking]');
  await page.waitForTimeout(400); // past any transition
}

async function leaveSelection(page, look) {
  if (look === 'ios') await page.locator('[data-kit="app-bar"] [data-kit="app-bar-confirm"] button:visible', { hasText: 'Done' }).first().tap();
  else await page.locator('[data-kit="app-bar"][data-kind="select"] [aria-label="Close selection"]:visible').first().tap();
  await page.waitForFunction(() => !document.querySelector('[data-kit="list-row"][data-picking]'));
  await page.waitForTimeout(400);
}

const shot = async (page, name) => SHOTS && page.screenshot({ path: path.join(SHOTS, `${name}.png`) });

for (const size of SIZES) {
  for (const scrolled of [false, true]) {
    const name = `${size.look}-${size.width}${scrolled ? '-scrolled' : ''}`;
    await check(name, `first_row_y_is_the_same_before_and_after_entering_selection (${size.look}${scrolled ? ', scrolled: folded title' : ''})`, async () => {
      const { ctx, page } = await openPhone(browser, size);
      try {
        await page.goto(`${APP}/`, { waitUntil: 'load' });
        await page.waitForSelector(ROW);
        await settlePhone(page, 100, []);
        if (scrolled) {
          // scroll the list so the large title folds (iOS) and the first visible row sits mid-screen
          await page.evaluate((sel) => {
            const row = document.querySelector(sel);
            let box = row?.parentElement;
            while (box && box.scrollHeight <= box.clientHeight) box = box.parentElement;
            if (box) box.scrollTop = 120;
          }, ROW);
          await page.waitForTimeout(400);
        }
        const before = await firstRowY(page);
        await shot(page, `${name}-before`);
        await enterSelection(page);
        const during = await firstRowY(page);
        await shot(page, `${name}-selecting`);
        await leaveSelection(page, size.look);
        const after = await firstRowY(page);
        ok(Number.isFinite(before), 'no row on screen');
        ok(during === before, `entering selection moved the first row: ${before} → ${during}`);
        ok(after === before, `leaving selection moved the first row: ${before} → ${after}`);
      } finally {
        await ctx.close();
      }
    });
  }
}

await browser.close();
process.exit(done());
