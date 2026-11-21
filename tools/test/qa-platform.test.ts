// qa/platform.mjs's metric rules (ADR-0065 §5, design/mobile.md §A.4, §A.6, §A.8) on fake measurements.

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  barRule,
  hoverReveals,
  hoverRule,
  lookStatus,
  parseRole,
  pickSizes,
  px,
  rolesOf,
  safeRule,
  SIZES,
  splitSelectors,
  tapRule,
  tokenRule,
  tokensOf,
  twinRule,
  typeRule,
  withoutHover,
  type Bar,
  type Insets,
} from '../../qa/platform-rules.ts';

const ANDROID = { '--tap-min': '48px', '--bar-h': '64px', '--bar-h-large': '112px', '--nav-h': '80px', '--nav-rail-w': '80px', '--fab': '56px', '--fab-ext-h': '56px', '--row-h-1': '56px' };
const IOS = { '--tap-min': '44px', '--bar-h': '44px', '--bar-h-large': '96px', '--nav-h': '49px', '--nav-rail-w': '320px', '--row-h-1': '44px' };
const VP = { width: 393, height: 852 };
const NOTCH: Insets = { top: 59, right: 0, bottom: 34, left: 0 };
const bar = (o: Partial<Bar> & Pick<Bar, 'kind'>): Bar => ({ part: 'x', top: 0, bottom: 44, left: 0, width: 393, height: 44, ...o });

test('the_sizes_are_adr_0065s', () => {
  assert.deepEqual(
    SIZES.map((s) => `${s.look} ${s.width}x${s.height}`),
    ['android 360x800', 'android 412x915', 'android 840x1200', 'ios 375x667', 'ios 393x852', 'ios 440x956', 'ios 840x1180'],
  );
  assert.deepEqual(pickSizes(['ios'], ['393x852', 'ipad']).map((s) => s.width), [393, 840]);
  assert.deepEqual(pickSizes(['android'], ['840']).map((s) => s.look), ['android']);
});

test('a_view_with_no_platform_look_is_a_skip_not_a_pass', () => {
  const unset = lookStatus('android', null, 0, 0);
  assert.equal(unset.check, false);
  assert.match('skip' in unset ? unset.skip : '', /data-platform not set/);
  const none = lookStatus('ios', 'ios', 0, 0);
  assert.match('skip' in none ? none.skip : '', /^no platform look yet/);
  const elsewhere = lookStatus('ios', 'ios', 12, 0);
  assert.match('skip' in elsewhere ? elsewhere.skip : '', /^no platform look yet: none of the kit's 12/);
  assert.deepEqual(lookStatus('ios', 'ios', 12, 3), { check: true });
});

test('the_other_look_on_the_page_fails', () => {
  const s = lookStatus('ios', 'android', 5, 5);
  assert.equal('fail' in s && s.fail.rule, 'look');
});

test('a_missing_token_set_fails_once', () => {
  assert.deepEqual(tokenRule('android', tokensOf(ANDROID)), []);
  assert.deepEqual(tokenRule('ios', tokensOf(IOS)), []); // iOS has no FAB
  const f = tokenRule('android', tokensOf({}));
  assert.equal(f.length, 1);
  assert.match(f[0]?.detail ?? '', /--tap-min.*--row-h-1/);
});

test('px_reads_only_plain_lengths', () => {
  assert.equal(px('48px'), 48);
  assert.equal(px(' 0.5px '), 0.5);
  assert.equal(px('env(safe-area-inset-top, 0px)'), undefined);
  assert.equal(px(''), undefined);
});

test('a_control_under_the_touch_target_fails', () => {
  const f = tapRule(
    [
      { part: 'ok', hitW: 48, hitH: 48 },
      { part: 'small', hitW: 32, hitH: 48 },
      { part: 'grown', hitW: 47.7, hitH: 48 }, // a 24 px icon with a 48 px hit area, sub-pixel layout
      { part: 'link', hitW: 30, hitH: 18, inline: true },
    ],
    48,
  );
  assert.deepEqual(f.map((x) => x.part), ['small']);
  assert.match(f[0]?.detail ?? '', /32 × 48, needs 48 × 48/);
  assert.deepEqual(tapRule([{ part: 'ios', hitW: 44, hitH: 44 }], 44), []);
  assert.deepEqual(tapRule([{ part: 'x', hitW: 1, hitH: 1 }], undefined), [], 'no token: tokenRule reports it');
});

test('bars_match_their_tokens_with_the_insets_added', () => {
  const t = tokensOf(IOS);
  // a 44 pt bar under a 59 pt notch is 103 high from the top edge
  assert.deepEqual(barRule([bar({ kind: 'app-bar', height: 103, bottom: 103 })], t, NOTCH, VP), []);
  assert.equal(barRule([bar({ kind: 'app-bar', height: 44, bottom: 44 })], t, NOTCH, VP).length, 1, 'ignores the notch');
  assert.deepEqual(barRule([bar({ kind: 'app-bar', top: 200, bottom: 244, height: 44 })], t, NOTCH, VP), [], 'not at the edge: no inset');
  assert.deepEqual(barRule([bar({ kind: 'app-bar-large', height: 155, bottom: 155 })], t, NOTCH, VP), []);
  // the tab bar: 49 + the 34 pt home indicator
  assert.deepEqual(barRule([bar({ kind: 'navigation-bar', top: 769, bottom: 852, height: 83 })], t, NOTCH, VP), []);
  const short = barRule([bar({ kind: 'navigation-bar', top: 803, bottom: 852, height: 49 })], t, NOTCH, VP);
  assert.match(short[0]?.detail ?? '', /15 px over the 34 px bottom inset, --nav-h is 49/);
});

test('fab_rail_and_rows_follow_android_tokens', () => {
  const t = tokensOf(ANDROID);
  const vp = { width: 412, height: 915 };
  const ins: Insets = { top: 24, right: 0, bottom: 24, left: 0 };
  assert.deepEqual(barRule([bar({ kind: 'fab', top: 700, bottom: 756, width: 56, height: 56 })], t, ins, vp), []);
  assert.equal(barRule([bar({ kind: 'fab', top: 700, bottom: 740, width: 40, height: 40 })], t, ins, vp).length, 1);
  assert.deepEqual(barRule([bar({ kind: 'fab-extended', top: 700, bottom: 756, width: 140, height: 56 })], t, ins, vp), []);
  assert.equal(barRule([bar({ kind: 'navigation-rail', top: 0, bottom: 915, width: 72, height: 915 })], t, ins, vp)[0]?.rule, 'bar-width');
  // rows grow with text at 200 %, never shrink below the one-line row
  assert.deepEqual(barRule([bar({ kind: 'list-row', top: 100, bottom: 220, height: 120 })], t, ins, vp), []);
  assert.equal(barRule([bar({ kind: 'list-row', top: 100, bottom: 140, height: 40 })], t, ins, vp).length, 1);
});

test('text_under_the_notch_or_home_indicator_fails', () => {
  const at = (part: string, top: number, bottom: number, inScroller = false) => ({ part, top, bottom, left: 16, right: 100, inScroller });
  const f = safeRule([at('clear', 70, 90), at('notch', 10, 40), at('pinned', 830, 850), at('row', 830, 880, true), at('below', 900, 950, true)], NOTCH, VP);
  assert.deepEqual(f.map((x) => `${x.part} ${x.detail}`), ['notch under the top inset (59 px)', 'pinned under the bottom inset (34 px)']);
  // a list's first row under the status bar fails even inside its scroller
  assert.equal(safeRule([at('first', 0, 40, true)], NOTCH, VP).length, 1);
  // one finding per part and edge
  assert.equal(safeRule([at('n', 0, 10), at('n', 5, 15)], NOTCH, VP).length, 1);
  assert.deepEqual(safeRule([at('x', 0, 10)], { top: 0, right: 0, bottom: 0, left: 0 }, VP), []);
});

test('type_roles_are_read_from_the_tokens', () => {
  assert.deepEqual(parseRole('--type-title-m', '500 16px/24px var(--font-ui)'), { name: 'title-m', weight: 500, size: 16, lineHeight: 24 });
  assert.equal(parseRole('--type-x', 'bold 1rem'), undefined);
  assert.deepEqual(rolesOf({ '--type-body': '400 17px/22px var(--font-ui)', '--bar-h': '44px' }).map((r) => r.name), ['body']);
});

test('text_off_the_type_roles_fails_and_scales_with_the_text_size', () => {
  const roles = rolesOf({ '--type-body': '400 17px/22px x', '--type-footnote': '400 13px/18px x' });
  const texts = [
    { part: 'row', size: 17, sample: 'Fix the login bug' },
    { part: 'row', size: 14, sample: 'v1 size' },
    { part: 'row', size: 14, sample: 'again' },
    { part: 'meta', size: 13, sample: '2m' },
  ];
  const f = typeRule(texts, roles, 1);
  assert.equal(f.length, 1, 'one finding per part and size');
  assert.match(f[0]?.detail ?? '', /14 px "v1 size" \(\+1 more\) is no type role at 100 % \(13\/17 px\)/);
  // at 200 % the same 17 px text didn't follow the system size
  const big = typeRule([{ part: 'row', size: 34, sample: 'scaled' }, { part: 'row', size: 17, sample: 'stuck' }], roles, 2);
  assert.deepEqual(big.map((x) => x.detail.split(' ')[0]), ['17']);
  assert.equal(typeRule(texts, [], 1)[0]?.part, ':root', 'no roles: says so instead of failing every text');
});

test('a_hover_rule_that_reveals_a_control_is_found', () => {
  assert.deepEqual(splitSelectors('.a:hover .b, :is(.c, .d):hover'), ['.a:hover .b', ':is(.c, .d):hover']);
  assert.deepEqual(hoverReveals({ selector: '.row:hover .act, .row:focus-within .act', props: { opacity: '1' } }), ['.row .act']);
  assert.deepEqual(hoverReveals({ selector: '.row:hover', props: { background: 'red' } }), [], 'a colour change hides nothing');
  assert.deepEqual(hoverReveals({ selector: '.row:hover .act', props: { opacity: '0' } }), []);
  assert.deepEqual(hoverReveals({ selector: '.row:hover .act', props: { visibility: 'visible' }, media: { text: '(hover: hover)', matches: false } }), [], 'never applies on a phone');
  assert.deepEqual(hoverReveals({ selector: '.m:hover > .sub', props: { display: 'block' }, media: { text: '(min-width: 1px)', matches: true } }), ['.m > .sub']);
  assert.deepEqual(hoverReveals({ selector: '.x:not(:hover) .y', props: { opacity: '1' } }), []);
  // inside a pseudo-class list :hover becomes :is(*), so the selector stays one the page can run
  assert.deepEqual(hoverReveals({ selector: '.row:is(:hover, :focus-within) .act', props: { opacity: '1' } }), ['.row:is(:is(*), :focus-within) .act']);
  assert.equal(withoutHover('.a:hover:focus .b:hover'), '.a:focus .b');
  // an opacity it can't read counts as revealing (fail closed)
  assert.deepEqual(hoverReveals({ selector: '.row:hover .act', props: { opacity: 'var(--shown)' } }), ['.row .act']);
});

test('only_hidden_controls_count_as_hover_only', () => {
  const f = hoverRule([
    { part: 'row.actions', selector: '.row .act', hidden: true, interactive: true },
    { part: 'row.actions', selector: '.row .act', hidden: true, interactive: true },
    { part: 'row.shade', selector: '.row .shade', hidden: true, interactive: false },
    { part: 'row.more', selector: '.row .more', hidden: false, interactive: true },
  ]);
  assert.deepEqual(f.map((x) => x.part), ['row.actions']);
});

test('a_gesture_without_its_visible_twin_fails', () => {
  const parts = new Set(['task.more', 'list.select']);
  const f = twinRule(
    [
      { part: 'list.row', gesture: 'swipe-done', twins: ['task.more', 'list.select'] },
      { part: 'list.row', gesture: 'swipe-done', twins: ['task.more', 'list.select'] },
      { part: 'task.head', gesture: 'swipe-next', twins: ['task.next'] },
      { part: 'queue.item', gesture: 'drag', twins: [] },
    ],
    parts,
  );
  assert.deepEqual(f.map((x) => x.detail), ['swipe-next: twin task.next not on screen', 'drag names no visible twin (data-twin)']);
});
