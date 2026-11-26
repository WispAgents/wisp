// @vitest-environment happy-dom
// Share again on a password-locked task with nothing typed made the link public: share.create sent the grid
// `{kind: anyone}`. The dialog sends no passkey then (share.create sends no access, and the grid keeps its password),
// and says the passkey is kept; Open sends '' to take it off; a typed passkey goes as typed.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { KitProvider, englishT } from '@wisp/ui';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { WhoPicker } from '../src/share/DialogParts.tsx';
import { passkeyArg } from '../src/share/model.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const S = lazy as Record<string, string>;
const t = (k: string): string => S[k] ?? k;

test('locked + empty passkey sends no passkey and shows the kept-passkey placeholder', () => {
  expect(passkeyArg('pass', '  ', true)).toEqual({});
  const host = document.createElement('div');
  document.body.append(host);
  render(() => (
    <KitProvider t={englishT}>
      <WhoPicker t={t} who="pass" onWho={() => {}} passkey="" onPasskey={() => {}} kept />
    </KitProvider>
  ), host);
  expect(host.querySelector('input[aria-label]')?.getAttribute('placeholder')).toBe('Kept: type a new one to change it');
});

test('open takes the passkey off, a typed one locks it, and a new lock needs one typed', () => {
  expect(passkeyArg('link', 'ignored', true)).toEqual({ passkey: '' });
  expect(passkeyArg('pass', ' pw ', true)).toEqual({ passkey: 'pw' });
  expect(passkeyArg('pass', '', false)).toBeUndefined();
});
