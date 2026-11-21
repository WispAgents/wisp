/// <reference types="vite/client" />
// audit-B #8 and #9, held by the kit's sheets (the layout itself is checked in real Chrome at 320–390):
//   #8 iOS: the Tasks bar's folded title ("All projects ▾") gives way to the bar's actions, ending in its ellipsis
//      with ▾ kept, instead of the actions running over it.
//   #9 the working panel's status line keeps the status whole; the count, then the time, drop out whole.
import { expect, test } from 'vitest';
import ios from '../src/css/platform/ios.css?raw';
import transcript from '../src/css/transcript.css?raw';
import { WorkingPanel } from '../src/transcript/WorkingPanel.tsx';
import { mount } from './helpers.tsx';

const rule = (css: string, selector: string): string => {
  const at = css.indexOf(`${selector} {`);
  expect(at, `no rule for ${selector}`).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf('}', at));
};

test('ios_folded_root_title_gives_way_to_the_actions_and_keeps_its_chevron', () => {
  const row = rule(ios, ":root[data-platform='ios'] [data-kit='app-bar'][data-large][data-collapsed] [data-kit='app-bar-row']");
  // the actions' column is never narrower than its buttons; the title's may shrink to nothing
  expect(row).toContain('grid-template-columns: minmax(0, 1fr) minmax(0, max-content) minmax(max-content, 1fr)');
  // the name shrinks inside the title button (a flex item's min-width is its text otherwise), so ▾ stays in view
  expect(rule(ios, ":root[data-platform='ios'] [data-kit='app-bar-title-button'] > [data-text='control']")).toContain('min-width: 0');
});

test('working_status_keeps_the_status_whole_and_drops_the_count_then_the_time', () => {
  const line = rule(transcript, "[data-kit='working'] [data-kit='working-status']");
  // a Wrap group (only Wrap wraps: design/layout.md §5) in a one-line-tall box: what doesn't fit goes to a line out of
  // sight, whole
  expect(line).not.toContain('flex-wrap');
  expect(line).toContain('row-gap: 0');
  expect(line).toContain('overflow: hidden');
  expect(line).toMatch(/height: (var\(--size-control-xs\))[^]*line-height: \1/);
  // the time and the count never shrink (never cut: design/layout.md §1)
  expect(rule(transcript, "[data-kit='working-elapsed'], [data-kit='working-count']")).toContain('flex: none');
  // the note needs no room to stay, and truncates in what's left
  expect(rule(transcript, "[data-kit='working-note']")).toContain('flex: 1 1 0');
});

test('working_status_is_a_wrap_group', () => {
  mount(() => <WorkingPanel status="Claude Code is waiting for you" elapsed="35m 25s" count="3 actions" waiting />);
  expect(document.querySelector("[data-kit='working-status']")?.getAttribute('data-layout')).toBe('wrap');
});
