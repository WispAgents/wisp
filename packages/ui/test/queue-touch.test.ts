/// <reference types="vite/client" />
// Audit A orchestration #23 (phone 390): nine queued follow-ups filled the screen and pushed the reply box's mode,
// harness, Queue and Send off it. On a screen with no hover a long queue scrolls in its own capped area, so the reply
// box's controls stay on screen; a row's actions are its menu there (a tap, v1), not the hover row.
import { expect, test } from 'vitest';
import css from '../src/css/queued.css?raw';

const noComments = (s: string): string => s.replace(/\/\*[^]*?\*\//g, '');

test('on_a_touch_screen_a_queued_rows_actions_show_and_a_long_queue_scrolls', () => {
  const q = noComments(css);
  const touch = /@media \(hover: none\) \{([^]*?)\n  \}/.exec(q)?.[1] ?? '';
  expect(touch, 'a rule for screens with no hover').not.toBe('');
  // the hover row stays hidden: a tap opens the row's menu (Send now, Edit, Combine…)
  expect(touch).toMatch(/\[data-kit='cq-acts'\],\s*\[data-kit='cq-keys'\] \{\s*display: none;/);
  expect(touch).toMatch(/\[data-kit='cq-list'\] \{[^}]*max-height: [^;]+;[^}]*overflow-y: auto;/);
});
