/// <reference types="vite/client" />
// audit C tasks #4: in a phone list's Select, the row tapped last looked unlike the other picked rows. A phone keeps
// :hover on what it last tapped, and the rows' :hover (and Android's pressed tint, on :hover too) beat the picked tint.
// Hover shades a row only for a pointer that hovers, and never a picked or selected row.
import { expect, test } from 'vitest';
import composites from '../src/css/composites.css?raw';
import android from '../src/css/platform/android.css?raw';
import ios from '../src/css/platform/ios.css?raw';

/** every rule whose selector shades a list row or feed item on :hover, with whether it sits in @media (hover: hover) */
function hoverRules(css: string): { sel: string; hovering: boolean }[] {
  const out: { sel: string; hovering: boolean }[] = [];
  const re = /([^{}]*?):hover\s*\{/g;
  for (let m = re.exec(css); m; m = re.exec(css)) {
    const sel = (m[1] ?? '').trim().split(',').pop()?.trim() ?? '';
    if (!/\[data-kit='(list-row|feed-item)'\]\[?[^>\s]*$/.test(sel)) continue;
    const before = css.slice(0, m.index);
    const media = before.lastIndexOf('@media (hover: hover)');
    // inside the media block when no closing brace at depth 0 came between
    let depth = 0;
    for (const ch of before.slice(media < 0 ? before.length : media)) depth += ch === '{' ? 1 : ch === '}' ? -1 : 0;
    out.push({ sel, hovering: media >= 0 && depth > 0 });
  }
  return out;
}

test('a_tapped_row_on_a_phone_looks_like_the_other_picked_rows_audit_c_4', () => {
  for (const [name, css] of [['composites', composites], ['android', android]] as const) {
    for (const r of hoverRules(css).filter((x) => !/transparent/.test(x.sel))) {
      expect(r.hovering, `${name}: ${r.sel}:hover shades a phone's last-tapped row`).toBe(true);
    }
  }
  // the platforms' own hover and pressed rows leave a picked or selected row's tint alone
  for (const css of [android, ios]) {
    for (const line of css.split('\n').filter((l) => /\[data-kit='list-row'\]\[data-interactive\][^{]*:(hover|active)/.test(l))) {
      expect(line).toContain(':not([data-selected]):not([data-picked])');
    }
  }
});
