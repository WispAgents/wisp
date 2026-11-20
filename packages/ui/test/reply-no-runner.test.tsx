// ADR-0114 rule 14: a factory run's task runs no agent of its own, so its reply box has no harness or model chip (it
// said "Default"): given no `runner`, neither the desktop bar nor the phone's options sheet shows one, and the hint's
// link (the shell's Stop the run) is there.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { ReplyBox } from '../src/app/Task.tsx';

test('a_reply_box_with_no_runner_shows_no_harness_or_model_chip', () => {
  const host = mount(() => (
    <ReplyBox
      placeholder="Factory · default: its stations do the work."
      value=""
      onInput={() => {}}
      onSend={() => {}}
      attach="Attach"
      mode={{ label: 'Auto', title: 'Mode' }}
      team="Team"
      goal="Goal"
      later="Later"
      send="Send"
      hint={{ long: 'Its stations do the work', short: 'Its stations do the work', link: { label: 'Stop the run', act: { onClick: () => {} } } }}
    />
  ));
  expect(host.querySelector('[data-part~="composer.harness"]')).toBeNull();
  expect(host.querySelector('[data-kit="c-model"]')).toBeNull();
  expect(host.textContent).toContain('Stop the run');
});

test('with_a_runner_the_chip_says_its_model', () => {
  const host = mount(() => (
    <ReplyBox placeholder="Follow up" value="" onInput={() => {}} onSend={() => {}} attach="Attach" mode={{ label: 'Auto', title: 'Mode' }} runner={{ title: 'Claude Code', model: 'Default' }} team="Team" goal="Goal" later="Later" send="Send" />
  ));
  expect(host.querySelector('[data-part~="composer.harness"] [data-kit="c-model"]')?.textContent).toBe('Default');
});
