// The browser half of a phone view (ADR-0065 §5), shared by qa/platform.mjs, qa/layout.mjs --phone and
// qa/feedback.mjs --phone so all three open the phone app the same way:
//
//   openPhone     a touch-screen context at the device's size, the fake Capacitor bridge that makes the shell
//                 take the page for the Android or iPhone app (detectPlatform, packages/shell/src/platform), and
//                 the device's safe-area insets (CDP, or the --safe-* tokens on a Chrome without the override)
//   settlePhone   after load: the text scale (--text-scale and the root font size, what the native side sets
//                 from Android's font scale / iOS Dynamic Type), fonts, and the look's lazily loaded stylesheet
//   phoneGate     whether the view has a platform look to check (lookVariants + lookStatus + gate): a skip with
//                 its reason, never a pass
//
// The pure half (env, views, the run matrix, the gate) is qa/phone-rules.ts.

import { AREAS } from './layout-areas.mjs';
import { gate, phoneEnv, phoneViews } from './phone-rules.ts';
import { lookStatus } from './platform-rules.ts';

/** The phone views: the app's `shell` area paths (qa/layout-areas.mjs), the list all three scripts check. */
export const PHONE_VIEWS = AREAS.shell;

/** The env of a phone run plus its views, read once (throws on a value that picks nothing). */
export function phoneConfig(env = process.env, defaults = {}) {
  const e = phoneEnv(env, defaults);
  return { ...e, viewList: phoneViews(PHONE_VIEWS, e.views) };
}

/** A context and page as the phone app: nothing navigated yet. `insetsBy` says how the insets got there. */
export async function openPhone(browser, size, { theme = 'light', reducedMotion = 'reduce', deviceScaleFactor = 2 } = {}) {
  const ctx = await browser.newContext({ viewport: { width: size.width, height: size.height }, deviceScaleFactor, isMobile: true, hasTouch: true, colorScheme: theme === 'dark' ? 'dark' : 'light', reducedMotion });
  // the shell's detectPlatform takes a native Capacitor bridge for the phone app (ADR-0065 §1)
  await ctx.addInitScript((look) => {
    window.Capacitor = { isNativePlatform: () => true, getPlatform: () => look, Plugins: {} };
  }, size.look);
  const page = await ctx.newPage();
  const { top, right, bottom, left } = size.insets;
  let insetsBy = 'cdp';
  try {
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: { top, topMax: top, right, rightMax: right, bottom, bottomMax: bottom, left, leftMax: left } });
  } catch {
    insetsBy = 'tokens';
    await ctx.addInitScript((i) => {
      const set = () => Object.entries(i).forEach(([k, v]) => document.documentElement.style.setProperty(`--safe-${k}`, `${v}px`));
      if (document.documentElement) set();
      else document.addEventListener('DOMContentLoaded', set);
    }, size.insets);
  }
  return { ctx, page, insetsBy };
}

/** After the view drew: the text scale, the fonts, and (once the shell set data-platform) the look's stylesheet,
 * waited for by its tokens. Problems go to `errors`. */
export async function settlePhone(page, scale, errors) {
  await page.evaluate((s) => {
    const r = document.documentElement;
    r.style.setProperty('--text-scale', String(s / 100));
    r.style.fontSize = `${s}%`;
    r.setAttribute('data-text-scale', String(s));
  }, scale);
  await page.evaluate(() => document.fonts.ready);
  const looked = await page.evaluate(() => document.documentElement.hasAttribute('data-platform'));
  if (looked)
    await page
      .waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue('--tap-min').trim() !== '', null, { timeout: 10_000 })
      .catch(() => errors.push('the platform stylesheet never loaded: --tap-min unset after 10 s'));
  await page.waitForTimeout(300); // the Toolbar's ResizeObserver settles its folding
}

/** Runs in the page (self-contained, it is serialised): <html data-platform>, the kit variants for that look (rules
 * under :root[data-platform=<look>] that set more than custom properties) and how many of them draw something
 * visible here. Every rule is read with its nesting resolved (& → :is(parent)) and the platform attribute replaced
 * by :root, so `:root[data-platform=ios] .list { & .row {…} }` is the variant `:is(:root .list) .row`. */
export function lookVariants() {
  const root = document.documentElement;
  const look = root.getAttribute('data-platform');
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none' && Number(cs.opacity) > 0;
  };
  const PLATFORM = /(?::root)?\[data-platform=(['"]?)(android|ios)\1\]/g;
  const STATE = /::?(?:hover|active|focus-visible|focus-within|focus|visited|before|after|placeholder|marker|selection|backdrop|-webkit-[a-z-]+)(?![a-z-])/g;
  const splitTop = (sel) => {
    const out = [];
    let depth = 0;
    let cur = '';
    for (const ch of sel) {
      if (ch === '(' || ch === '[') depth++;
      else if (ch === ')' || ch === ']') depth--;
      if (ch === ',' && depth === 0) {
        out.push(cur.trim());
        cur = '';
      } else cur += ch;
    }
    if (cur.trim()) out.push(cur.trim());
    return out;
  };
  const variantSels = [];
  const consider = (sel, style) => {
    if (!sel || !style || style.length === 0) return;
    if (![...style].some((p) => !p.startsWith('--'))) return; // custom properties only: the token set (dark/light too)
    for (const part of splitTop(sel)) {
      if (![...part.matchAll(PLATFORM)].some((m) => m[2] === look)) continue;
      const rest = part.replace(PLATFORM, ':root').trim();
      if (rest.replace(STATE, '').trim() !== ':root') variantSels.push(rest); // the root itself is page-wide plumbing
    }
  };
  const walk = (rules, parentSel) => {
    for (const rule of rules) {
      if (rule.styleSheet) {
        try {
          walk(rule.styleSheet.cssRules, parentSel);
        } catch {}
        continue;
      }
      if (rule.selectorText !== undefined) {
        const sel = parentSel ? (rule.selectorText.includes('&') ? rule.selectorText.replace(/&/g, `:is(${parentSel})`) : `:is(${parentSel}) ${rule.selectorText}`) : rule.selectorText;
        consider(sel, rule.style);
        if (rule.cssRules?.length) walk(rule.cssRules, sel);
        continue;
      }
      if (rule.style && !rule.cssRules) {
        consider(parentSel, rule.style); // CSSNestedDeclarations: declarations after a nested rule or under @media
        continue;
      }
      if (rule.cssRules) walk(rule.cssRules, parentSel);
    }
  };
  if (look)
    for (const sheet of document.styleSheets) {
      try {
        walk(sheet.cssRules, '');
      } catch {
        // a cross-origin sheet: not the kit's
      }
    }
  let matched = 0;
  const unique = new Set(variantSels);
  for (const s of unique) {
    try {
      if ([...document.querySelectorAll(s.replace(STATE, '').trim())].some(visible)) matched++;
    } catch {
      // a selector querySelector can't take once its states are stripped
    }
  }
  return { look, variants: unique.size, matched };
}

/** Whether this view is checked as the `look` app: { run, forced } | { run: false, skip } | { run: false, fail }. */
export async function phoneGate(page, look, force) {
  const v = await page.evaluate(lookVariants);
  return { ...gate(lookStatus(look, v.look, v.variants, v.matched), force), ...v };
}
