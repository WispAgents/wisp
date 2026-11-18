// The Tauri desktop app (desktop/src-tauri). Its init.js already backs window.Notification with
// tauri-plugin-notification and opens the task on a click, so notifications take the browser's
// road. What differs: links open in the system browser (tauri-plugin-opener), the dock badge is the
// window's, and the app's own focus event is the resume signal. A command the native side hasn't
// wired answers like the browser would.

import { browserPlatform } from './browser.ts';
import type { Platform, PlatformWindow } from './types.ts';

export function desktopPlatform(w: PlatformWindow): Platform {
  const base = browserPlatform(w);
  const invoke = (cmd: string, args?: Record<string, unknown>): Promise<unknown> =>
    w.__TAURI_INTERNALS__ ? w.__TAURI_INTERNALS__.invoke(cmd, args) : Promise.reject(new Error('not in Tauri'));
  const mac = /Mac/.test(w.navigator.platform);
  return {
    ...base,
    kind: 'desktop',
    mobile: false,
    standalone: true,
    titlebar: mac ? 'mac' : 'none',
    // WebKit has no -webkit-app-region: a press on a region the frame marks [data-drag] (the top bar,
    // the rail, a task's head) moves the window, and a double press zooms it, as v1's init.js did; the
    // nearest control (a button, a link, a field, or [data-no-drag]) keeps its press. A [data-drag-click]
    // region (a task's title: a click renames it) is both: a press that moves drags the window, one that
    // doesn't is its click (audit A #18: the title was most of the head, and the head's middle didn't drag).
    dragRegions(root) {
      if (!mac) return () => {};
      const down = (e: Event): void => {
        const m = e as MouseEvent;
        if (m.button !== 0 || m.ctrlKey || !(m.target instanceof Element)) return;
        const hit = m.target.closest('[data-drag], [data-drag-click], [data-no-drag], button, a, input, textarea, select, [role="button"], [contenteditable="true"], [contenteditable="plaintext-only"]');
        // being edited, its press selects text
        if (hit?.hasAttribute('data-drag-click')) return void (!(hit as HTMLElement).isContentEditable && dragOnMove(m));
        if (!hit?.hasAttribute('data-drag')) return;
        m.preventDefault();
        invoke(m.detail === 2 ? 'zoom_window' : 'start_drag').catch(() => {});
      };
      // drags once the pointer moves past a click's wobble with the button still held; a release before that (or one the
      // page never saw: the button is up at the next move) leaves the click alone. One press watched at a time.
      let pressing: (() => void) | undefined;
      const dragOnMove = (start: MouseEvent): void => {
        pressing?.();
        const doc = root.ownerDocument;
        const move = (e: Event): void => {
          const m = e as MouseEvent;
          if ((m.buttons & 1) === 0) return end();
          if (Math.abs(m.clientX - start.clientX) + Math.abs(m.clientY - start.clientY) < 4) return;
          end();
          invoke('start_drag').catch(() => {});
        };
        const end = (): void => {
          doc.removeEventListener('mousemove', move, true);
          doc.removeEventListener('mouseup', end, true);
          pressing = undefined;
        };
        pressing = end;
        doc.addEventListener('mousemove', move, true);
        doc.addEventListener('mouseup', end, true);
      };
      root.addEventListener('mousedown', down, true);
      return () => {
        root.removeEventListener('mousedown', down, true);
        pressing?.();
      };
    },
    async badge(count) {
      try {
        await invoke('plugin:window|set_badge_count', { label: 'main', value: count > 0 ? count : null });
        return 'set';
      } catch {
        return base.badge(count);
      }
    },
    // desktop/src-tauri main.rs pick_folder: the native dialog (NSOpenPanel on a Mac)
    async pickFolder() {
      const dir = await invoke('pick_folder').catch(() => null);
      return typeof dir === 'string' && dir ? dir : null;
    },
    openExternal(url) {
      invoke('plugin:opener|open_url', { url }).catch(() => base.openExternal(url));
    },
    // no in-app browser on a computer: the system browser, as openExternal
    openInApp(url) {
      invoke('plugin:opener|open_url', { url }).catch(() => base.openExternal(url));
    },
  };
}
