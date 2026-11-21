// QA r9 #3: the reply bar's options chip never cuts its words ("Full au…" at iOS 320): when they don't fit it folds to
// its icons (the mode's, the harness's logo, ▾), its name still saying it all; with room again it says it again.
import { afterEach, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { PlatformLookProvider } from '../src/index.ts';
import { ReplyOptions } from '../src/app/ReplyOptions.tsx';

let room = 200;
const was = {
  scroll: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollWidth'),
  client: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth'),
};
afterEach(() => {
  if (was.scroll) Object.defineProperty(HTMLElement.prototype, 'scrollWidth', was.scroll);
  if (was.client) Object.defineProperty(HTMLElement.prototype, 'clientWidth', was.client);
  document.body.innerHTML = '';
});

test('on_a_phone_the_options_chip_folds_to_its_icons_when_its_words_would_be_cut', async () => {
  // the mode's words need 64 px; the chip gives them `room`
  Object.defineProperty(HTMLElement.prototype, 'scrollWidth', { configurable: true, get(this: HTMLElement) { return this.getAttribute('data-kit') === 'c-opts-mode' ? 64 : 0; } });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get(this: HTMLElement) { return this.getAttribute('data-kit') === 'c-opts-mode' ? Math.min(64, room) : 0; } });
  room = 40;
  const box = mount(() => (
    <PlatformLookProvider look={() => 'ios'}>
      <div data-kit="c-bar">
        <ReplyOptions labels={{ options: 'Reply options', mode: 'Permissions', agent: 'Agent' }} mode={{ label: 'Full auto', icon: 'spark' }} runner={{ title: 'Claude Code' }} rows={[]} />
      </div>
    </PlatformLookProvider>
  ));
  const chip = box.querySelector<HTMLElement>('[data-kit="c-opts"]') as HTMLElement;
  await until(() => chip.hasAttribute('data-folded'), 2000);
  expect(chip.hasAttribute('data-folded'), 'folded: the words would be cut').toBe(true);
  expect(chip.getAttribute('aria-label'), 'its name says it all').toContain('Full auto');
  // room again (a wider screen, a shorter mode): it says it again
  room = 200;
  window.dispatchEvent(new Event('resize'));
  await until(() => !chip.hasAttribute('data-folded'), 2000);
  expect(chip.hasAttribute('data-folded'), 'room again: its words back').toBe(false);
});
