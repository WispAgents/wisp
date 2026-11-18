// Audit A F32: a deep field scrolled into view (the keyboard, focus) slid the whole Settings sheet up: its title row
// went and a blank band showed. A box with `overflow: hidden` is still scrollable by script and focus; only the pane's
// body scrolls, so every box around it clips (`overflow: clip` never scrolls).
import { expect, test } from 'vitest';
import settings from '../src/css/settings.css?raw';
import overlays from '../src/css/overlays.css?raw';

const rule = (css: string, selector: string): string => {
  const i = css.indexOf(`${selector} {`);
  expect(i, selector).toBeGreaterThanOrEqual(0);
  return css.slice(i, css.indexOf('}', i));
};

test('the_boxes_around_settings_body_clip_and_never_scroll_audit_a_f32', () => {
  expect(rule(settings, "[data-kit='settings-frame']")).toMatch(/overflow: clip/);
  expect(rule(settings, "[data-kit='settings-main']")).toMatch(/overflow: clip/);
  expect(rule(settings, "[data-kit='settings-nav']")).toMatch(/overflow: clip/);
  expect(rule(overlays, "[data-kit='dialog'][data-bare], [data-kit='sheet'][data-bare]")).toMatch(/overflow: clip/);
  // the body still scrolls
  expect(rule(settings, "[data-kit='settings-body']")).toMatch(/overflow-y: auto/);
});
