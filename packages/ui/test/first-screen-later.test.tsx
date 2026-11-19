// ADR-0105: what the first screen doesn't draw loads after it. `drawnLater` draws a note once the first screen is up,
// `bindLater` binds a phone gesture once the binders land (the browser never fetches them), and `LazySwipe` draws the
// row alone until the phone app's Swipe is there.
import { describe, expect, test } from 'vitest';
import { mount, part, until } from './helpers.tsx';
import { drawnLater, firstScreenDrawn } from '../src/controls/lazy-part.ts';
import { binders, bindLater } from '../src/gestures/binders-part.ts';
import { LazySwipe, swipePart } from '../src/shell-list/swipe-part.ts';
import { Text } from '../src/index.ts';

describe('after the first screen', () => {
  test('drawnLater_draws_nothing_until_the_first_screen_is_up_then_its_component', () => {
    const Note = drawnLater((p: { readonly words: string }) => <Text part="t.note">{p.words}</Text>);
    mount(() => <Note words="spare" />);
    expect(part('t.note')).toBeNull();
    firstScreenDrawn();
    expect(part('t.note')?.textContent).toBe('spare');
    // drawn after the first screen: at once
    mount(() => <Note words="again" />);
    expect(document.querySelectorAll('[data-part~="t.note"]')).toHaveLength(2);
  });

  test('bindLater_binds_once_the_binders_land_and_a_stop_before_then_binds_nothing', async () => {
    expect(binders.ready()).toBeUndefined();
    const bound: string[] = [];
    const stopEarly = bindLater(() => (bound.push('early'), () => bound.push('early-stop')));
    stopEarly();
    const stop = bindLater(() => (bound.push('kept'), () => bound.push('kept-stop')));
    await until(() => binders.ready() !== undefined && bound.includes('kept'));
    expect(bound).toEqual(['kept']);
    // once they're here it binds at once
    const now = bindLater(() => (bound.push('now'), () => undefined));
    expect(bound).toEqual(['kept', 'now']);
    stop();
    now();
    expect(bound).toEqual(['kept', 'now', 'kept-stop']);
  });

  test('LazySwipe_draws_the_row_alone_until_Swipe_lands_then_the_row_in_its_Swipe', async () => {
    mount(() => (
      <LazySwipe onSwipe={() => undefined} part="t.swipe">
        <Text part="t.row">row</Text>
      </LazySwipe>
    ));
    expect(part('t.row')).not.toBeNull();
    // the phone app's look asks for it (platform/styles.ts); LazySwipe never does
    expect(swipePart.ready()).toBeUndefined();
    void swipePart.load();
    await until(() => swipePart.ready() !== undefined && part('t.swipe') !== null);
    expect(part('t.swipe')?.contains(part('t.row'))).toBe(true);
  });
});
