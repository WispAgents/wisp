// qa/phone-rules.ts: what a phone view is for qa/platform.mjs, qa/layout.mjs --phone and qa/feedback.mjs --phone.

import assert from 'node:assert/strict';
import test from 'node:test';
import { gate, phoneEnv, phoneRuns, phoneViews, runLabel, tally } from '../../qa/phone-rules.ts';
import { lookStatus } from '../../qa/platform-rules.ts';

const VIEWS = [{ name: 'home' }, { name: 'task' }, { name: 'palette' }];

test('the_defaults_are_adr_0065s_sizes_both_themes_and_text_at_100_and_200', () => {
  const e = phoneEnv({});
  assert.deepEqual(e.looks, ['android', 'ios']);
  assert.equal(e.sizes.length, 7);
  assert.deepEqual(e.themes, ['light', 'dark']);
  assert.deepEqual(e.scales, [100, 200]);
  assert.equal(e.views, undefined);
  assert.equal(e.force, false);
  assert.deepEqual(phoneEnv({}, { themes: 'light' }).themes, ['light']);
});

test('the_env_picks_a_subset', () => {
  const e = phoneEnv({ PLATFORMS: 'ios', SIZES: '393x852, ipad', THEMES: 'dark', SCALES: '200', VIEWS: 'home,task', PLATFORM_FORCE: '1' });
  assert.deepEqual(e.sizes.map((s) => `${s.look} ${s.width}`), ['ios 393', 'ios 840']);
  assert.deepEqual(e.themes, ['dark']);
  assert.deepEqual(e.scales, [200]);
  assert.deepEqual(e.views, ['home', 'task']);
  assert.equal(e.force, true);
});

test('an_env_that_picks_nothing_throws_instead_of_passing_empty', () => {
  assert.throws(() => phoneEnv({ PLATFORMS: 'windows' }), /not a phone look/);
  assert.throws(() => phoneEnv({ PLATFORMS: 'ios', SIZES: '360x800' }), /no sizes/);
  assert.throws(() => phoneEnv({ SCALES: '100,big' }), /positive percentages/);
  assert.throws(() => phoneEnv({ SCALES: '0' }), /positive percentages/);
});

test('views_keep_the_list_order_and_an_unknown_name_throws', () => {
  assert.deepEqual(phoneViews(VIEWS).map((v) => v.name), ['home', 'task', 'palette']);
  assert.deepEqual(phoneViews(VIEWS, ['palette', 'home']).map((v) => v.name), ['home', 'palette']);
  assert.throws(() => phoneViews(VIEWS, ['home', 'settings']), /no phone view settings/);
});

test('the_runs_are_sizes_by_themes_by_scales_by_views', () => {
  const { sizes } = phoneEnv({ PLATFORMS: 'android', SIZES: '360,412' });
  const runs = phoneRuns(sizes, ['light', 'dark'], [100, 200], VIEWS.slice(0, 2));
  assert.equal(runs.length, 2 * 2 * 2 * 2);
  assert.deepEqual(
    runs.slice(0, 5).map((r) => runLabel(r.size.look, r.size, r.theme, r.scale, r.view.name)),
    ['android 360x800 light 100% home', 'android 360x800 light 100% task', 'android 360x800 light 200% home', 'android 360x800 light 200% task', 'android 360x800 dark 100% home'],
  );
  assert.equal(runLabel('ios', { width: 393, height: 852 }, undefined, 200, 'task'), 'ios 393x852 200% task');
});

test('a_view_with_no_platform_look_is_skipped_never_passed', () => {
  const none = gate(lookStatus('ios', 'ios', 0, 0), false);
  assert.equal(none.run, false);
  assert.match('skip' in none ? none.skip : '', /^no platform look yet/);
  assert.deepEqual(gate(lookStatus('ios', 'ios', 4, 2), false), { run: true, forced: false });
});

test('force_checks_a_view_with_no_look_yet_but_never_a_page_that_isnt_the_app', () => {
  assert.deepEqual(gate(lookStatus('android', 'android', 9, 0), true), { run: true, forced: true });
  const unset = gate(lookStatus('android', null, 0, 0), true);
  assert.equal(unset.run, false);
  assert.match('skip' in unset ? unset.skip : '', /data-platform not set/);
  const other = gate(lookStatus('ios', 'android', 3, 3), true);
  assert.equal('fail' in other && other.fail.rule, 'look');
});

test('the_tally_counts_each_status_and_lists_each_skip_reason_once', () => {
  const t = tally([
    { status: 'pass' },
    { status: 'skip', skip: 'no platform look yet: a' },
    { status: 'fail' },
    { status: 'skip', skip: 'no platform look yet: a' },
    { status: 'skip', skip: 'data-platform not set' },
  ]);
  assert.deepEqual(t, { pass: 1, fail: 1, skip: 3, reasons: ['no platform look yet: a', 'data-platform not set'] });
});
