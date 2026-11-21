// @vitest-environment happy-dom
// audit C #6: WebKit's ResizeObserver loop notice is handled, and only it: any other page error goes on as before.
import { afterEach, expect, test, vi } from 'vitest';
import { isObserverLoopNotice, quietObserverLoop } from '../src/observer-loop.ts';

let undo: (() => void) | undefined;
afterEach(() => undo?.());

test('the_resize_observer_loop_notice_is_handled_and_other_errors_still_reach_the_page_audit_c_6', () => {
  expect(isObserverLoopNotice('ResizeObserver loop completed with undelivered notifications.')).toBe(true);
  expect(isObserverLoopNotice('ResizeObserver loop limit exceeded')).toBe(true);
  expect(isObserverLoopNotice('TypeError: x is undefined')).toBe(false);
  expect(isObserverLoopNotice(undefined)).toBe(false);
  undo = quietObserverLoop(window);
  const later = vi.fn();
  window.addEventListener('error', later);
  const notice = new ErrorEvent('error', { message: 'ResizeObserver loop completed with undelivered notifications.', cancelable: true });
  window.dispatchEvent(notice);
  expect(notice.defaultPrevented).toBe(true);
  expect(later).not.toHaveBeenCalled();
  const other = new ErrorEvent('error', { message: 'TypeError: boom', cancelable: true });
  window.dispatchEvent(other);
  expect(other.defaultPrevented).toBe(false);
  expect(later).toHaveBeenCalledTimes(1);
  window.removeEventListener('error', later);
});
