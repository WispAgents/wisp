// Audit F setup F9: since E3 every connector's state pill was cut, so on a phone "1 tool" read "1 …" while the command
// beside it kept most of the row. Only a failure's sentence gives way (its words in the title); a short state keeps its
// words and the command is cut.
import { expect, test } from 'vitest';
import { statusCuts } from '../src/lib.ts';
import type { ConnectorView } from '../src/types.ts';

const view = (over: Partial<ConnectorView>): ConnectorView => ({ id: 'qa_echo', name: 'qa_echo', ...over }) as ConnectorView;

test('only_a_failures_sentence_gives_way_in_the_pill_audit_f_setup_f9', () => {
  expect(statusCuts(view({ test: { ok: true, count: 1 } } as Partial<ConnectorView>), false)).toBe(false);
  expect(statusCuts(view({ testing: true }), false)).toBe(false);
  expect(statusCuts(view({}), false)).toBe(false);
  expect(statusCuts(view({}), true)).toBe(false);
  expect(statusCuts(view({ test: { ok: false, error: "It wouldn't start: definitely-not-a-command-qa: not found" } } as Partial<ConnectorView>), false)).toBe(true);
});
