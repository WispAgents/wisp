// The v1 visual spec's shape (design/foundation.md §1): the widths, themes and states design/v1-reference/ was captured
// at, the structural selectors beside v1's PARTS (qa/v1-parts.mjs), and the in-page collector of boxes and computed
// styles. qa/parity.mjs measures v2 with the same ones. design/v1-reference/ is frozen: v1 left the repo on
// 2026-11-27, and the capture run (a seeded v1 shot at every width, theme and state) is this file at a7c98f73.

export const WIDTHS = [[1440, 900], [1024, 768], [900, 900], [600, 900], [390, 844], [320, 640]];
export const THEMES = ['light', 'dark'];
// To keep the PNGs near 40 MB, the base screens get all six widths, and their variants (a menu, picker,
// panel or dialog opened over one of them) only the widths where the layout changes: desktop, the split
// point, phone and the smallest phone (1440, 900, 390, 320).
export const FULL_WIDTH_VIEWS = new Set(['home', 'task', 'task.composer', 'crew', 'stuff', 'palette', 'me-menu', 'projects', 'settings.general']);
export const VARIANT_WIDTHS = [1440, 900, 390, 320];
export const widthsFor = (view) => (FULL_WIDTH_VIEWS.has(view) ? WIDTHS : WIDTHS.filter(([w]) => VARIANT_WIDTHS.includes(w)));

// Structural elements beside the PARTS names, so the kit gets numbers for controls no spell names.
export const STRUCT = {
  'x.button': 'button',
  'x.input': 'input:not([type=hidden]), textarea, select',
  'x.menu': '.menu',
  'x.menuItem': '.menu [data-a], .menu button, .menu [role=menuitem]',
  'x.dialog': '[role=dialog], dialog[open], .modal, .sheet',
  'x.tab': '[role=tab]',
  'x.pill': '.pill',
  'x.heading': 'h1, h2, h3, h4',
  'x.row': '#feed .item[data-id]',
  'x.kbd': 'kbd, .kbd',
};

// The three elements captured in more than their resting state (foundation §1), on the view that shows
// them. `selected` has no separate shot: the list row's selected state is the open row in `task`, the
// composer's selected toolbar item is `task.composer-goal`, and the open task head is always selected.
export const STATES = {
  // A v1 row has no DOM focus (its quick actions are aria-hidden spans). Its keyboard state is the
  // `.item.focused` highlight that j/k and Esc out of a task leave (feed.css, split.css); j also opens the
  // task, so the class is set directly on the second row.
  home: {
    what: 'list row',
    target: '#feed .item[data-id]:not(.open) .it-head',
    selected: 'task',
    focus: {
      target: '#feed .item.focused',
      run: (p) => p.evaluate(() => document.querySelectorAll('#feed .item[data-id]:not(.open)')[1]?.classList.add('focused')),
    },
  },
  task: { what: 'task head', target: '#splitHead button:visible, .item.open .it-actions button:visible', selected: 'task (rest)' },
  'task.composer': { what: 'composer toolbar', target: '.composer .c-mode:visible, .composer button:visible', selected: 'task.composer-goal' },
};

// Runs in the page: every visible element matching each selector, with its box and computed style.
// Values are kept as Chrome reports them (px strings, rgb()), with zero/none defaults dropped.
export function collectInPage({ sels, scope, max, exact = false }) {
  const root = scope ? document.querySelector(scope) : document;
  const out = [];
  if (!root) return out;
  const pick = (cs) => {
    const s = {};
    const four = (p) => [cs[`${p}Top`], cs[`${p}Right`], cs[`${p}Bottom`], cs[`${p}Left`]].join(' ');
    const pad = four('padding');
    if (pad !== '0px 0px 0px 0px') s.pad = pad;
    const mar = four('margin');
    if (mar !== '0px 0px 0px 0px') s.mar = mar;
    if (cs.rowGap !== 'normal' || cs.columnGap !== 'normal') s.gap = `${cs.rowGap} ${cs.columnGap}`;
    s.fs = cs.fontSize;
    s.lh = cs.lineHeight;
    s.fw = cs.fontWeight;
    s.ff = cs.fontFamily;
    s.col = cs.color;
    if (cs.backgroundColor !== 'rgba(0, 0, 0, 0)') s.bg = cs.backgroundColor;
    if (cs.backgroundImage !== 'none') s.bgi = cs.backgroundImage.slice(0, 200);
    const bw = [cs.borderTopWidth, cs.borderRightWidth, cs.borderBottomWidth, cs.borderLeftWidth].join(' ');
    if (bw !== '0px 0px 0px 0px') {
      s.bw = bw;
      s.bc = [cs.borderTopColor, cs.borderRightColor, cs.borderBottomColor, cs.borderLeftColor].join(' ');
    }
    const rad = cs.borderRadius;
    if (rad !== '0px') s.rad = rad;
    if (cs.boxShadow !== 'none') s.sh = cs.boxShadow;
    if (cs.outlineStyle !== 'none' && cs.outlineWidth !== '0px') s.ol = `${cs.outlineWidth} ${cs.outlineStyle} ${cs.outlineColor} ${cs.outlineOffset}`;
    if (cs.opacity !== '1') s.op = cs.opacity;
    s.disp = cs.display;
    if (cs.whiteSpace !== 'normal') s.ws = cs.whiteSpace;
    return s;
  };
  for (const [part, sel] of Object.entries(sels)) {
    let els;
    try {
      els = [...root.querySelectorAll(sel)];
      if (scope && root.matches?.(sel)) els.unshift(root);
    } catch {
      continue; // a selector this engine can't parse
    }
    let i = 0;
    for (const el of els) {
      if (i >= (part === 'x.button' ? max * 5 : max)) break;
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || cs.display === 'none') continue;
      // inside a box whose content is skipped (content-visibility: hidden, a phone's page off the stage) the element's
      // own style isn't computed, so it can read `visible`: the browser's own answer says what's on screen
      if (typeof el.checkVisibility === 'function' && !el.checkVisibility({ contentVisibilityAuto: false, visibilityProperty: true })) continue;
      // an answer block's comment button at rest: v1 made it on hover, v2 keeps it (opacity 0) so the keyboard reaches it
      if (el.matches('[data-kit="cmt-add"]') && cs.opacity === '0') continue;
      // to the half pixel, as the references are kept; `exact` (REF_EXACT=1): as layout has it, to find a sub-pixel gap
      const round = (n) => (exact ? n : Math.round(n * 2) / 2);
      out.push({ part, i: i++, box: [round(r.x), round(r.y), round(r.width), round(r.height)], s: pick(cs) });
    }
  }
  return out;
}

// Focuses the target, or its nearest focusable descendant or ancestor, the way the keyboard reaches it:
// focusable ancestor or descendant.
async function focusTarget(p, loc) {
  const found = await loc.evaluate((el) => {
    const ok = (e) => e && (e.tabIndex >= 0 || /^(BUTTON|A|INPUT|TEXTAREA|SELECT)$/.test(e.tagName));
    const t = ok(el) ? el : el.querySelector('button, a[href], [tabindex]:not([tabindex="-1"]), input, textarea') || el.closest('[tabindex], button, a[href]');
    if (!t) return false;
    t.setAttribute('data-ref-focus', '');
    t.focus();
    return document.activeElement === t;
  });
  if (!found) return false;
  // Reach it by keyboard (Shift+Tab, Tab) so :focus-visible applies, as it does for a person tabbing.
  await p.keyboard.press('Shift+Tab');
  await p.keyboard.press('Tab');
  return p.evaluate(() => {
    const t = document.querySelector('[data-ref-focus]');
    t?.removeAttribute('data-ref-focus');
    if (document.activeElement !== t) t?.focus();
    return document.activeElement === t;
  });
}

export async function openView(browser, url, open, [width, height], colorScheme, { at, desktop } = {}) {
  const ctx = await browser.newContext({ viewport: { width, height }, colorScheme, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  // The Tauri shell's init.js defines window.wispDesktop before the page's scripts run; index.html then
  // adds body.desktop and .mac (navigator.platform, a Mac here).
  if (desktop) await ctx.addInitScript(() => (window.wispDesktop = Object.freeze({ pickFolder: async () => null })));
  const p = await ctx.newPage();
  if (at) await p.clock.setFixedTime(new Date(at));
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  await p.goto(url);
  await p.locator('#feed .item[data-id]').first().waitFor({ state: 'attached', timeout: 20_000 });
  await p.waitForTimeout(300);
  await open(p);
  await p.mouse.move(0, height - 1); // nothing hovered by accident
  await p.waitForTimeout(1200); // v1 focuses the prompt a beat after load: let the page settle
  return { ctx, p, errors };
}
