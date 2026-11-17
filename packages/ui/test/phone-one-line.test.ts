/// <reference types="vite/client" />
// Bars stay one line on a phone (ADR-0012, design/layout.md): New task's Recipes row and a turn's foot (QA r1 #18, #19).
import { expect, test } from 'vitest';
import home from '../src/css/app-home.css?raw';

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
