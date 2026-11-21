// TRY-15: a refused command's toast says why in words, never a command id, and offers Try again when trying again
// can help (a timeout, a peer that was away), never for a no (forbidden, invalid, vetoed).
import { expect, test } from 'vitest';
import { CommandError } from '@wisp/client';
import { commandErrorToast, errorWords } from '../src/toasts.ts';
import { SHELL_STRINGS } from '../src/i18n/en.ts';

function toasts() {
  const shown: { text: string; action?: { label: string; onClick: () => void } }[] = [];
  const toast = commandErrorToast({
    error: (text, o) => {
      shown.push({ text, ...(o?.action && { action: o.action }) });
      return 't';
    },
  });
  return { shown, toast };
}

test('a refusal with no message reads as its code in words, never the command id', () => {
  const words = SHELL_STRINGS as Record<string, string>;
  errorWords((code) => words[`shell.err.${code}`], words['shell.pane.retry'] ?? '');
  const { shown, toast } = toasts();
  toast(new CommandError({ code: 'plugin_off', message: '' }), 'runtime.setState');
  toast(new CommandError({ code: 'brand_new_code', message: '  ' }), 'runtime.setState');
  toast(new CommandError({ code: 'vetoed', message: 'titles are locked' }), 'session.rename');
  expect(shown.map((s) => s.text)).toEqual(['The plugin that does that is off.', "That didn't work.", 'titles are locked']);
  expect(shown.every((s) => !s.text.includes('runtime.'))).toBe(true);
});

test('Try again shows for a refusal worth retrying and runs the same command again', () => {
  const { shown, toast } = toasts();
  let ran = 0;
  toast(new CommandError({ code: 'timeout', message: 'the peer was busy' }), 'x.y', () => ran++);
  toast(new CommandError({ code: 'invalid', message: 'no such branch' }), 'x.y', () => ran++);
  toast(new CommandError({ code: 'internal', message: 'disk full', retry: true }), 'x.y', () => ran++);
  toast(new CommandError({ code: 'unavailable', message: 'laptop is away' }), 'x.y');
  expect(shown.map((s) => s.action?.label)).toEqual(['Try again', undefined, 'Try again', undefined]);
  shown[0]?.action?.onClick();
  expect(ran).toBe(1);
});

test('every code with words of its own has them in English', () => {
  for (const code of ['not_found', 'invalid', 'forbidden', 'conflict', 'unavailable', 'timeout', 'service_gone', 'vetoed', 'plugin_off', 'too_large', 'internal', 'unknown']) {
    expect((SHELL_STRINGS as Record<string, string>)[`shell.err.${code}`], code).toBeTruthy();
  }
});
