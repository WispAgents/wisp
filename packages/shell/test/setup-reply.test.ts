// Audit G4: words typed under a setup card were sent as a follow-up, which queued behind the card (and an "Allow" ran
// later as the agent's prompt). They answer the card, naming it: Allow runs it, anything else is the Deny with them.
import { expect, test } from 'vitest';
import { answerSetup, setupCardOf } from '../src/frame/commands.ts';

const row = (ask: unknown) => ({ id: 't1', body: { ext: { 'wisp.runtime': { ask } } } }) as never;

test('typed words under a setup card answer that card, by its id_audit_G4', () => {
  expect(setupCardOf(row({ id: 'q9', question: 'Ada wants to add Bob', options: ['Allow', 'Deny'], setup: { headline: 'Ada wants to add Bob' } }))).toBe('q9');
  expect(setupCardOf(row({ id: 'q1', question: 'Which file?', options: ['a', 'b'] })), 'an ask_user question answers as before').toBeUndefined();
  expect(setupCardOf(row(null))).toBeUndefined();
  expect(answerSetup.wire?.({ id: 'local:t1', text: 'keep Eve, she runs the nightly', askId: 'q9' })).toEqual({ id: 't1', text: 'keep Eve, she runs the nightly', askId: 'q9' });
});
