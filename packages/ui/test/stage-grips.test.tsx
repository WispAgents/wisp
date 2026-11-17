// Owner's desktop test 2026-11-17 #17, #18: the sidebar button did nothing and neither the task list nor the side panel
// could be resized. The stage draws the list hidden (v1 body.side-hidden) and both grips drag (v1 split.js #splitGrip,
// panel.js .pn-grip): the width follows the pointer and is reported once more as it's let go.
import { describe, expect, test } from 'vitest';
import { Stage } from '../src/index.ts';
import { mount } from './helpers.tsx';

function stage(o: { hidden?: boolean; side?: boolean; list?: (px: number, done: boolean) => void; panel?: (px: number, done: boolean) => void }): HTMLElement {
  return mount(() => (
    <Stage
      layout="split"
      page="main"
      head={<span />}
      list={<span />}
      main={<span />}
      side={o.side ? <span /> : undefined}
      gripLabel="Resize the task list"
      listWidth={400}
      listHidden={o.hidden}
      panelWidth={500}
      onListResize={o.list}
      onPanelResize={o.panel}
    />
  ));
}

const press = (el: EventTarget, type: string, x: number, pointerId = 1): void =>
  void el.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, button: 0, pointerId, isPrimary: pointerId === 1 }));

describe('the stage', () => {
  test('the_list_and_panel_widths_are_the_stage_s_custom_properties', () => {
    const host = stage({ side: true });
    const el = host.querySelector<HTMLElement>('[data-kit="stage"]');
    expect(el?.style.getPropertyValue('--side-w')).toBe('400px');
    expect(el?.style.getPropertyValue('--panel-w')).toBe('500px');
  });

  test('a_hidden_list_is_said_on_the_stage_and_has_no_grip', () => {
    const host = stage({ hidden: true });
    expect(host.querySelector('[data-kit="stage"]')?.hasAttribute('data-list-hidden')).toBe(true);
    expect(host.querySelector('[data-kit="split-grip"]')).toBeNull();
  });

  test('dragging_the_list_grip_reports_its_width_while_moving_and_once_let_go', () => {
    const got: [number, boolean][] = [];
    const host = stage({ list: (px, done) => void got.push([px, done]) });
    const grip = host.querySelector('[data-kit="split-grip"]');
    if (!grip) throw new Error('no grip');
    press(grip, 'pointerdown', 404);
    expect(host.querySelector('[data-kit="stage"]')?.hasAttribute('data-resizing')).toBe(true);
    press(grip, 'pointermove', 454);
    press(grip, 'pointerup', 480);
    expect(got).toEqual([
      [450, false],
      [476, true],
    ]);
    expect(host.querySelector('[data-kit="stage"]')?.hasAttribute('data-resizing')).toBe(false);
  });

  test('dragging_the_panel_s_edge_left_widens_it', () => {
    const got: [number, boolean][] = [];
    const host = stage({ side: true, panel: (px, done) => void got.push([px, done]) });
    const grip = host.querySelector('[data-kit="pn-grip"]');
    if (!grip) throw new Error('no panel grip');
    press(grip, 'pointerdown', 900);
    press(grip, 'pointermove', 850);
    press(grip, 'pointerup', 800);
    expect(got).toEqual([
      [550, false],
      [600, true],
    ]);
  });

  test('a_cancelled_drag_keeps_the_last_width_and_a_press_that_never_moved_reports_nothing', () => {
    const got: [number, boolean][] = [];
    const host = stage({ list: (px, done) => void got.push([px, done]) });
    const grip = host.querySelector('[data-kit="split-grip"]');
    if (!grip) throw new Error('no grip');
    press(grip, 'pointerdown', 404);
    press(grip, 'pointermove', 434);
    press(grip, 'pointercancel', 0);
    expect(got).toEqual([
      [430, false],
      [430, true],
    ]);
    got.length = 0;
    press(grip, 'pointerdown', 404);
    press(grip, 'pointerup', 404);
    expect(got).toEqual([]);
  });

  test('a_grip_that_goes_away_mid_drag_still_ends_it_and_a_second_pointer_is_ignored', () => {
    const got: [number, boolean][] = [];
    const host = stage({ list: (px, done) => void got.push([px, done]) });
    const grip = host.querySelector('[data-kit="split-grip"]');
    if (!grip) throw new Error('no grip');
    press(grip, 'pointerdown', 404);
    press(grip, 'pointerdown', 300, 2);
    press(grip, 'pointermove', 424);
    grip.remove();
    press(window, 'pointerup', 444);
    expect(got).toEqual([
      [420, false],
      [440, true],
    ]);
    expect(host.querySelector('[data-kit="stage"]')?.hasAttribute('data-resizing')).toBe(false);
  });
});
