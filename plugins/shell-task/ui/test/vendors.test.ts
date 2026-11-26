import { expect, test } from 'vitest';
import { handoffTargets, limitTarget, limitTargets, vendorOf } from '../src/cards/vendors.ts';

test('a_refused_turn_goes_only_to_a_harness_on_another_vendors_model_with_that_model_pinned', () => {
  expect([vendorOf('claude-sonnet-4.5', 'copilot'), vendorOf('openrouter/openai/gpt-5', 'opencode'), vendorOf('o4-mini', 'x'), vendorOf('opus', 'x'), vendorOf('gemini-3-pro', 'x'), vendorOf('', 'grok'), vendorOf('default', 'claude'), vendorOf('', 'copilot')]).toEqual(['anthropic', 'openai', 'openai', 'anthropic', 'google', 'xai', 'anthropic', '']);
  const agents = [
    { id: 'claude', available: true },
    { id: 'copilot', available: true, models: [{ id: 'claude-sonnet-4.5' }, { id: 'gpt-5' }] },
    { id: 'opencode', available: true, models: [{ id: 'anthropic/claude-opus' }] },
    { id: 'codex', available: false },
    { id: 'grok', available: true },
    { id: 'wisp', available: true },
  ];
  // a Claude refusal: Copilot pinned to gpt-5 (its default could be Claude), never opencode (only Claude), never
  // codex (not set up), grok on its own vendor
  expect(handoffTargets({ from: 'claude', agents })).toEqual([
    { agent: 'copilot', model: 'gpt-5', vendor: 'openai' },
    { agent: 'grok', model: '', vendor: 'xai' },
  ]);
  // a configured model counts first; max caps the list
  expect(handoffTargets({ from: 'claude', agents, models: { copilot: 'gemini-3-pro' }, max: 1 })).toEqual([{ agent: 'copilot', model: 'gemini-3-pro', vendor: 'google' }]);
  // the refusing vendor unknown: every other harness that's set up (the runtime's rule, backend round 6)
  expect(handoffTargets({ from: 'copilot', agents: [{ id: 'opencode', available: true }, { id: 'codex', available: true }] }).map((x) => x.agent)).toEqual(['opencode', 'codex']);
  // the runtime's vendor wins over reading the model
  expect(handoffTargets({ from: 'copilot', fromModel: 'gpt-5', vendor: 'anthropic', agents: [{ id: 'codex', available: true }, { id: 'claude', available: true }] }).map((x) => x.agent)).toEqual(['codex']);
  // the limit card: the first other harness that's set up
  expect(limitTarget('claude', agents)?.agent).toBe('copilot');
  expect(limitTarget('claude', [{ id: 'claude', available: true }])).toBeUndefined();
});

test('the_limit_card_hands_to_the_harness_with_the_most_plan_left_and_never_to_one_used_up', () => {
  const now = 1_000;
  const agents = [
    { id: 'claude', available: true },
    { id: 'copilot', available: true },
    { id: 'codex', available: true },
    { id: 'grok', available: true },
    { id: 'cursor', available: false },
  ];
  const plans = {
    // its 5-hour window is spent: a hand-off would only hit another limit
    copilot: [{ account: 'default', windows: [{ used: 1, resetsAt: 5_000 }, { used: 0.2, resetsAt: 9_000 }] }],
    // the tightest live window counts (0.7 used → 30% left); a window past its reset doesn't
    codex: [{ account: 'default', windows: [{ used: 0.4, resetsAt: 5_000 }, { used: 0.7, resetsAt: 9_000 }, { used: 0.99, resetsAt: 500 }] }],
    // two accounts: the roomier one
    cursor: [{ account: 'a', windows: [{ used: 0, resetsAt: 9_000 }] }],
  };
  const ranked = limitTargets('claude', agents, plans, now);
  // codex (30% left) first, grok (nothing known) after, copilot (spent) and cursor (not set up) never
  expect(ranked.map((x) => [x.agent, x.left])).toEqual([
    ['codex', expect.closeTo(0.3, 6)],
    ['grok', undefined],
  ]);
  expect(limitTarget('claude', agents, plans, now)?.agent).toBe('codex');
  // two known: more room first; equal room keeps the order they're set up in
  const two = { codex: [{ account: 'default', windows: [{ used: 0.9, resetsAt: 9_000 }] }], grok: [{ account: 'x', windows: [{ used: 0.1, resetsAt: 9_000 }] }, { account: 'y', windows: [{ used: 0.95, resetsAt: 9_000 }] }] };
  expect(limitTargets('claude', agents, two, now).map((x) => x.agent)).toEqual(['grok', 'codex', 'copilot']);
  // nothing read yet: v1's first other harness
  expect(limitTarget('claude', agents)?.agent).toBe('copilot');
});
