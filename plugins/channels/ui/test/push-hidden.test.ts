// QA setup audit F7: Settings → Phone showed a card named "push" with no description and an empty "Set it up", and
// switching it on was refused ("Settings → Phone sets up ntfy, telegram, …"). The push channel is the phone app's own
// (wisp.push, ADR-0065 §3): a phone pairs itself from the app and that turns it on, so there is nothing to set up here.
// The pane lists the channels it can set up, in v1's order; push stays out until it has a setup of its own here.
import { expect, test } from 'vitest';
import { settable } from '../src/lib.ts';

test('the_phone_pane_lists_only_the_channels_it_can_set_up', () => {
  expect(settable(['ntfy', 'push', 'telegram', 'slack', 'discord', 'github', 'email'])).toEqual(['ntfy', 'telegram', 'slack', 'discord', 'github', 'email']);
  // the server's order, and a channel another plugin adds, stay
  expect(settable(['email', 'ntfy'])).toEqual(['email', 'ntfy']);
  expect(settable(['ntfy', 'matrix', 'push'])).toEqual(['ntfy', 'matrix']);
  expect(settable(['push'])).toEqual([]);
});
