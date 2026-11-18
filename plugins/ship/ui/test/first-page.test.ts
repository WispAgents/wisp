// What wisp.ship's registration reads while the list draws (the pill's `if`, the dialog's title) lives in two tiny
// modules, so the first page doesn't carry the rest of the model (budget, ADR-0011: initial JS ≤ 120 KB br). The
// model modules re-export the same functions, so every other caller is unchanged.
import { expect, test } from 'vitest';
import { prOf as pillPr, prsOf as rowPrs, shippedPrs as rowShipped } from '../src/model/prs.ts';
import { dialogTitle as titleOnly } from '../src/model/title.ts';
import { prOf, prsOf, shippedPrs } from '../src/model/pr.ts';
import { dialogTitle } from '../src/model/ship.ts';
import { t } from '../src/i18n.ts';
import type { ShipSession } from '../src/types.ts';

const pr = (number: number, state: 'OPEN' | 'MERGED' = 'OPEN') => ({ number, state, url: `https://x/${number}`, title: 'x' });

test('the pill and the title read the tiny modules, the same functions the model exports', () => {
  expect([pillPr, rowPrs, rowShipped, titleOnly]).toEqual([prOf, prsOf, shippedPrs, dialogTitle]);
  const s = { id: 's', project: 'wisp', pr: pr(412), also: [{ project: 'docs', branch: 'b', pr: pr(7, 'MERGED') }], prs: [{ repo: 'other', pr: pr(9) }] } as unknown as ShipSession;
  expect(pillPr(s)?.number).toBe(412);
  expect(rowPrs(s).map((x) => x.pr.number)).toEqual([412, 7, 9]);
  expect(pillPr(undefined)).toBeUndefined();
  expect(titleOnly(t, { update: true, repos: 1, pr: 412, branches: ['b'] })).toBe(t('wisp.ship.dialog.pushOne', { n: 412 }));
  expect(titleOnly(t, { update: false, repos: 2, branches: ['a', 'b'] })).toBe(t('wisp.ship.dialog.shipMany', { count: 2 }));
});
