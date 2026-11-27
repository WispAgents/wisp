// Every loader in the kit draws the one Liquid mark (ADR-0140, owner 2026-11-27): a button's loading, a pending act,
// live work, the current step, an upload. A new loader that draws its own ring, dot grid or
// pulse fails here.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { Spinner } from '../src/controls/Spinner.tsx';
import { Orb } from '../src/transcript/Orb.tsx';
import { Steps } from '../src/composites/Steps.tsx';
import { FileChip } from '../src/composer/FileChip.tsx';

const liquid = (host: HTMLElement): SVGElement | null => host.querySelector<SVGElement>('[data-kit="liquid"]');

test('every_kit_loader_draws_the_liquid', () => {
  const views = [
    () => <Spinner label="Saving" />,
    () => <Orb live />,
    () => <Steps items={[{ title: 'Build', status: 'current' }]} />,
    () => <FileChip name="a.png" status="uploading" removeLabel="Remove" />,
  ];
  for (const v of views) {
    const host = mount(v);
    expect(liquid(host), host.innerHTML).not.toBeNull();
    expect(liquid(host)?.querySelectorAll('[data-kit="liquid-drop"]').length).toBe(3);
  }
});

test('the_metaball_filter_is_in_the_document_once_however_many_loaders', () => {
  mount(() => <Spinner />);
  mount(() => <Orb live />);
  expect(document.querySelectorAll('#k-liquid').length).toBe(1);
});

test('waiting_is_the_amber_mark_and_idle_is_still', () => {
  expect(liquid(mount(() => <Orb live waiting />))?.hasAttribute('data-waiting')).toBe(true);
  expect(liquid(mount(() => <Orb />))?.hasAttribute('data-still')).toBe(true);
  expect(liquid(mount(() => <Orb live />))?.hasAttribute('data-still')).toBe(false);
});

test('no_kit_css_spins_or_pulses_a_loader_of_its_own', () => {
  // the old loaders: a rotating ring or icon, the 3×3 ripple, pulsing dots. Pull to refresh (platform css) is the
  // gesture's own native indicator, and the sandbox's BuildBox has its own design (owner, 2026-11-27); both stay.
  const dir = join(import.meta.dirname, '../src/css');
  const css = readdirSync(dir).filter((f) => f.endsWith('.css')).map((f) => [f, readFileSync(join(dir, f), 'utf8')] as const);
  const bad = css.filter(([f, s]) => f !== 'motion.css' && /animation:\s*kit-spin|kit-orb|btw-pulse/.test(s)).map(([f]) => f);
  expect(bad).toEqual([]);
});
