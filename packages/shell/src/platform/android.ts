// The Capacitor Android app (mobile/android). Its Java stays as it is (plan §7): WispSsh opens links
// outside, and the stock Capacitor plugins (Share, Haptics, LocalNotifications) are used when the
// build has them. Anything not wired yet falls back to the WebView's own browser APIs, which on
// Android cover share, vibrate and the file picker.

import { browserPlatform } from './browser.ts';
import { capacitorPlatform, pluginFn } from './capacitor.ts';
import type { Dictation, DictationEvent, Platform, PlatformWindow } from './types.ts';

/** The app's speech recognizer (Dictation.java): `wisp-dictation` events on the window, one listener at a time.
 * An app without it (older) or a phone without a speech service: none, so no mic shows. */
export function androidDictation(w: PlatformWindow): Dictation | undefined {
  const app = w.wispAndroid;
  const { dictate, dictateStop } = app ?? {};
  if (!app || typeof dictate !== 'function' || typeof dictateStop !== 'function' || app.dictation?.() !== true) return undefined;
  let cur: ((e: DictationEvent) => void) | undefined;
  w.addEventListener('wisp-dictation', (e) => {
    const d = (e as CustomEvent<{ state?: string; text?: string; error?: string } | null>).detail ?? {};
    const fn = cur;
    if (!fn || !d.state) return;
    if (d.state === 'partial' || d.state === 'final') fn({ state: d.state, text: d.text ?? '' });
    else if (d.state === 'error') fn({ state: 'error', error: (d.error || 'failed') as never, ...(d.text ? { text: d.text } : {}) });
    else if (d.state === 'listening' || d.state === 'end') fn({ state: d.state });
    if (d.state === 'end' || d.state === 'error') cur = undefined;
  });
  return {
    start(onEvent) {
      if (cur) {
        cur({ state: 'end' }); // the box that was listening lets go
        dictateStop.call(app);
      }
      cur = onEvent;
      dictate.call(app);
      return () => {
        if (cur === onEvent) dictateStop.call(app); // the final result still comes, then `end`
      };
    },
  };
}

export function androidPlatform(w: PlatformWindow): Platform {
  const base = browserPlatform(w);
  const open = pluginFn(w.Capacitor, 'WispSsh', 'openExternal');
  const openExternal = (url: string): void => {
    if (open) void open({ url }).catch(() => base.openExternal(url));
    else base.openExternal(url);
  };
  const dictation = androidDictation(w);
  return {
    ...capacitorPlatform(w, 'android', openExternal),
    // an older app has no camera handler: its attach button stays the file picker
    camera: w.wispAndroid?.camera?.() === true,
    ...(dictation && { dictation }),
  };
}
