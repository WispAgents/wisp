// Audit G tasks #5: a prompt holding a long path or JSON ran out of its bubble on a phone (390 px: to x 446), cut, as the
// turn's text kept `overflow-wrap: normal`. It breaks anywhere, as the transcript's bubbles do.
import { expect, test } from 'vitest';
import css from '../src/css/app-task.css?raw';

test('the_prompts_text_breaks_a_long_word_inside_its_bubble_audit_g_tasks_5', () => {
  const rule = css.replace(/\/\*[^]*?\*\//g, '').match(/\[data-kit='ask-text'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/overflow-wrap:\s*anywhere/);
});
