// Shared QA scaffolding: Playwright loaded from outside the repo (wisp adds no dependencies), the browser it
// drives, and a tiny scenario runner.
//
//   PW_DIR   a folder with playwright-core installed (default /tmp/wisp-qa-pw):
//            mkdir -p /tmp/wisp-qa-pw && npm i --prefix /tmp/wisp-qa-pw playwright-core
//   CHROME   Chrome to drive (default: the system Google Chrome)
//   QA_ENGINE=webkit  drive Playwright's WebKit instead (the Mac engine of the Tauri app):
//            cd /tmp/wisp-qa-pw && npx playwright-core install webkit
//   QA_ENGINE=webview2  drive the WebView2 of a running Tauri app on Windows, through its DevTools
//            port (QA_CDP)

import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = fileURLToPath(new URL('..', import.meta.url));
export const OUT = process.env.QA_OUT || path.join(ROOT, 'qa', 'out');
fs.mkdirSync(OUT, { recursive: true });

export const CHROME =
  process.env.CHROME ||
  ({ darwin: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', win32: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' }[process.platform] || '/usr/bin/google-chrome');

export function playwright() {
  const dir = process.env.PW_DIR || (process.platform === 'win32' ? path.join(os.tmpdir(), 'wisp-qa-pw') : '/tmp/wisp-qa-pw');
  try {
    return createRequire(path.join(dir, 'package.json'))('playwright-core');
  } catch {
    throw new Error(`playwright-core not found in ${dir}: npm i --prefix ${dir} playwright-core`);
  }
}

export const ENGINE = ['webkit', 'webview2'].includes(process.env.QA_ENGINE) ? process.env.QA_ENGINE : 'chrome';
export const launchBrowser = (opts = {}) => {
  const { chromium, webkit } = playwright();
  if (ENGINE === 'webview2') return chromium.connectOverCDP(process.env.QA_CDP);
  return ENGINE === 'webkit' ? webkit.launch(opts) : chromium.launch({ executablePath: CHROME, ...opts });
};

// A tiny scenario runner: each check is named after its scenario id in qa/scenarios/*.md.
export function runner(surface) {
  const results = [];
  const check = async (id, title, fn) => {
    const t0 = Date.now();
    try {
      await fn();
      results.push({ id, title, ok: true, ms: Date.now() - t0 });
      console.log(`  ✔ ${id} ${title}`);
    } catch (err) {
      results.push({ id, title, ok: false, ms: Date.now() - t0, error: String(err?.message || err).split('\n')[0] });
      console.log(`  ✗ ${id} ${title}\n      ${String(err?.message || err).split('\n')[0]}`);
    }
  };
  const done = () => {
    const failed = results.filter((r) => !r.ok);
    fs.writeFileSync(path.join(OUT, `${surface}.json`), JSON.stringify(results, null, 2));
    console.log(`\n  ${surface}: ${results.length - failed.length} passed, ${failed.length} failed · ${OUT}`);
    return failed.length ? 1 : 0;
  };
  return { check, done, results };
}

export const ok = (v, msg) => {
  if (!v) throw new Error(msg);
};
