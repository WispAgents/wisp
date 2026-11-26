// UI QA r1: a SettingsRow's below band drew its hairline and gap with nothing in it: an element passed as `below`
// (a notes list whose own Show is off) is always truthy, so the band rendered empty. The band hides when it's empty,
// and nothing else (no stray text) sits in it, so `:empty` holds.
import { expect, test } from 'vitest';
import { Show } from 'solid-js';
import CSS from '../src/css/composites.css?raw';
import { mount } from './helpers.tsx';
import { SettingsRow } from '../src/index.ts';

test('an_empty_below_band_is_hidden', () => {
  const body = CSS.match(/\[data-kit='settings-row-below'\]:empty\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(body).toMatch(/display:\s*none/);
});

test('a_below_that_renders_nothing_leaves_the_band_empty_and_real_content_shows', () => {
  const host = mount(() => (
    <>
      <SettingsRow part="t.off" title="Off" control={<span />} below={<Show when={false}>x</Show>} />
      <SettingsRow part="t.on" title="On" control={<span />} below={<Show when={true}>the note</Show>} />
    </>
  ));
  const off = host.querySelector('[data-part="t.off"] [data-kit="settings-row-below"]');
  expect(off).not.toBeNull();
  expect(off?.childNodes.length, 'no child nodes, so :empty applies').toBe(0);
  const on = host.querySelector('[data-part="t.on"] [data-kit="settings-row-below"]');
  expect(on?.textContent).toBe('the note');
});
