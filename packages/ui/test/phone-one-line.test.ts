/// <reference types="vite/client" />
// Bars stay one line on a phone (ADR-0012, design/layout.md): New task's Recipes row and a turn's foot (QA r1 #18, #19).
import { expect, test } from 'vitest';
import home from '../src/css/app-home.css?raw';
import stackCss from '../src/css/app-stack.css?raw';
import taskCss from '../src/css/app-task.css?raw';
import iosCss from '../src/css/platform/ios.css?raw';
import androidCss from '../src/css/platform/android.css?raw';
import compositesCss from '../src/css/composites.css?raw';

const noComments = (css: string): string => css.replace(/\/\*[^]*?\*\//g, '');

test('the_recipes_row_stays_one_line_on_a_phone_its_hint_cut_short_qa_r1_18', () => {
  const css = noComments(home);
  // the hint never wraps and nothing stacks the row in a column
  expect(css).not.toMatch(/\[data-kit='lr-empty'\][^{]*\{[^}]*text-wrap-mode: wrap/);
  expect(css).not.toMatch(/\[data-kit='lr-list'\][^{]*\{[^}]*flex-direction: column/);
  const empty = /\[data-kit='lr-empty'\] \{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(empty).toContain('white-space: nowrap');
  expect(empty).toContain('text-overflow: ellipsis');
});

test('comparing_drops_worktrees_label_so_the_runs_keep_their_names_v1_l_cmp_qa_r1_20', () => {
  // v1 launcher.css: .l-bar.l-cmp #wtBtn span { display: none } (the compare chip read "Claude ..." at 760 px)
  const css = noComments(home);
  expect(css).toMatch(/\[data-kit='l-bar'\]\[data-cmp\] > \[data-a='worktree'\] > \[data-text='control'\][^{]*\{[^}]*display: none/);
});

test('a_turns_trail_never_runs_past_its_card_on_a_phone_the_receipt_cut_short_qa_r2_8', () => {
  const stack = noComments(stackCss);
  const task = noComments(taskCss);
  // the last column may shrink below its words (minmax(auto, …) kept the receipt's whole width)
  expect(stack).toMatch(/\[data-kit='trail-bar'\] \{ display: grid; grid-template-columns: auto auto auto minmax\(0, 1fr\)/);
  const receipt = /\[data-kit='tb'\]\[data-part='turn.receipt'\] \{([^}]*)\}/.exec(task)?.[1] ?? '';
  expect(receipt).toContain('min-width: 0');
  expect(receipt).toContain('text-overflow: ellipsis');
});

test('the_phone_apps_small_controls_take_a_full_tap_target_qa_r2_6', () => {
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const) {
    const flat = noComments(css);
    for (const sel of ["[data-kit='trail-bar'] [data-kit='tb-btn']", "[data-kit='trail-bar'] [data-kit='rate-btn']", "[data-kit='story-head'] [data-kit='icon-button']", "[data-part~='stuff.phone.row.menu.trigger']"]) {
      const at = flat.indexOf(`:root[data-platform='${plat}'] ${sel}`);
      expect(at, `${plat} ${sel}`).toBeGreaterThan(-1);
      expect(flat.slice(at, flat.indexOf('}', at))).toContain('min-height: var(--tap-min)');
    }
  }
});

test('on_the_phone_apps_a_toast_never_draws_over_a_menu_or_sheet_qa_r2_5', () => {
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const) {
    expect(noComments(css)).toContain(`:root[data-platform='${plat}'] [data-kit='toasts'] { z-index: calc(var(--z-menu) - 1); }`);
  }
});

test('on_a_phone_the_harness_pill_folds_to_its_logo_so_new_tasks_bar_fits_qa_r2_2', () => {
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const) {
    expect(noComments(css)).toMatch(new RegExp(`@media \\(not \\(min-width: 600px\\)\\) \\{\\s*:root\\[data-platform='${plat}'\\] \\[data-kit='launcher'\\] \\[data-kit='runner-btn'\\] > \\[data-kit='rb-h'\\] \\{ display: none; \\}`));
  }
});

test('check_now_and_comment_on_this_take_a_full_tap_target_on_the_phone_apps_qa_r3_a6', () => {
  // QA phone r3 A6: the PR card's Check now was 28 px and a block's Comment on this 29 px
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const) {
    const flat = noComments(css);
    const rule = (sel: string): string => {
      const at = flat.indexOf(`:root[data-platform='${plat}'] ${sel}`);
      expect(at, `${plat} ${sel}`).toBeGreaterThan(-1);
      return flat.slice(at, flat.indexOf('}', at));
    };
    expect(rule("[data-part~='side.pr.check']")).toContain('min-height: var(--tap-min)');
    expect(rule("[data-part~='pr.strip.check']")).toContain('min-height: var(--tap-min)');
    const cmt = rule("[data-kit='cmt-add']");
    expect(cmt).toContain('width: var(--tap-min)');
    expect(cmt).toContain('height: var(--tap-min)');
  }
});

test('a_toolbars_hidden_measuring_copy_never_widens_the_page_it_sits_in_qa_r3_a8', () => {
  // QA phone r3 A8 / #4: on an iOS task page the transcript scrolled sideways: the details bar's measuring copy (every
  // item in both forms, max-content wide from the bar's left) ran 8 px past the screen. Its items keep their own
  // widths (flex: none), so the copy itself is no wider than nothing and clips them
  const css = noComments(compositesCss);
  const copy = /\[data-kit='toolbar-measure'\] \{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(copy).toContain('width: 0');
  expect(copy).toContain('overflow: hidden');
  expect(copy).not.toContain('max-content');
  expect(css).toMatch(/\[data-kit='toolbar-measure'\] > span \{[^}]*flex: none/);
});

test('the_receipt_is_held_to_its_column_so_it_gives_way_with_an_ellipsis_qa_r3_4', () => {
  // QA phone r3 #4 (A8, still): "… 2% of weekl" cut at the screen's edge and the transcript scrolling sideways: the
  // trail's grid puts each item at its start (justify-items: start), so the receipt kept its own width past its column
  const receipt = /\[data-kit='tb'\]\[data-part='turn.receipt'\] \{([^}]*)\}/.exec(noComments(taskCss))?.[1] ?? '';
  expect(receipt).toContain('justify-self: stretch');
  expect(receipt).toContain('max-width: 100%');
});

test('the_goal_line_says_done_when_on_the_phone_apps_and_its_placeholder_gives_way_with_an_ellipsis_qa_r3_3', () => {
  // QA phone r3 #3: "all tests pass and it works on a ph", cut by the judges chip with no "…", and no "Done when" to
  // say what the field is for (v1's ≤720 px rule gives a browser's narrow box the label's room; a native form names it)
  const input = /\[data-kit='l-goal-input'\] \{([^}]*)\}/.exec(noComments(home))?.[1] ?? '';
  expect(input).toContain('text-overflow: ellipsis');
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const)
    expect(noComments(css)).toContain(`:root[data-platform='${plat}'] [data-kit='lg-k'] > [data-text='control'] { display: inline; }`);
});

test('a_tight_phone_bar_folds_the_pills_plus_and_model_before_anything_reaches_send_qa_r4_1', () => {
  // QA phone r4 #1: "Claude Code ▾ +" pushed More under Send. The bar measures itself (bar-labels.ts); tight, the pill
  // keeps its logo alone (Try another agent is in More)
  for (const [plat, css] of [['ios', iosCss], ['android', androidCss]] as const) {
    const flat = noComments(css);
    expect(flat).toContain(`:root[data-platform='${plat}'] [data-kit='l-bar'][data-tight] [data-kit='vs-btn']:not([data-del]),`);
    expect(flat).toMatch(new RegExp(`:root\\[data-platform='${plat}'\\] \\[data-kit='l-bar'\\]\\[data-tight\\] \\[data-kit='runner-btn'\\] > \\[data-kit='auto-pv'\\] \\{ display: none; \\}`));
  }
  // and the rule that shows the harness's name never outranks the phone's that hides it (round 3's :not() did)
  // (0,4,0) under the phone's (0,5,0): it loses whatever order the sheets load in
  expect(noComments(home)).toContain("[data-kit='runners']:where(:not([data-cmp]):not([data-trial])) > [data-kit='runner-btn'][data-def] > [data-kit='rb-h']");
  expect(noComments(home)).not.toMatch(/\[data-kit='runners'\]:not\(\[data-cmp\]\)[^{]*> \[data-kit='rb-h'\] \{\s*display: block/);
});
