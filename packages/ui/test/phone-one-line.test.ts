/// <reference types="vite/client" />
// Bars stay one line on a phone (ADR-0012, design/layout.md): New task's Recipes row and a turn's foot (QA r1 #18, #19).
import { expect, test } from 'vitest';
import home from '../src/css/app-home.css?raw';
import stackCss from '../src/css/app-stack.css?raw';
import taskCss from '../src/css/app-task.css?raw';
import iosCss from '../src/css/platform/ios.css?raw';
import androidCss from '../src/css/platform/android.css?raw';

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
