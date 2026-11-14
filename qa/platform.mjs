// The phone apps' metric gate (ADR-0065 §5, design/mobile.md §A.4, §A.6, §A.8): each view as the Android and
// the iPhone app draw it (data-platform="android" | "ios" on <html>), in real Chrome as a touch screen with the
// device's safe-area insets, light and dark, text at 100 % and 200 %. It fails when
//
//   tokens        the look's token set isn't on the page (--tap-min, --bar-h, …)
//   tap-target    a control answers a touch on less than --tap-min × --tap-min (48 dp / 44 pt)
//   bar-height    an app bar, navigation bar, FAB or list row is off its token (bar-width: the rail)
//   safe-area     text or a control sits under the status bar, notch, home indicator or gesture bar
//   type-role     a text's size is none of the look's --type-* roles at that text scale
//   hover-only    a control is hidden until :hover (nothing hovers on a phone)
//   gesture-twin  a [data-gesture] names no visible twin, or its twin isn't on screen
//   layout:*      design/layout.md's rules (qa/layout.mjs inspect()): control text wraps, bar rows, overflow
//   look          data-platform is set to the other look
// The rules are pure functions in qa/platform-rules.ts (tests: tools/test/qa-platform.test.ts); the kit's side
// of the contract (data-kit names, data-gesture/data-twin) is listed at its top.
//
// A view the platform look doesn't reach yet is a skip, never a pass: "data-platform not set" (the shell didn't
// take the page for the app: build step 1) or "no platform look yet" (no kit variant under
// :root[data-platform=…] matches anything in the view: build steps 3–7).
//
//   node qa/platform.mjs                 starts apps/web on the mock peer (v1's seed) on a free port, checks the
//                                        views of qa/layout-areas.mjs `shell` as the phone apps, stops it
//   APP_URL=http://127.0.0.1:5180 node qa/platform.mjs     against an app already running
//   node qa/platform.mjs --target=gallery                 the kit gallery (?platform=android|ios) instead
//   PLATFORMS=ios SIZES=393x852,ipad THEMES=dark SCALES=200 VIEWS=home,task node qa/platform.mjs   a subset
//   PLATFORM_FORCE=1 node qa/platform.mjs     checks views with no platform look yet too: what's left to do
//
// The app takes itself for a phone app because the page gets a Capacitor global saying so (detectPlatform in
// packages/shell/src/platform), the way qa/feedback.mjs fakes the desktop app's __TAURI_INTERNALS__. Insets go
// through CDP Emulation.setSafeAreaInsetsOverride (env(safe-area-inset-*)); a Chrome without it gets the
// --safe-* tokens set instead, and the report says which. Text scale: --text-scale and the root font size
// (what the app's native side will set from Android's font scale / iOS Dynamic Type).
// That setup, the env and the view list are qa/phone.mjs + qa/phone-rules.ts, shared with qa/layout.mjs --phone
// and qa/feedback.mjs --phone, so the three agree on what a phone view is and when it is skipped.
//
// Needs playwright-core in PW_DIR (default /tmp/wisp-qa-pw) and the system Chrome (qa/lib.mjs).
// Writes $QA_OUT/platform.json (qa/out by default) and, per failing view, $QA_OUT/platform/<…>.png.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { launchBrowser, OUT, ROOT } from './lib.mjs';
import { inspect, startGallery } from './layout.mjs';
import { lookVariants, openPhone, phoneConfig, settlePhone } from './phone.mjs';
import { gate, phoneRuns, runLabel, tally } from './phone-rules.ts';
import { barRule, hoverReveals, hoverRule, lookStatus, rolesOf, safeRule, tapRule, tokenRule, tokensOf, TOKEN_NAMES, twinRule, typeRule } from './platform-rules.ts';

const arg = (k) => process.argv.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3);
const TARGET = arg('target') || 'app';
const SHOTS = path.join(OUT, 'platform');
const shown = (p) => (path.relative(ROOT, p).startsWith('..') ? p : path.relative(ROOT, p));

/** Runs in the page: what the rules need, measured. Self-contained (it is serialised into the page). */
export function measure(tokenNames) {
  const root = document.documentElement;
  const rs = getComputedStyle(root);
  const look = root.getAttribute('data-platform');
  const vars = {};
  for (const n of tokenNames) vars[n] = rs.getPropertyValue(n).trim();
  for (let i = 0; i < rs.length; i++) {
    const n = rs.item(i);
    if (n.startsWith('--type-')) vars[n] = rs.getPropertyValue(n).trim();
  }
  const vw = root.clientWidth;
  const vh = innerHeight;
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none' && Number(cs.opacity) > 0;
  };
  const name = (el) => {
    const p = el.closest('[data-part]')?.getAttribute('data-part');
    const k = el.getAttribute('data-kit');
    return k && p ? `${p} ${k}` : p || k || el.tagName.toLowerCase();
  };
  const onScreen = (r) => r.bottom > 0 && r.right > 0 && r.top < vh && r.left < vw;
  const inScroller = (el) => {
    for (let a = el.parentElement; a && a !== root; a = a.parentElement) {
      const o = getComputedStyle(a).overflowY;
      if ((o === 'auto' || o === 'scroll') && a.scrollHeight > a.clientHeight + 1) return true;
    }
    return false;
  };
  const placed = (el, part) => {
    const r = el.getBoundingClientRect();
    return { part, top: r.top, bottom: r.bottom, left: r.left, right: r.right, inScroller: inScroller(el) };
  };

  // :hover rules (the kit variants for the look are qa/phone.mjs lookVariants, run beside this)
  const hoverRules = [];
  // one block of declarations (a style rule's own, or Chrome's CSSNestedDeclarations) under its resolved selector
  const consider = (sel, style, media) => {
    if (!sel || !style || style.length === 0) return;
    if (sel.includes(':hover')) {
      const props = {};
      for (const p of ['opacity', 'visibility', 'display', 'pointer-events']) {
        const v = style.getPropertyValue(p);
        if (v) props[p] = v.trim();
      }
      const m = media.length ? { text: media.join(' and '), matches: media.every((q) => matchMedia(q).matches) } : undefined;
      hoverRules.push(m ? { selector: sel, props, media: m } : { selector: sel, props });
    }
  };
  const walk = (rules, media, parentSel) => {
    for (const rule of rules) {
      if (rule.styleSheet) {
        try {
          walk(rule.styleSheet.cssRules, media, parentSel);
        } catch {}
        continue;
      }
      if (rule.selectorText !== undefined) {
        const sel = parentSel ? (rule.selectorText.includes('&') ? rule.selectorText.replace(/&/g, `:is(${parentSel})`) : `:is(${parentSel}) ${rule.selectorText}`) : rule.selectorText;
        consider(sel, rule.style, media);
        if (rule.cssRules?.length) walk(rule.cssRules, media, sel);
        continue;
      }
      if (rule.style && !rule.cssRules) {
        consider(parentSel, rule.style, media); // CSSNestedDeclarations: declarations after a nested rule or under @media
        continue;
      }
      if (rule.cssRules) walk(rule.cssRules, rule.conditionText && rule.media ? [...media, rule.conditionText] : media, parentSel);
    }
  };
  for (const sheet of document.styleSheets) {
    try {
      walk(sheet.cssRules, [], '');
    } catch {
      // a cross-origin sheet: not the kit's
    }
  }

  // controls and their hit areas: a touch at a point lands on the control if elementFromPoint is it or inside it
  const INTERACTIVE = 'button, a[href], [role="button"], [role="tab"], [role="switch"], [role="menuitem"], [role="checkbox"], [role="radio"], input:not([type=hidden]), textarea, select, [tabindex]:not([tabindex="-1"])';
  const tapMin = Number.parseFloat(vars['--tap-min']) || 0;
  const targets = [];
  const place = [];
  const seen = new Set();
  for (const el of document.querySelectorAll(INTERACTIVE)) {
    if (seen.has(el) || !visible(el) || el.closest('[inert], [aria-hidden="true"]')) continue;
    if (el.disabled || el.getAttribute('aria-disabled') === 'true') continue;
    const r = el.getBoundingClientRect();
    if (!onScreen(r)) continue;
    const cx = Math.min(vw - 1, Math.max(0, r.x + r.width / 2));
    const cy = Math.min(vh - 1, Math.max(0, r.y + r.height / 2));
    const on = (x, y) => {
      const h = document.elementFromPoint(x, y);
      return !!h && (h === el || el.contains(h));
    };
    if (!on(cx, cy)) continue; // covered: not something a touch reaches here
    seen.add(el);
    const part = name(el);
    let hitW = r.width;
    let hitH = r.height;
    if (tapMin && (r.width < tapMin || r.height < tapMin)) {
      const reach = (dx, dy) => {
        let n = 0;
        while (n < tapMin && on(cx + dx * (n + 1), cy + dy * (n + 1))) n++;
        return n;
      };
      hitW = Math.max(r.width, reach(-1, 0) + reach(1, 0) + 1);
      hitH = Math.max(r.height, reach(0, -1) + reach(0, 1) + 1);
    }
    const cs = getComputedStyle(el);
    const inline = el.tagName === 'A' && cs.display === 'inline' && (el.parentElement?.textContent ?? '').trim().length > (el.textContent ?? '').trim().length;
    targets.push({ part, hitW, hitH, inline });
    place.push(placed(el, part));
  }

  // text: every element with its own text, in the type roles' terms
  const texts = [];
  const textEls = new Set();
  const tw = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n = tw.nextNode(); n; n = tw.nextNode()) {
    const el = n.parentElement;
    if (!el || textEls.has(el) || !(n.textContent ?? '').trim()) continue;
    if (el.closest('code, pre, kbd, samp, textarea, select, svg, [aria-hidden="true"], [inert]')) continue;
    const cs = getComputedStyle(el);
    if (/mono|symbols|icons/i.test(cs.fontFamily) || !visible(el)) continue;
    const r = el.getBoundingClientRect();
    if (!onScreen(r)) continue;
    textEls.add(el);
    const part = name(el);
    texts.push({ part, size: Math.round(Number.parseFloat(cs.fontSize) * 100) / 100, sample: (n.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 30) });
    place.push(placed(el, part));
  }

  const bars = [];
  for (const [sel, kind] of [
    // the kit's markup (packages/ui/src/nav): TopAppBar, NavigationBar (a rail when it stands upright), Fab
    ['[data-kit="app-bar"]', (el) => (el.hasAttribute('data-large') && !el.hasAttribute('data-collapsed') ? 'app-bar-large' : 'app-bar')],
    ['[data-kit="nav-bar"]', (el) => (el.getBoundingClientRect().height > el.getBoundingClientRect().width ? 'navigation-rail' : 'navigation-bar')],
    ['[data-kit="fab"]', (el) => (el.hasAttribute('data-extended') ? 'fab-extended' : 'fab')],
    ['[data-kit="list-row"]', () => 'list-row'],
  ]) {
    for (const el of document.querySelectorAll(sel)) {
      if (!visible(el)) continue;
      const r = el.getBoundingClientRect();
      if (!onScreen(r)) continue;
      bars.push({ kind: kind(el), part: name(el), top: r.top, bottom: r.bottom, left: r.left, width: r.width, height: r.height });
    }
  }

  const gestures = [...document.querySelectorAll('[data-gesture]')]
    .filter(visible)
    .map((el) => ({ part: name(el), gesture: el.getAttribute('data-gesture') ?? '', twins: (el.getAttribute('data-twin') ?? '').split(/\s+/).filter(Boolean) }));
  const parts = [...new Set([...document.querySelectorAll('[data-part]')].filter(visible).map((el) => el.getAttribute('data-part')))];

  return { look, vars, targets, texts, placed: place, bars, hoverRules, gestures, parts, viewport: { width: vw, height: vh } };
}

/** Runs in the page: what each :hover rule's base selector finds, hidden now and holding a control, and the
 * selectors the page couldn't parse (reported, so a hover rule is never silently left unchecked). */
export function hoverHits(sels) {
  const INTERACTIVE = 'button, a[href], [role="button"], input, textarea, select, [tabindex]:not([tabindex="-1"])';
  const hidden = (el) => {
    const cs = getComputedStyle(el);
    return Number(cs.opacity) === 0 || cs.visibility === 'hidden' || cs.display === 'none' || cs.pointerEvents === 'none';
  };
  const out = [];
  const unparsed = [];
  for (const selector of sels) {
    let els = [];
    try {
      els = [...document.querySelectorAll(selector)].slice(0, 20);
    } catch {
      unparsed.push(selector);
      continue;
    }
    for (const el of els) {
      const part = el.closest('[data-part]')?.getAttribute('data-part') || el.getAttribute('data-kit') || el.tagName.toLowerCase();
      out.push({ part, selector, hidden: hidden(el), interactive: el.matches(INTERACTIVE) || !!el.querySelector(INTERACTIVE) });
    }
  }
  return { hits: out, unparsed };
}

/** Applies every rule to one measured view (the browser-free half; the tests drive the rules directly). */
export function judge(m, look, size, scale, layoutFound, force = false) {
  const g = gate(lookStatus(look, m.look, m.variants, m.matched), force);
  if (!g.run) return 'skip' in g ? { status: 'skip', skip: g.skip, findings: [] } : { status: 'fail', findings: [g.fail] };
  const tokens = tokensOf(m.vars);
  const findings = [
    ...tokenRule(look, tokens),
    ...tapRule(m.targets, tokens.tapMin),
    ...barRule(m.bars, tokens, size.insets, m.viewport),
    ...safeRule(m.placed, size.insets, m.viewport),
    ...typeRule(m.texts, rolesOf(m.vars), scale / 100),
    ...hoverRule(m.hoverHits ?? []),
    ...twinRule(m.gestures, new Set(m.parts)),
    ...layoutFound.map((f) => ({ ...f, rule: `layout:${f.rule}` })),
  ];
  return { status: findings.length ? 'fail' : 'pass', findings };
}

async function main() {
  fs.mkdirSync(SHOTS, { recursive: true });
  const gallery = TARGET === 'gallery';
  // the gallery is one page: VIEWS names the app's views, so it doesn't apply there
  const env = phoneConfig(gallery ? { ...process.env, VIEWS: undefined } : process.env);
  const { sizes, themes, scales, force } = env;
  const fixed = gallery ? process.env.GALLERY_URL : process.env.APP_URL;
  const server = fixed ? { url: fixed, stop: () => {} } : await startGallery(gallery ? undefined : 'shell');
  const views = gallery ? [{ name: 'gallery', path: '/', ready: 'body' }] : env.viewList;
  const results = [];
  let insetsBy = 'cdp';
  let browser;
  try {
    browser = await launchBrowser();
    for (const { size, theme, scale, view } of phoneRuns(sizes, themes, scales, views)) {
      const { ctx, page, insetsBy: by } = await openPhone(browser, size, { theme });
      if (by !== 'cdp') insetsBy = by;
      const errors = [];
      page.on('pageerror', (e) => errors.push(e.message));
      const url = gallery ? `${server.url}/?theme=${theme}&platform=${size.look}` : `${server.url}${view.path}`;
      await page.goto(url, { waitUntil: 'load' });
      await page.waitForSelector(view.ready, { timeout: 15_000 }).catch(() => errors.push(`${view.ready} never showed`));
      await settlePhone(page, scale, errors);
      const m = { ...(await page.evaluate(measure, Object.values(TOKEN_NAMES))), ...(await page.evaluate(lookVariants)) };
      const hover = await page.evaluate(hoverHits, [...new Set(m.hoverRules.flatMap(hoverReveals))]);
      m.hoverHits = hover.hits;
      const r = judge(m, size.look, size, scale, m.look === size.look ? await page.evaluate(inspect) : [], force);
      if (r.status !== 'skip' && hover.unparsed.length) r.unchecked = hover.unparsed;
      for (const e of errors) r.findings.push({ rule: 'page-error', part: 'page', detail: e });
      if (r.status !== 'fail' && errors.length) r.status = 'fail';
      const row = { look: size.look, size: `${size.width}x${size.height}`, theme, scale, view: view.name, ...r };
      results.push(row);
      if (r.status === 'fail') await page.screenshot({ path: path.join(SHOTS, `${size.look}-${row.size}-${theme}-${scale}-${view.name}.png`) });
      await ctx.close();
    }
  } finally {
    try {
      await browser?.close();
    } finally {
      server.stop();
    }
  }
  const t = tally(results);
  const summary = { checked: results.length, forced: force, pass: t.pass, fail: t.fail, skip: t.skip, insets: insetsBy, target: TARGET, results };
  fs.writeFileSync(path.join(OUT, 'platform.json'), `${JSON.stringify(summary, null, 2)}\n`);
  for (const r of results) {
    const head = `${r.status === 'pass' ? '✔' : r.status === 'fail' ? '✗' : '–'} ${runLabel(r.look, { width: Number(r.size.split('x')[0]), height: Number(r.size.split('x')[1]) }, r.theme, r.scale, r.view)}`;
    console.log(r.status === 'skip' ? `${head}: skipped, ${r.skip}` : head);
    for (const f of r.findings) console.log(`    ${f.rule} ${f.part}: ${f.detail}`);
    for (const u of r.unchecked ?? []) console.log(`    (hover rule not checked, the page can't parse it: ${u})`);
  }
  console.log(
    `\nplatform (${TARGET}): ${summary.pass} pass, ${summary.fail} fail, ${summary.skip} skipped of ${results.length} (${views.length} view${views.length === 1 ? '' : 's'} × ${sizes.length} sizes × ${themes.length} themes × ${scales.length} text scales); insets by ${insetsBy === 'cdp' ? 'CDP' : 'the --safe-* tokens (this Chrome has no inset override)'}`,
  );
  for (const s of t.reasons) console.log(`  skipped: ${s}`);
  console.log(`  ${shown(path.join(OUT, 'platform.json'))}${summary.fail ? `; screenshots in ${shown(SHOTS)}` : ''}`);
  process.exitCode = summary.fail ? 1 : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
