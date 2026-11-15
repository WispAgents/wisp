// ADR-0065 build step 8: the app's frame. Under a phone look the shell's Stage is the kit's Frame with the
// destinations (a NavigationBar) under the panes; the browser's Stage stays v1's, `nav` or not.
import { beforeAll, describe, expect, it } from 'vitest';
import type { JSX } from 'solid-js';
import { mount } from './helpers.tsx';
import { loadPhoneStage, NavigationBar, PlatformLookProvider, Stage, type Look, type NavigationItem } from '../src/index.ts';

const ITEMS: readonly NavigationItem[] = [
  { id: 'tasks', label: 'Tasks', icon: 'feed' },
  { id: 'computers', label: 'Computers', icon: 'laptop' },
];
const stage = (page: 'list' | 'main', side?: JSX.Element): JSX.Element => (
  <Stage
    layout="stack"
    page={page}
    head={<span data-x="head">head</span>}
    list={<span data-x="list">list</span>}
    main={<span data-x="main">main</span>}
    side={side}
    gripLabel="Resize"
    nav={<NavigationBar aria-label="Destinations" items={ITEMS} selected="computers" onSelect={() => {}} />}
  />
);
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);

describe('the stage on a phone', () => {
  // the phone frame is its own chunk; the app loads it with its look, before the first render
  beforeAll(() => loadPhoneStage());

  for (const look of ['android', 'ios'] as const) {
    it(`a_phone_stage_is_the_frame_with_its_destinations (${look})`, () => {
      const host = as(look, () => stage('list'));
      const frame = host.querySelector('[data-kit="frame"]');
      expect(frame).not.toBeNull();
      expect(host.querySelector('[data-kit="stage"]')).toBeNull();
      expect(frame?.getAttribute('data-page')).toBe('list');
      expect(host.querySelector('[data-kit="frame-top"] [data-x="head"]')).not.toBeNull();
      expect(host.querySelector('[data-kit="frame-nav"] [data-kit="nav-bar"]')).not.toBeNull();
      expect(host.querySelector('[data-kit="nav-item"][data-item="computers"]')?.getAttribute('aria-current')).toBe('page');
      host.remove();
    });
  }

  it('a_phone_stage_shows_the_side_pane_as_its_own_page', () => {
    const host = as('android', () => stage('main', <span data-x="side">side</span>));
    expect(host.querySelector('[data-kit="frame"]')?.getAttribute('data-page')).toBe('side');
    expect(host.querySelector('[data-pane="side"] [data-x="side"]')).not.toBeNull();
    host.remove();
  });

  it('a_browser_stage_is_v1s_and_never_draws_the_destinations', () => {
    const host = as('web', () => stage('list'));
    expect(host.querySelector('[data-kit="stage"]')).not.toBeNull();
    expect(host.querySelector('[data-kit="frame"]')).toBeNull();
    expect(host.querySelector('[data-kit="nav-bar"]')).toBeNull();
    host.remove();
  });
});
