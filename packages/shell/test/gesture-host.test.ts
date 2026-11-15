// The shell hands the kit the device's side of its gestures (ADR-0065 build step 7 seam, design/mobile.md §A.5–§A.6):
// the phone apps' haptics and native accessibility actions; the browser and the desktop app get no host at all.
import { describe, expect, it, vi } from 'vitest';
import { gestureHostOf } from '../src/platform/gesture-host.ts';
import { nullPlatform } from '../src/platform/index.ts';

describe('gestureHostOf', () => {
  it('a_browser_or_the_desktop_app_gets_no_gesture_host', () => {
    const haptic = vi.fn();
    for (const kind of ['browser', 'desktop'] as const) expect(gestureHostOf(nullPlatform({ kind, haptic }))).toBeUndefined();
    expect(haptic).not.toHaveBeenCalled();
  });

  it('the_phone_apps_hand_the_kit_their_haptics_and_accessibility_actions', () => {
    for (const kind of ['android', 'ios'] as const) {
      const off = vi.fn();
      const p = nullPlatform({ kind, phone: true, haptic: vi.fn(), accessibilityActions: vi.fn(() => off) });
      const host = gestureHostOf(p);
      host?.haptic('tap');
      expect(p.haptic).toHaveBeenCalledWith('tap');
      const el = document.createElement('div');
      const list = [{ label: 'Mark done', run: () => {} }];
      expect(host?.actions?.(el, list)).toBe(off);
      expect(p.accessibilityActions).toHaveBeenCalledWith(el, list);
    }
  });

  it('a_phone_app_without_native_actions_keeps_them_in_the_dom', () => {
    const host = gestureHostOf(nullPlatform({ kind: 'android', phone: true }));
    expect(host).toBeDefined();
    expect(host?.actions).toBeUndefined();
  });
});
