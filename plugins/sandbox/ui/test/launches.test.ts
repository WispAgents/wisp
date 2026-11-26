import { describe, expect, it } from 'vitest';
import { setupFailed, shown, stepsOf, type LaunchRow } from '../src/launch-model.ts';

const row = (over: Partial<LaunchRow>): LaunchRow => ({ id: 'l', title: 'T', state: 'preparing', step: 'make', at: 1, agent: 'claude', ...over });

describe('a /sandbox launch', () => {
  it('shows in the list until its task started, newest first', () => {
    const list = [row({ id: 'a', at: 1 }), row({ id: 'b', at: 3, state: 'failed' }), row({ id: 'c', at: 2, state: 'started' }), row({ id: 'd', at: 2, state: 'signin' })];
    expect(shown(list).map((r) => r.id)).toEqual(['b', 'd', 'a']);
  });
  it('says each step done, being done, or still to come, and which one failed', () => {
    expect(stepsOf(row({ step: 'copy' })).map((s) => s.status)).toEqual(['done', 'done', 'current', 'pending']);
    expect(stepsOf(row({ step: 'wake', state: 'failed' })).map((s) => s.status)).toEqual(['done', 'failed', 'pending', 'pending']);
    expect(stepsOf(undefined).map((s) => s.status)).toEqual(['current', 'pending', 'pending', 'pending']);
    expect(stepsOf(row({ step: 'start', state: 'started' })).map((s) => s.status)).toEqual(['done', 'done', 'done', 'done']);
  });
  // SBX-7: a failed .wisp/sandbox.sh was never shown; the page shows what it said before it opens the task
  it('says what a failed setup said once the task started, and nothing for one that worked', () => {
    expect(setupFailed(row({ state: 'started', setup: { ok: false, tail: 'npm ERR!' } }))).toBe('npm ERR!');
    expect(setupFailed(row({ state: 'started', setup: { ok: true, tail: 'done' } }))).toBeUndefined();
    expect(setupFailed(row({ state: 'started', setup: null }))).toBeUndefined();
    expect(setupFailed(row({ state: 'preparing', setup: { ok: false, tail: 'x' } }))).toBeUndefined();
  });
});
