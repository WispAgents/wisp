/// <reference types="vite/client" />
// The reply box's bar on a narrow phone (Android 360: its tools ran 3 px past it and the bar scrolled): ADR-0012, a bar
// folds, it never clips. Its labels go to their icons when they don't fit (the mode's Full auto), measured, and come
// back once the box holds them again; only past that does the bar scroll under its pinned Send.
import { expect, test } from 'vitest';
import taskCss from '../src/css/app-task.css?raw';
import { labelsGo } from '../src/app/bar-labels.ts';

test('the_labels_go_to_their_icons_only_while_they_dont_fit_and_come_back_with_room_android_360', () => {
  // 299 px of tools with their words in a 296 px bar: the words go
  expect(labelsGo(299, 296)).toBe(true);
  // they fit (or exactly): they stay
  expect(labelsGo(296, 296)).toBe(false);
  expect(labelsGo(280, 314)).toBe(false);
  // not measured with its words yet: keep them, the next measure knows
  expect(labelsGo(undefined, 200)).toBe(false);
});

test('a_tight_reply_bar_shows_the_modes_icon_alone', () => {
  const css = taskCss.replace(/\/\*[^]*?\*\//g, '');
  expect(css).toMatch(/\[data-kit='c-bar'\]\[data-tight\] > \[data-kit='mode-btn'\] > \[data-text='control'\] \{[^}]*display: none/);
});
