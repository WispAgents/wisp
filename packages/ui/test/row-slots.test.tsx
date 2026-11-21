// docs/ux/2026-11-18-desktop.md #2: a feed row's quick actions take the time's slot (Later · Done · ⋯), each a named
// button out of the Tab order (the row is the one stop), placed by its slot so a missing one moves nothing; and the CSS
// swaps them with the time by opacity alone, the title's room fixed.
import { expect, test } from 'vitest';
import css from '../src/css/composites.css?raw';
import { ListRow, RowButton, RowRun } from '../src/index.ts';
import { mount } from './helpers.tsx';

test('slotted_row_buttons_are_named_and_placed_by_slot_ux_2', () => {
  let more: HTMLElement | undefined;
  const host = mount(() => (
    <ListRow
      layout="feed"
      title="Fix the login"
      slots
      quick={1}
      onOpen={() => {}}
      actions={
        <>
          <RowButton slot={2} icon="done" tone="good" label="Mark done" onClick={() => {}} />
          <RowButton slot={3} icon="dots" label="More" onClick={(el) => (more = el)} />
        </>
      }
    />
  ));
  const row = host.querySelector('[data-kit="list-row"]');
  expect(row?.hasAttribute('data-slots')).toBe(true);
  const btns = [...host.querySelectorAll<HTMLButtonElement>('button[data-slot]')];
  expect(btns.map((b) => [b.dataset['slot'], b.getAttribute('aria-label'), b.title, b.tabIndex])).toEqual([
    ['2', 'Mark done', 'Mark done', -1],
    ['3', 'More', 'More', -1],
  ]);
  btns[1]?.click();
  expect(more).toBe(btns[1]);
});

test('the_slot_swap_is_opacity_only_and_the_title_room_never_changes_on_hover_ux_2', () => {
  // the title's room is one rule for rest and hover alike
  expect(css).toMatch(/\[data-slots\] > \[data-kit='list-row-title'\],\s*[^{]*\[data-slots\]\[data-quick\]:hover > \[data-kit='list-row-title'\],/);
  const swap = css.match(/\[data-slots\]\[data-selected\] > \[data-kit='list-row-pills'\] > \[data-part~='list.row.time'\] \{([^}]*)\}/)?.[1] ?? '';
  expect(swap.trim()).toBe('opacity: 0;');
});

test('a_run_line_on_this_computer_says_no_place_ux_2', () => {
  const host = mount(() => (
    <>
      <RowRun name="Claude Code" model="opus" where={{ icon: 'laptop', text: 'This computer' }} noWhere />
      <RowRun name="Codex" where={{ icon: 'server', text: 'gpu-box' }} />
    </>
  ));
  const runs = [...host.querySelectorAll('[data-kit="row-run"]')];
  expect(runs[0]?.querySelector('[data-kit="ir-w"]')).toBeNull();
  expect(runs[1]?.querySelector('[data-kit="ir-w"]')?.textContent).toBe('gpu-box');
});
