/// <reference types="vite/client" />
// Layout fixes from the owner's desktop pass (r330), held by the kit's sheets: an icon never touches its text, a
// step's output stays inside the transcript, the working orb sits on its status line, and a plugin's mono mark
// follows the app's theme (not the OS's) in WebKit.
import { expect, test } from 'vitest';
import reset from '../src/css/reset.css?raw';
import answer from '../src/css/answer-content.css?raw';
import transcript from '../src/css/transcript.css?raw';

const rule = (css: string, selector: string): string => {
  const at = css.indexOf(`${selector} {`);
  expect(at, `no rule for ${selector}`).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf('}', at));
};

test('diff_block_without_captured_edits_keeps_a_gap_between_the_file_icon_and_its_name', () => {
  // #4: the head had no rule at all, so the icon touched "README.md"
  expect(rule(answer, "[data-kit='cb-diff-missing-head']")).toMatch(/display: flex;[^]*gap: var\(--space-5\)/);
});

test('step_output_wraps_and_scrolls_inside_the_transcript', () => {
  // #5: a bare <pre> ran past the transcript and over the side panel
  const pre = rule(transcript, "[data-kit='timeline-detail'] > pre");
  expect(pre).toContain('white-space: pre-wrap');
  expect(pre).toContain('overflow-wrap: anywhere');
  expect(pre).toContain('overflow: auto');
  expect(pre).toContain('max-height:');
});

test('working_orb_is_one_status_line_tall_and_centred_in_it', () => {
  // #6: the orb hung 4px lower than the title line
  const orb = rule(transcript, "[data-kit='working-orb']");
  const status = rule(transcript, "[data-kit='working-status']");
  const h = /height: (var\(--[a-z-]+\))/.exec(orb)?.[1];
  expect(h).toBeDefined();
  expect(orb).toContain('align-items: center');
  expect(status).toContain(`line-height: ${h}`);
});

test('mono_marks_follow_the_app_theme_and_colour_marks_are_left_alone', () => {
  // #7: WebKit draws an <img> SVG by the OS's scheme, so a dark app on a light OS drew Cursor/opencode/Grok dark on dark.
  // The marks are files now (plugins/*/ui/src/logo.svg, out of the first page's JS); a mono one says so with `#mono`
  const sel = "img[src$='#mono']";
  expect(reset).toContain(`:where([data-theme='dark']) ${sel},`);
  expect(reset).toContain(`:where([data-theme='light']) ${sel},`);
  expect(reset).toMatch(/:where\(\[data-theme='dark'\]\) img\[src\$='#mono'\],\n[^{]*\{ filter: brightness\(0\) invert\(0\.93\); \}/);
  expect(reset).toMatch(/:where\(\[data-theme='light'\]\) img\[src\$='#mono'\],\n[^{]*\{ filter: brightness\(0\) invert\(0\.09\); \}/);
  // no theme picked: the OS's (the light ink, the dark one under a dark OS)
  expect(reset).toMatch(/\n  img\[src\$='#mono'\],\n[^{]*\{ filter: brightness\(0\) invert\(0\.09\); \}/);
  expect(reset).toMatch(/@media \(prefers-color-scheme: dark\) \{\n    :where\(:root:not\(\[data-theme='light'\]\)\) img\[src\$='#mono'\],\n[^{]*\{ filter: brightness\(0\) invert\(0\.93\); \}/);
  // a mono mark as the plugins give it (plugins/{cursor,opencode,grok,copilot}/ui/src/logo.ts), a colour one, and a
  // data: URL with its own scheme rule (a user's plugin as the contract allows)
  const img = (src: string): HTMLImageElement => {
    const el = document.createElement('img');
    el.setAttribute('src', src);
    return el;
  };
  const both = `${sel}, img[src^='data:image/svg+xml'][src*='prefers-color-scheme']`;
  expect(img('/assets/logo-Bl3UK_BM.svg#mono').matches(both)).toBe(true);
  expect(img('/assets/logo-DvGkpdRw.svg').matches(both)).toBe(false);
  expect(img("data:image/svg+xml,%3Csvg fill=%22%23171717%22%3E%3Cstyle%3E%40media (prefers-color-scheme: dark)%7B%2A%7Bfill:%23ededed%7D%7D%3C/style%3E%3C/svg%3E").matches(both)).toBe(true);
  expect(img('data:image/svg+xml,%3Csvg%3E%3Cpath fill=%22%23D97757%22/%3E%3C/svg%3E').matches(both)).toBe(false);
});

test('commit_rows_keep_the_hash_files_and_counts_apart_on_a_timeline_v1_b_commits', () => {
  // QA r1 #32: "b9beff41 file+2 −1": the hash ran into "1 file" and the counts; the list showed "1." numbers
  expect(rule(answer, "[data-kit='cb-commits']")).toContain('list-style: none');
  expect(rule(answer, "[data-kit='cb-commit-sub']")).toMatch(/display: flex;[^]*gap: var\(--space-5\)/);
  expect(rule(answer, "[data-kit='cb-commit']")).toContain('grid-template-columns');
  expect(rule(answer, "[data-kit='cb-commit-node']")).toContain('border-radius: var(--radius-pill)');
});
