// Render audit doc28a: an unclosed ```wisp block took the next unrelated ``` as its close, swallowing the prose
// and the code fence between them. Only a fence right after the JSON closes it.
import { expect, test } from 'vitest';
import { parseInlineChatBlocks } from '../src/frame/chat-blocks.ts';

const JSON_BODY = '{"blocks":[{"type":"markdown","text":"hi"}]}';

test('an_unclosed_wisp_fence_keeps_the_prose_and_the_next_fence_doc28a', () => {
  const out = parseInlineChatBlocks(`\`\`\`wisp\n${JSON_BODY}\n\nProse that should show.\n\n\`\`\`ts\nconst x = 1;\n\`\`\`\nafter`);
  // the block, then everything after it as one markdown block
  expect(out).toHaveLength(2);
  expect(out?.[0]).toEqual({ type: 'markdown', text: 'hi' });
  expect(out?.[1]?.['type']).toBe('markdown');
  const rest = String(out?.[1]?.['text']);
  expect(rest).toContain('Prose that should show.');
  expect(rest).toContain('```ts\nconst x = 1;\n```');
  expect(rest.endsWith('after')).toBe(true);
});

test('a_closed_wisp_fence_still_drops_its_closing_fence_doc28a', () => {
  const out = parseInlineChatBlocks(`\`\`\`wisp\n${JSON_BODY}\n\`\`\`\ntail`);
  expect(out).toEqual([{ type: 'markdown', text: 'hi' }, { type: 'markdown', text: 'tail' }]);
});
