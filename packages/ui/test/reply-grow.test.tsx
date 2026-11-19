// Audit F orchestration F3: the reply box stayed one 34 px line and a long follow-up (or Ask to fix's sentence) ran off
// sideways, its start out of view. v1 (composer.css:10, composer.js:157) keeps only the empty box's placeholder on one
// line and grows the box with its text up to 260 px (autosize), then it scrolls.
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { ReplyBox, REPLY_MAX } from '../src/app/Task.tsx';
import css from '../src/css/app-task.css?raw';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = '';
});

test('the_reply_box_grows_with_its_text_up_to_260_px_then_scrolls_audit_f_orchestration_f3', async () => {
  expect(REPLY_MAX).toBe(260);
  // happy-dom lays nothing out: a line is 20 px, ~50 characters a line in the box
  const tall = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollHeight');
  Object.defineProperty(HTMLTextAreaElement.prototype, 'scrollHeight', { configurable: true, get(this: HTMLTextAreaElement) { return 12 + 20 * Math.max(1, Math.ceil(this.value.length / 50)); } });
  try {
    const [value, setValue] = createSignal('');
    const host = document.createElement('div');
    document.body.append(host);
    dispose = render(
      () => (
        <ReplyBox placeholder="Follow up" value={value()} onInput={(v) => setValue(v)} onSend={() => {}} attach="Attach" mode={{ label: 'Auto', title: 'Mode' }} runner={{ title: 'Claude Code' }} team="Team" goal="Goal" later="Later" send="Send" />
      ),
      host,
    );
    const box = host.querySelector('[data-kit="reply"]') as HTMLTextAreaElement;
    setValue('x'.repeat(160)); // four lines
    await Promise.resolve();
    expect(box.style.height).toBe('92px');
    expect(box.style.overflowY).toBe('hidden');
    setValue('x'.repeat(2000)); // past the cap
    await Promise.resolve();
    expect(box.style.height).toBe(`${REPLY_MAX}px`);
    expect(box.style.overflowY).toBe('auto');
  } finally {
    if (tall) Object.defineProperty(HTMLTextAreaElement.prototype, 'scrollHeight', tall);
    else delete (HTMLTextAreaElement.prototype as { scrollHeight?: number }).scrollHeight;
  }
  // only the empty box's placeholder is one line: the typed text wraps
  const reply = css.slice(css.indexOf("[data-kit='reply'],"), css.indexOf('}', css.indexOf("[data-kit='reply'],")));
  expect(reply).not.toContain('nowrap');
  const empty = css.slice(css.indexOf("[data-kit='reply']:placeholder-shown"));
  expect(empty.slice(0, empty.indexOf('}'))).toContain('white-space: nowrap');
});
