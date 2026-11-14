// The metric rules of qa/platform.mjs (ADR-0065 §5, design/mobile.md §A.4, §A.6, §A.8), as pure functions
// on measurements: the page measures (qa/platform.mjs `measure`), these decide. No browser, no clock, so
// tools/test/qa-platform.test.ts checks each rule on fake measurements.
//
// The kit's side of the contract (build steps 3–7 draw these; until a view has any, it is skipped). The bar
// names are packages/ui/src/nav's (step 3):
//   [data-kit="app-bar"]          TopAppBar: the top app bar (Android) / navigation bar (iOS). Height --bar-h,
//                                 or --bar-h-large with data-large and not data-collapsed (+ --safe-top when
//                                 it touches the top edge)
//   [data-kit="nav-bar"]          NavigationBar: M3 navigation bar / iOS tab bar, --nav-h high (+ --safe-bottom
//                                 at the bottom edge); standing upright (Android from 600 dp) it's the rail,
//                                 --nav-rail-w wide (+ --safe-left at the left edge)
//   [data-kit="fab"]              Fab: --fab square, or --fab-ext-h high with data-extended
//   [data-kit="list-row"]         at least --row-h-1 high (rows grow with text, never shrink; step 4)
//   [data-gesture="<name>"]       an element a gesture acts on; data-twin="<data-part> …" names the visible
//                                 controls doing the same (§A.4), each on screen in the same view
//   text sizes                    from the look's --type-* roles, times the text scale (§A.6)

export type Look = 'android' | 'ios';

export interface Insets {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

export interface Size {
  readonly look: Look;
  readonly width: number;
  readonly height: number;
  /** what the system draws over the edges on that device: status bar, notch, home indicator, gesture bar */
  readonly insets: Insets;
  readonly name: string;
}

/** ADR-0065 §5's sizes. Insets: Android's status bar 24 dp + gesture bar 24 dp; iPhone SE 20/0, iPhone 15/16
 * 59/34, 16 Pro Max 62/34; the 840 px tablets (M3 expanded) a status bar and a gesture bar / home indicator. */
export const SIZES: readonly Size[] = [
  { look: 'android', name: 'compact', width: 360, height: 800, insets: { top: 24, right: 0, bottom: 24, left: 0 } },
  { look: 'android', name: 'pixel', width: 412, height: 915, insets: { top: 24, right: 0, bottom: 24, left: 0 } },
  { look: 'android', name: 'tablet', width: 840, height: 1200, insets: { top: 24, right: 0, bottom: 24, left: 0 } },
  { look: 'ios', name: 'se', width: 375, height: 667, insets: { top: 20, right: 0, bottom: 0, left: 0 } },
  { look: 'ios', name: 'iphone', width: 393, height: 852, insets: { top: 59, right: 0, bottom: 34, left: 0 } },
  { look: 'ios', name: 'max', width: 440, height: 956, insets: { top: 62, right: 0, bottom: 34, left: 0 } },
  { look: 'ios', name: 'ipad', width: 840, height: 1180, insets: { top: 24, right: 0, bottom: 20, left: 0 } },
];

/** Picks sizes by look and, optionally, `<width>x<height>` or bare widths (the SIZES env). */
export function pickSizes(looks: readonly string[], only?: readonly string[]): Size[] {
  return SIZES.filter((s) => looks.includes(s.look) && (!only?.length || only.some((o) => o === `${s.width}x${s.height}` || o === String(s.width) || o === s.name)));
}

export interface Finding {
  readonly rule: string;
  readonly part: string;
  readonly detail: string;
}

const EPS = 0.5;

/** "12px", "12", " 12.5px " → 12 / 12.5; anything else (empty, env(), auto) → undefined. */
export function px(v: string | undefined): number | undefined {
  if (v === undefined) return undefined;
  const m = /^\s*(-?\d+(?:\.\d+)?)(?:px)?\s*$/.exec(v);
  return m?.[1] === undefined ? undefined : Number(m[1]);
}

// ---------------------------------------------------------------------------------------------------------
// Whether a view has a platform look to check at all.

export type Status = { readonly check: true } | { readonly check: false; readonly skip: string } | { readonly check: false; readonly fail: Finding };

/** attr: <html data-platform>; variants: kit rules under :root[data-platform=<look>] that aren't tokens;
 * matched: how many of those match something in the view. */
export function lookStatus(look: Look, attr: string | null, variants: number, matched: number): Status {
  if (attr === null || attr === '') return { check: false, skip: `data-platform not set: the shell didn't take the page for the ${look} app (build step 1)` };
  if (attr !== look) return { check: false, fail: { rule: 'look', part: ':root', detail: `data-platform="${attr}", expected "${look}"` } };
  if (variants === 0) return { check: false, skip: `no platform look yet: the kit has no ${look} variants (build steps 3–7)` };
  if (matched === 0) return { check: false, skip: `no platform look yet: none of the kit's ${variants} ${look} variants draws this view` };
  return { check: true };
}

// ---------------------------------------------------------------------------------------------------------
// Tokens, read from the root's computed custom properties.

export interface Tokens {
  readonly tapMin?: number;
  readonly barH?: number;
  readonly barHLarge?: number;
  readonly navH?: number;
  readonly navRailW?: number;
  readonly fab?: number;
  readonly fabExtH?: number;
  readonly rowH1?: number;
}

export const TOKEN_NAMES: Readonly<Record<keyof Tokens, string>> = {
  tapMin: '--tap-min',
  barH: '--bar-h',
  barHLarge: '--bar-h-large',
  navH: '--nav-h',
  navRailW: '--nav-rail-w',
  fab: '--fab',
  fabExtH: '--fab-ext-h',
  rowH1: '--row-h-1',
};

export function tokensOf(vars: Readonly<Record<string, string>>): Tokens {
  const out: Record<string, number> = {};
  for (const [k, name] of Object.entries(TOKEN_NAMES)) {
    const v = px(vars[name]);
    if (v !== undefined) out[k] = v;
  }
  return out;
}

/** The platform token set must be on the page: without --tap-min nothing below means anything. */
export function tokenRule(look: Look, t: Tokens): Finding[] {
  const want: (keyof Tokens)[] = look === 'android' ? ['tapMin', 'barH', 'barHLarge', 'navH', 'navRailW', 'fab', 'fabExtH', 'rowH1'] : ['tapMin', 'barH', 'barHLarge', 'navH', 'navRailW', 'rowH1'];
  const missing = want.filter((k) => t[k] === undefined).map((k) => TOKEN_NAMES[k]);
  return missing.length ? [{ rule: 'tokens', part: ':root', detail: `no ${missing.join(', ')}: the ${look} token set isn't loaded` }] : [];
}

// ---------------------------------------------------------------------------------------------------------
// Touch targets (§A.6): 48 × 48 dp on Android, 44 × 44 pt on iOS, measured as the area that answers a
// touch (elementFromPoint), so a small control that grows its hit area passes.

export interface Target {
  readonly part: string;
  /** the hit area: how far from the control's centre a touch still lands on it, both ways summed */
  readonly hitW: number;
  readonly hitH: number;
  /** a link inside running text: the platforms exempt it */
  readonly inline?: boolean;
}

export function tapRule(targets: readonly Target[], tapMin: number | undefined): Finding[] {
  if (tapMin === undefined) return [];
  return targets
    .filter((t) => !t.inline && (t.hitW < tapMin - EPS || t.hitH < tapMin - EPS))
    .map((t) => ({ rule: 'tap-target', part: t.part, detail: `${Math.round(t.hitW)} × ${Math.round(t.hitH)}, needs ${tapMin} × ${tapMin}` }));
}

// ---------------------------------------------------------------------------------------------------------
// Bars (§A.7, §A.8).

export type BarKind = 'app-bar' | 'app-bar-large' | 'navigation-bar' | 'navigation-rail' | 'fab' | 'fab-extended' | 'list-row';

export interface Bar {
  readonly kind: BarKind;
  readonly part: string;
  readonly top: number;
  readonly bottom: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

const near = (a: number, b: number, slack = 1): boolean => Math.abs(a - b) <= slack;

export function barRule(bars: readonly Bar[], t: Tokens, insets: Insets, viewport: { readonly width: number; readonly height: number }): Finding[] {
  const out: Finding[] = [];
  const atTop = (b: Bar): number => (b.top <= 1 ? insets.top : 0);
  const atBottom = (b: Bar): number => (b.bottom >= viewport.height - 1 ? insets.bottom : 0);
  const f = (b: Bar, detail: string): void => void out.push({ rule: 'bar-height', part: b.part, detail });
  for (const b of bars) {
    switch (b.kind) {
      case 'app-bar':
      case 'app-bar-large': {
        const token = b.kind === 'app-bar' ? t.barH : t.barHLarge;
        const name = b.kind === 'app-bar' ? '--bar-h' : '--bar-h-large';
        if (token === undefined) break;
        const h = b.height - atTop(b);
        if (!near(h, token)) f(b, `${b.kind} ${Math.round(h)} px${atTop(b) ? ` under the ${insets.top} px top inset` : ''}, ${name} is ${token}`);
        break;
      }
      case 'navigation-bar': {
        if (t.navH === undefined) break;
        const h = b.height - atBottom(b);
        if (!near(h, t.navH)) f(b, `navigation bar ${Math.round(h)} px${atBottom(b) ? ` over the ${insets.bottom} px bottom inset` : ''}, --nav-h is ${t.navH}`);
        break;
      }
      case 'navigation-rail': {
        if (t.navRailW === undefined) break;
        const w = b.width - (b.left <= 1 ? insets.left : 0);
        if (!near(w, t.navRailW)) out.push({ rule: 'bar-width', part: b.part, detail: `navigation rail ${Math.round(w)} px wide, --nav-rail-w is ${t.navRailW}` });
        break;
      }
      case 'fab': {
        if (t.fab === undefined) break;
        if (!near(b.height, t.fab) || !near(b.width, t.fab)) f(b, `FAB ${Math.round(b.width)} × ${Math.round(b.height)}, --fab is ${t.fab}`);
        break;
      }
      case 'fab-extended': {
        if (t.fabExtH === undefined) break;
        if (!near(b.height, t.fabExtH)) f(b, `extended FAB ${Math.round(b.height)} px high, --fab-ext-h is ${t.fabExtH}`);
        break;
      }
      case 'list-row': {
        if (t.rowH1 !== undefined && b.height < t.rowH1 - EPS) f(b, `row ${Math.round(b.height)} px, --row-h-1 is ${t.rowH1}`);
        break;
      }
    }
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------
// Safe areas: nothing a person reads or touches sits under the status bar, the notch, the home indicator or
// the gesture bar, at scroll 0. A scrolling list's rows may pass under the bottom edge (edge-to-edge); the
// top of every pane and anything pinned may not.

export interface Placed {
  readonly part: string;
  readonly top: number;
  readonly bottom: number;
  readonly left: number;
  readonly right: number;
  /** inside a scroll container that scrolls vertically */
  readonly inScroller: boolean;
}

export function safeRule(items: readonly Placed[], insets: Insets, viewport: { readonly width: number; readonly height: number }): Finding[] {
  const out = new Map<string, Finding>();
  const add = (p: Placed, edge: string, inset: number): void => {
    const key = `${p.part}|${edge}`;
    if (!out.has(key)) out.set(key, { rule: 'safe-area', part: p.part, detail: `under the ${edge} inset (${inset} px)` });
  };
  for (const p of items) {
    if (p.bottom <= 0 || p.top >= viewport.height || p.right <= 0 || p.left >= viewport.width) continue; // off screen
    if (insets.top && p.top < insets.top - EPS) add(p, 'top', insets.top);
    if (insets.bottom && !p.inScroller && p.bottom > viewport.height - insets.bottom + EPS) add(p, 'bottom', insets.bottom);
    if (insets.left && p.left < insets.left - EPS) add(p, 'left', insets.left);
    if (insets.right && p.right > viewport.width - insets.right + EPS) add(p, 'right', insets.right);
  }
  return [...out.values()];
}

// ---------------------------------------------------------------------------------------------------------
// Type roles (§A.8): every text's size is one of the look's --type-* roles, times the text scale (§A.6).

export interface Role {
  readonly name: string;
  readonly weight: number;
  readonly size: number;
  readonly lineHeight: number;
}

/** `--type-body-l: 400 16px/24px var(--font-ui)` → {body-l, 400, 16, 24}. */
export function parseRole(name: string, value: string): Role | undefined {
  const m = /^\s*(\d{3})\s+(\d+(?:\.\d+)?)px\s*\/\s*(\d+(?:\.\d+)?)px/.exec(value);
  if (!m?.[1] || !m[2] || !m[3]) return undefined;
  return { name: name.replace(/^--type-/, ''), weight: Number(m[1]), size: Number(m[2]), lineHeight: Number(m[3]) };
}

export function rolesOf(vars: Readonly<Record<string, string>>): Role[] {
  return Object.entries(vars).flatMap(([k, v]) => (k.startsWith('--type-') ? (parseRole(k, v) ?? []) : []));
}

export interface Text {
  readonly part: string;
  readonly size: number;
  readonly sample: string;
}

/** One finding per part and size (a list of 40 rows in the wrong size is one problem). */
export function typeRule(texts: readonly Text[], roles: readonly Role[], scale: number): Finding[] {
  if (!roles.length) return [{ rule: 'type-role', part: ':root', detail: 'no --type-* roles on the page: type is not checked' }];
  const sizes = [...new Set(roles.map((r) => r.size))];
  const groups = new Map<string, { t: Text; n: number }>();
  for (const t of texts) {
    if (sizes.some((s) => Math.abs(t.size - s * scale) <= EPS)) continue;
    const key = `${t.part}|${t.size}`;
    const g = groups.get(key);
    if (g) g.n++;
    else groups.set(key, { t, n: 1 });
  }
  const roleList = sizes
    .sort((a, b) => a - b)
    .map((s) => s * scale)
    .join('/');
  return [...groups.values()].map(({ t, n }) => ({
    rule: 'type-role',
    part: t.part,
    detail: `${t.size} px "${t.sample}"${n > 1 ? ` (+${n - 1} more)` : ''} is no type role at ${Math.round(scale * 100)} % (${roleList} px)`,
  }));
}

// ---------------------------------------------------------------------------------------------------------
// Nothing hover-only (§A.6, design/mobile.md B: Touch): a control that only shows on :hover can't be found
// with a finger. The page lists its :hover rules; this picks those that reveal something, and the selector
// to find what they reveal.

export interface HoverRuleIn {
  readonly selector: string;
  /** the declarations that matter: opacity, visibility, display, pointer-events */
  readonly props: Readonly<Record<string, string>>;
  /** the enclosing @media's condition and whether it matches on this (touch) page; none: always applies */
  readonly media?: { readonly text: string; readonly matches: boolean };
}

/** Splits a selector list on its top-level commas. */
export function splitSelectors(sel: string): string[] {
  const out: string[] = [];
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
}

/** The selectors of what a :hover rule reveals (`.row:hover .act` → `.row .act`), or [] if it reveals nothing
 * or never applies on a touch screen (inside `@media (hover: hover)`, which the page reports as not matching). */
export function hoverReveals(r: HoverRuleIn): string[] {
  if (r.media && !r.media.matches) return [];
  const p = r.props;
  const op = p['opacity'];
  const reveals =
    // an opacity the rule can't be read for (var(), calc()) counts as revealing: fail closed
    (op !== undefined && (Number.isNaN(Number.parseFloat(op)) || Number.parseFloat(op) > 0)) ||
    p['visibility'] === 'visible' ||
    (p['display'] !== undefined && p['display'] !== '' && p['display'] !== 'none') ||
    (p['pointer-events'] !== undefined && p['pointer-events'] !== 'none');
  if (!reveals) return [];
  return splitSelectors(r.selector)
    .filter((s) => s.includes(':hover') && !/:not\([^)]*:hover/.test(s))
    .map((s) => withoutHover(s))
    .filter((s) => s !== '');
}

/** Drops `:hover` from a selector: at the top level it goes (`.row:hover .act` → `.row .act`); inside a list
 * (`:is(:hover, :focus-within)`) it becomes `:is(*)`, true for any element, so the list stays valid. */
export function withoutHover(sel: string): string {
  let depth = 0;
  let out = '';
  for (let i = 0; i < sel.length; i++) {
    const ch = sel[i];
    if (sel.startsWith(':hover', i) && !/[a-z-]/.test(sel[i + 6] ?? '')) {
      if (depth > 0) out += ':is(*)';
      i += 5;
      continue;
    }
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    out += ch;
  }
  return out.replace(/\s+/g, ' ').trim();
}

export interface HoverHit {
  readonly part: string;
  readonly selector: string;
  /** hidden now (opacity 0, visibility hidden, display none or pointer-events none) */
  readonly hidden: boolean;
  /** is or holds a control (button, link, input, [role=button], [tabindex]) */
  readonly interactive: boolean;
}

export function hoverRule(hits: readonly HoverHit[]): Finding[] {
  const seen = new Set<string>();
  const out: Finding[] = [];
  for (const h of hits) {
    if (!h.hidden || !h.interactive || seen.has(h.part)) continue;
    seen.add(h.part);
    out.push({ rule: 'hover-only', part: h.part, detail: `hidden until :hover (${h.selector})` });
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------
// Every gesture's visible twin (§A.4): an element with data-gesture names its twins in data-twin, and each
// must be on screen in the same view.

export interface Gesture {
  readonly part: string;
  readonly gesture: string;
  /** data-twin's data-part names; empty when the attribute is missing */
  readonly twins: readonly string[];
}

export function twinRule(gestures: readonly Gesture[], visibleParts: ReadonlySet<string>): Finding[] {
  const out: Finding[] = [];
  const seen = new Set<string>();
  for (const g of gestures) {
    const key = `${g.part}|${g.gesture}`;
    if (seen.has(key)) continue;
    seen.add(key);
    if (!g.twins.length) {
      out.push({ rule: 'gesture-twin', part: g.part, detail: `${g.gesture} names no visible twin (data-twin)` });
      continue;
    }
    const missing = g.twins.filter((t) => !visibleParts.has(t));
    if (missing.length) out.push({ rule: 'gesture-twin', part: g.part, detail: `${g.gesture}: twin ${missing.join(', ')} not on screen` });
  }
  return out;
}
