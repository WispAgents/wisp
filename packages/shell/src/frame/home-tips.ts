// Home's tips under the prompt box: three at a time, picked at random on each visit, each linking to the docs
// section that explains it (trywisp.dev/docs, ADR-0039; the old /guides/*.html pages only redirect). The text is in
// the catalogue (`shell.tips.<id>`, `<id>b` the bold words, `<id>a` after them); this file holds which tips there
// are and where each one links. DOM-free.

const DOCS = 'https://trywisp.dev/docs';
const G = (page: string, at: string): string => `${DOCS}/guides/${page}/#${at}`;

export interface HomeTip {
  readonly id: string;
  /** the docs section that explains it: a page under docs/site and one of its headings' anchors */
  readonly href: string;
  /** about Auto: offered only while its switch is on (owner #37) */
  readonly auto?: true;
  /** points at New task's +, which a first run keeps behind More options (v1 tips-data.js `full`) */
  readonly full?: true;
}

export const HOME_TIPS: readonly HomeTip[] = [
  { id: 'goal', href: G('run/goals', 'set-a-goal') },
  { id: 'goalReply', href: G('run/goals', 'set-a-goal') },
  { id: 'judges', href: G('run/goals', 'pick-the-judges') },
  { id: 'compare', href: G('run/compare-and-try', 'compare-harnesses'), full: true },
  { id: 'tryTwo', href: G('run/compare-and-try', 'try-it-two-ways'), full: true },
  { id: 'compareRuns', href: G('run/compare-and-try', 'read-the-comparison') },
  { id: 'auto', href: `${DOCS}/concepts/agents/#auto`, auto: true },
  { id: 'limit', href: `${DOCS}/concepts/agents/#auto`, auto: true },
  { id: 'limitWait', href: G('run/budgets-and-usage', 'at-the-limit') },
  { id: 'autoStop', href: G('run/budgets-and-usage', 'keep-automations-off-your-limit') },
  { id: 'worktree', href: G('run/worktrees-and-shipping', 'worktree-or-in-place') },
  { id: 'ship', href: G('run/worktrees-and-shipping', 'ship-it') },
  { id: 'later', href: G('run/schedules-and-triggers', 'later-on-one-task'), full: true },
  { id: 'recipes', href: G('run/schedules-and-triggers', 'recipes') },
  { id: 'review', href: G('run/review-by-another-agent', 'review-every-pr-by-another-harness') },
  { id: 'crew', href: G('run/crew-and-check-ins', 'add-a-member') },
  { id: 'machines', href: G('run/machines', 'add-a-computer-over-ssh') },
  { id: 'mcp', href: G('run/mcp-from-your-harness', 'add-wisp-to-your-harness') },
];

/** How many rows Home's tips show at once (a setup card counts as one). */
export const TIPS_SHOWN = 3;

/** Whether to offer `tip`: Auto's only while Auto is on, and those about + only once New task shows its + (not a first run). */
export const offered = (tip: HomeTip, d: { readonly autoOn: boolean; readonly firstRun: boolean }): boolean =>
  (d.autoOn || !tip.auto) && (!d.firstRun || !tip.full);

/** This visit's tips in their random order, less those not offered. Shuffled first, then filtered, so the order stays
 *  put when a first run's footer opens up or Auto is switched. */
export const tipOrder = (seed: number, d: { readonly autoOn: boolean; readonly firstRun: boolean }): HomeTip[] =>
  shuffled(HOME_TIPS, seed).filter((x) => offered(x, d));

/** `items` in an order fixed by `seed` (mulberry32 into Fisher–Yates): random per Home visit, fixed in a test. */
export function shuffled<T>(items: readonly T[], seed: number): T[] {
  let s = seed >>> 0;
  const next = (): number => {
    s = (s + 0x6d2b79f5) >>> 0;
    let r = Math.imul(s ^ (s >>> 15), 1 | s);
    r = (r + Math.imul(r ^ (r >>> 7), 61 | r)) ^ r;
    return ((r ^ (r >>> 14)) >>> 0) / 2 ** 32;
  };
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(next() * (i + 1));
    const a = out[i];
    const b = out[j];
    if (a === undefined || b === undefined) continue;
    out[i] = b;
    out[j] = a;
  }
  return out;
}

export type TipPick = { readonly kind: 'setup'; readonly i: number } | { readonly kind: 'tip'; readonly tip: HomeTip };

/**
 * Page `page` of Home's tips: at most one of the `setup` setup cards (GitHub, the Grid; they take turns), then
 * plain tips from `order` up to `count` rows. Each page (More tips) moves on through `order`, so no tip comes back
 * before every other one has shown; a page never holds the same tip twice.
 */
export function tipPage(page: number, setup: number, order: readonly HomeTip[], count = TIPS_SHOWN): TipPick[] {
  const out: TipPick[] = [];
  if (setup > 0 && count > 0) out.push({ kind: 'setup', i: ((page % setup) + setup) % setup });
  const want = Math.min(count - out.length, order.length);
  if (want <= 0) return out;
  const n = order.length;
  const start = (((page * want) % n) + n) % n;
  for (let k = 0; k < want; k++) {
    const tip = order[(start + k) % n];
    if (tip) out.push({ kind: 'tip', tip });
  }
  return out;
}
