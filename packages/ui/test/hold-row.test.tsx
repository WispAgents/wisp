// Ask near the limit's row (composer/HoldRow; v1 features/hold.js showHold): its keys while it shows, and its place in
// New task (under the bar, recipes hidden) and the reply box (over the text).
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { HoldRow, HOLD_ENTER_GRACE_MS } from '../src/composer/HoldRow.tsx';
import { ReplyBox } from '../src/app/Task.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
  vi.useRealTimers();
});

function mount(wait = true): { box: HTMLTextAreaElement; go: ReturnType<typeof vi.fn>; waitFn: ReturnType<typeof vi.fn>; keep: ReturnType<typeof vi.fn>; host: HTMLElement } {
  const host = document.createElement('div');
  document.body.append(host);
  const box = document.createElement('textarea');
  host.append(box);
  const go = vi.fn();
  const waitFn = vi.fn();
  const keep = vi.fn();
  dispose = render(
    () => (
      <HoldRow
        place="launcher"
        say="Claude Code’s weekly window is at 91%, past your 90% line."
        goLabel="Start anyway"
        keepLabel="Keep editing (Esc)"
        {...(wait && { wait: { label: 'Wait until Wed 14:00' } })}
        onGo={go}
        onWait={waitFn}
        onKeep={keep}
        box={() => box}
      />
    ),
    host,
  );
  return { box, go, waitFn, keep, host };
}

const key = (el: HTMLElement, init: KeyboardEventInit): KeyboardEvent => {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  el.dispatchEvent(e);
  return e;
};

test('the row says why, offers Start anyway, Wait until the reset and Keep editing, and its buttons go', () => {
  const { host, go, waitFn, keep } = mount();
  expect(host.querySelector('[data-kit="hold-say"]')?.textContent).toBe('Claude Code’s weekly window is at 91%, past your 90% line.');
  (host.querySelector('[data-part="ui.hold.go"]') as HTMLButtonElement).click();
  (host.querySelector('[data-part="ui.hold.wait"]') as HTMLButtonElement).click();
  (host.querySelector('[data-part="ui.hold.keep"]') as HTMLButtonElement).click();
  expect([go.mock.calls.length, waitFn.mock.calls.length, keep.mock.calls.length]).toEqual([1, 1, 1]);
});

test('no Wait until when the reset is not known', () => {
  const { host } = mount(false);
  expect(host.querySelector('[data-part="ui.hold.wait"]')).toBeNull();
});

test('Enter in the box goes anyway, but not in its first 300 ms and not as a held-down repeat (v1: a double Enter does not slip through)', () => {
  vi.useFakeTimers({ toFake: ['performance'] });
  const { box, go } = mount();
  const e = key(box, { key: 'Enter' });
  expect(e.defaultPrevented).toBe(true);
  expect(go).not.toHaveBeenCalled();
  vi.advanceTimersByTime(HOLD_ENTER_GRACE_MS + 1);
  key(box, { key: 'Enter', repeat: true });
  expect(go).not.toHaveBeenCalled();
  key(box, { key: 'Enter', shiftKey: true });
  expect(go).not.toHaveBeenCalled();
  key(box, { key: 'Enter' });
  expect(go).toHaveBeenCalledTimes(1);
});

test('Alt+Enter waits for the reset, Esc keeps editing', () => {
  const { box, waitFn, keep } = mount();
  key(box, { key: 'Enter', altKey: true });
  expect(waitFn).toHaveBeenCalledTimes(1);
  key(box, { key: 'Escape' });
  expect(keep).toHaveBeenCalledTimes(1);
});

test('the reply box shows the row over its text (v1 .c-hold)', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <ReplyBox
        placeholder="Follow up"
        value="and this"
        onInput={() => {}}
        onSend={() => {}}
        attach="Attach"
        mode={{ label: 'Auto', title: 'Mode' }}
        runner={{ title: 'Claude Code' }}
        team="Team"
        goal="Goal"
        later="Later"
        send="Send"
        hold={{ say: 'Claude Code’s 5-hour window is at 92%.', goLabel: 'Send anyway', keepLabel: 'Keep editing', onGo: () => {}, onKeep: () => {} }}
      />
    ),
    host,
  );
  const row = host.querySelector('[data-part="composer.hold"]') as HTMLElement;
  expect(row.dataset['place']).toBe('reply');
  const text = host.querySelector('[data-kit="reply"]') as HTMLElement;
  expect(row.compareDocumentPosition(text) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});
