// Home's tips: three at a time, in an order drawn at random per visit, a setup card taking one row; More tips moves
// on without repeating until every tip has shown. Each tip links to a docs page and heading that exist (the old
// links went to trywisp.dev/guides#<id>, a page with no such anchors).
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { HOME_TIPS, TIPS_SHOWN, shuffled, tipOrder, tipPage, type HomeTip } from '../src/frame/home-tips.ts';
import { SHELL_STRINGS } from '../src/i18n/en.ts';

const tipIds = (picks: ReturnType<typeof tipPage>): string[] => picks.flatMap((p) => (p.kind === 'tip' ? [p.tip.id] : []));

test('home_shows_three_tips_a_setup_card_taking_one_row', () => {
  const order = HOME_TIPS;
  expect(tipPage(0, 0, order)).toHaveLength(TIPS_SHOWN);
  const withSetup = tipPage(0, 2, order);
  expect(withSetup[0]).toEqual({ kind: 'setup', i: 0 });
  expect(tipIds(withSetup)).toHaveLength(TIPS_SHOWN - 1);
  expect(tipPage(1, 2, order)[0], 'the setup cards take turns').toEqual({ kind: 'setup', i: 1 });
  expect(tipPage(0, 1, []), 'no tips: the card alone').toEqual([{ kind: 'setup', i: 0 }]);
  expect(tipPage(0, 0, [])).toEqual([]);
  expect(tipIds(tipPage(0, 0, order.slice(0, 2))), 'fewer tips than rows, none twice').toEqual([order[0]?.id, order[1]?.id]);
});

test('more_tips_shows_every_tip_before_one_comes_back', () => {
  const order = shuffled(HOME_TIPS, 7);
  const pages = Math.ceil(order.length / TIPS_SHOWN);
  const seen = Array.from({ length: pages }, (_, p) => tipIds(tipPage(p, 0, order))).flat();
  expect(new Set(seen).size).toBe(order.length);
  for (let p = 0; p < 40; p++) {
    const ids = tipIds(tipPage(p, 1, order));
    expect(new Set(ids).size, `page ${p} holds a tip twice`).toBe(ids.length);
  }
});

test('the_order_is_random_per_seed_and_keeps_every_tip', () => {
  const a = shuffled(HOME_TIPS, 1).map((x) => x.id);
  const b = shuffled(HOME_TIPS, 2).map((x) => x.id);
  expect(a).not.toEqual(b);
  expect(shuffled(HOME_TIPS, 1).map((x) => x.id), 'the same seed, the same order').toEqual(a);
  expect([...a].sort()).toEqual(HOME_TIPS.map((x) => x.id).sort());
  const firsts = new Set(Array.from({ length: 50 }, (_, s) => tipIds(tipPage(0, 0, shuffled(HOME_TIPS, s))).join()));
  expect(firsts.size, 'visits see different first tips').toBeGreaterThan(10);
});

test('auto_tips_only_while_auto_is_on_and_plus_tips_only_past_a_first_run', () => {
  expect(tipOrder(3, { autoOn: true, firstRun: false })).toHaveLength(HOME_TIPS.length);
  expect(tipOrder(3, { autoOn: false, firstRun: false }).some((x) => x.auto)).toBe(false);
  const first = tipOrder(3, { autoOn: true, firstRun: true });
  expect(first.some((x) => x.full), "a first run's footer has no +").toBe(false);
  expect(first.length).toBeLessThan(HOME_TIPS.length);
  const all = tipOrder(3, { autoOn: true, firstRun: false }).map((x) => x.id);
  expect(first.map((x) => x.id), 'the footer opening up keeps the order').toEqual(all.filter((id) => first.some((x) => x.id === id)));
});

// Starlight's heading ids (github-slugger): lower case, punctuation dropped, each space a dash
const slug = (h: string): string =>
  h
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .replace(/\s/g, '-');
const SITE = `${join(import.meta.dirname, '..', '..', '..', 'docs', 'site')}/`;

test('every_tip_has_its_words_and_links_to_a_docs_heading_that_exists', () => {
  const strings = SHELL_STRINGS as Record<string, string>;
  for (const tip of HOME_TIPS satisfies readonly HomeTip[]) {
    for (const suf of ['', 'b', 'a']) expect(strings[`shell.tips.${tip.id}${suf}`], `${tip.id}${suf}`).toBeTruthy();
    const m = /^https:\/\/trywisp\.dev\/docs\/(.+)\/#([a-z0-9-]+)$/.exec(tip.href);
    expect(m, `${tip.id}: ${tip.href}`).not.toBeNull();
    const [, page = '', anchor = ''] = m ?? [];
    const file = [`${SITE}${page}.mdx`, `${SITE}${page}.md`].find((f) => existsSync(f));
    expect(file, `${tip.id}: no docs/site/${page}.md(x)`).toBeDefined();
    const headings = readFileSync(file ?? '', 'utf8')
      .split('\n')
      .flatMap((l) => (/^#{2,4} /.test(l) ? [slug(l.replace(/^#+ /, '').trim())] : []));
    expect(headings, `${tip.id}: #${anchor} in ${page}`).toContain(anchor);
  }
});
