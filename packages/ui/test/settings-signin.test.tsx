// The owner's Settings → Agents (2026-11-19): a sign-in under way spelled out its whole OAuth address as a link, flush
// against the box's edge. The kit's box is v1's .sx-login: numbered steps, an "Open the sign-in page" button (the
// address never shown), the code with Copy, a paste box that sends its code.
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './helpers.tsx';
import { SettingsSignIn, type SettingsSignInLabels } from '../src/settings/SettingsSignIn.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

const L: SettingsSignInLabels = {
  starting: 'Starting the sign-in…',
  openStep: 'Open the sign-in page and sign in.',
  open: 'Open the sign-in page',
  codeStep: 'Enter this code there:',
  copy: 'Copy the code',
  waiting: 'Waiting for you to finish there…',
  pasteStep: 'Paste the code it shows you:',
  paste: 'Paste the code',
  send: 'Continue',
  checking: 'Checking the code…',
};
const URL = 'https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c&scope=org%3Acreate_api_key&state=xyz';

test('a_sign_in_never_spells_out_its_address_and_its_steps_copy_open_and_send', () => {
  const onOpen = vi.fn();
  const onCopy = vi.fn();
  const onSend = vi.fn();
  const device = mount(() => <SettingsSignIn url={URL} code="QZ5Y-ELHLW" labels={L} onOpen={onOpen} onCopy={onCopy} onSend={onSend} />);
  expect(device.textContent).not.toContain('claude.com');
  expect(device.textContent).toContain('Waiting for you to finish there…');
  (device.querySelector('[data-part~="settings.signin.open"]') as HTMLElement).click();
  expect(onOpen).toHaveBeenCalledWith(URL);
  (device.querySelector('[data-part~="settings.signin.copy"]') as HTMLElement).click();
  expect(onCopy).toHaveBeenCalledWith('QZ5Y-ELHLW');
  expect(device.querySelector('form')).toBeNull();

  const paste = mount(() => <SettingsSignIn url={URL} paste labels={L} onCopy={onCopy} onSend={onSend} />);
  const input = paste.querySelector('input') as HTMLInputElement;
  input.value = '  abc#123  ';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  (paste.querySelector('form') as HTMLFormElement).dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  expect(onSend).toHaveBeenCalledWith('abc#123');

  const starting = mount(() => <SettingsSignIn url="" labels={L} onCopy={onCopy} onSend={onSend} />);
  expect(starting.textContent).toContain('Starting the sign-in…');
  const failed = mount(() => <SettingsSignIn url={URL} error="error: authentication failed or timed out" labels={L} onCopy={onCopy} onSend={onSend} />);
  expect(failed.querySelector('[role="alert"]')?.textContent).toContain('authentication failed');
});

// Audit D1: Copy the code changed nothing on screen (v1 toasted "Code copied"; the owner's rule now: toasts only for
// errors). The button turns into a check, labelled Copied, for about 1.5 s, then back; a copy that fails turns back
// at once.
test('copy_the_code_turns_into_a_check_for_a_moment_and_back', async () => {
  vi.useFakeTimers();
  try {
    let ok = true;
    const onCopy = vi.fn(() => (ok ? Promise.resolve() : Promise.reject(new Error('denied'))));
    const box = mount(() => <SettingsSignIn url={URL} code="QZ5Y-ELHLW" labels={{ ...L, copied: 'Copied' }} onCopy={onCopy} onSend={() => {}} />);
    const copy = (): HTMLElement => box.querySelector('[data-part~="settings.signin.copy"]') as HTMLElement;
    expect(copy().getAttribute('aria-label')).toBe('Copy the code');
    copy().click();
    expect(onCopy).toHaveBeenCalledWith('QZ5Y-ELHLW');
    expect(copy().getAttribute('aria-label')).toBe('Copied');
    expect(copy().querySelector('[data-icon="check"]')).not.toBeNull();
    await vi.advanceTimersByTimeAsync(1600);
    expect(copy().getAttribute('aria-label')).toBe('Copy the code');
    ok = false;
    copy().click();
    expect(copy().getAttribute('aria-label')).toBe('Copied');
    await vi.advanceTimersByTimeAsync(0);
    expect(copy().getAttribute('aria-label')).toBe('Copy the code');
  } finally {
    vi.useRealTimers();
  }
});
