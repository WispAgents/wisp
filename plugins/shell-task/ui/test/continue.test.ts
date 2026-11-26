// Continue on… (continue/model.ts, ADR-0125): the preview says what each choice carries and its size, summary only
// never writes out the transcript, the follow-up names the harness, model and choice, and a turn's switch mark reads
// as the Timeline's divider.
import { expect, test } from 'vitest';
import { continueArgs, dividerWords, parseContinueParams, previewModel, switchOf, tokenWords, CARRY_ON, type HandoffPreview } from '../src/continue/model.ts';
import CARDS from '../src/i18n/en.cards.json' with { type: 'json' };
import LAZY from '../src/i18n/en.lazy.json' with { type: 'json' };

/** both halves: the dialog's words are lazy (they load with it), its title and menu entries eager */
const words = { ...CARDS, ...LAZY } as Record<string, string>;
/** the catalogue's words, `{x}` filled, `.one`/`.other` by `count` */
const t = (key: string, vars: Readonly<Record<string, string | number>> = {}): string => {
  const plural = typeof vars['count'] === 'number' ? `${key}.${vars['count'] === 1 ? 'one' : 'other'}` : key;
  const s = words[plural] ?? words[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? ''));
};

const P: HandoffPreview = {
  summary: 'Fixed the login redirect',
  files: ['src/login.ts', 'src/session.ts'],
  missed: 3,
  rules: 1,
  brief: { turns: 1, tokens: 420 },
  full: { turns: 3, tokens: 5_600 },
};

test('the_preview_gives_each_choice_its_token_estimate_and_summary_only_omits_the_transcript', () => {
  const brief = previewModel(P, 'summary', t);
  expect(brief.options.map((o) => [o.id, o.label, o.hint])).toEqual([
    ['summary', 'Summary only', '~420 tokens'],
    ['full', 'Include full transcript', '~5.6k tokens'],
  ]);
  expect(brief.tokens).toBe('~420 tokens');
  expect(brief.rows.map((r) => [r.id, r.text])).toEqual([
    ['summary', 'Fixed the login redirect'],
    ['files', 'src/login.ts, src/session.ts'],
    ['turns', 'The latest of 3 turns it missed'],
    ['rules', '1 rule you set goes along'],
  ]);
  const full = previewModel(P, 'full', t);
  expect(full.tokens).toBe('~5.6k tokens');
  expect(full.rows.find((r) => r.id === 'turns')?.text).toBe('3 turns it missed, written out');
  // it has seen everything: nothing goes; not read yet: no rows
  expect(previewModel({ ...P, missed: 0 }, 'summary', t).nothing).toBe(true);
  expect(previewModel(undefined, 'summary', t).rows).toEqual([]);
  expect(tokenWords(12_345, t)).toBe('~12k tokens');
});

test('continue_sends_one_follow_up_on_the_harness_and_model_with_the_choice', () => {
  const key = 'laptop:s1' as never;
  expect(continueArgs(key, { agent: 'codex', model: 'gpt-5' }, 'summary', '  ')).toEqual({ key, text: CARRY_ON, run: { agent: 'codex', model: 'gpt-5' }, handoff: 'summary' });
  expect(continueArgs(key, { agent: 'grok', model: '' }, 'full', 'fix the tests')).toEqual({ key, text: 'fix the tests', run: { agent: 'grok' }, handoff: 'full' });
  expect(parseContinueParams({ key: 'laptop:s1', agent: 'codex', text: '' })).toEqual({ key: 'laptop:s1', agent: 'codex' });
  expect(() => parseContinueParams({ key: 's1' })).toThrow();
});

test('a_turns_switch_mark_reads_as_the_timelines_divider', () => {
  const turn = { body: { ext: { 'wisp.runtime': { switch: { from: { agent: 'claude', model: 'claude-opus-5-5' }, to: { agent: 'codex', model: '' }, context: 'summary' } } } } };
  const m = switchOf(turn);
  expect(m).toEqual({ from: { agent: 'claude', model: 'claude-opus-5-5' }, to: { agent: 'codex', model: '' }, context: 'summary' });
  const names: Record<string, string> = { claude: 'Claude Code', codex: 'Codex' };
  expect(dividerWords(m!, (a) => names[a] ?? a, (_a, model) => (model === 'claude-opus-5-5' ? 'Opus 5.5' : model), t)).toBe('Switched from Claude Code (Opus 5.5) to Codex · context: summary');
  expect(switchOf({ body: { ext: {} } })).toBeUndefined();
  expect(switchOf({ ext: { 'wisp.runtime': { switch: { from: { agent: 'a' }, to: { agent: 'b' } } } } })?.context, 'a switch that said nothing carried it all').toBe('full');
});
