// Something shared to wisp from Android's share sheet (v1 web/js/features/share-in.js fromAndroid): New task, with it
// in the box and its files attached, ready to send. Nothing here starts a task, and nothing leaves the phone before
// Send. The manifest offered wisp in the share sheet and MainActivity held the share, but nothing in the bundle took it,
// so it sat there until the native side's toast said to update wisp on the computer.
//
// MainActivity says a share is here with a `wisp-share` event carrying its token, in the main frame of the app's own
// page only (a task's preview frame can reach the bridge but is never told the token). takeShare(token) claims it and
// says what came; each file comes a piece at a time (shareChunk, base64), then shareDone lets Android's copy go.
// While the shell's computer can't take tasks yet (none opened, or not connected) the share is left unclaimed, with a
// note to open a computer; once it can, askShare() has the event sent again. Loaded only in the Android app (app.tsx).

import { subscribe, type PluginClient, type Readable, type Scope } from '@wisp/client';
import { receiveShared } from '@wisp-plugin/stuff-ui/share-in';

/** MainActivity's share methods on `window.wispAndroid` (a @JavascriptInterface each: they answer at once). */
export interface ShareBridge {
  askShare?(): void;
  takeShare?(token: string): string;
  shareChunk?(token: string, file: number, offset: number): string;
  shareDone?(token: string): void;
}

export interface ShareWindow {
  readonly wispAndroid?: ShareBridge;
  addEventListener(type: string, fn: (e: Event) => void): void;
  removeEventListener(type: string, fn: (e: Event) => void): void;
}

export interface ShareInOptions {
  readonly window: ShareWindow;
  /** the shell's computer can take tasks (launch.ts tasksReady); absent: always */
  readonly ready?: Readable<boolean>;
  /** wisp.stuff's client on this shell: New task's draft (receiveShared) */
  readonly stuff: () => PluginClient | undefined;
  /** the files, as New task's attachments (ShellContext.prompts.attach) */
  readonly attach: (files: readonly File[]) => void;
  /** a note for the person (the toast) */
  readonly say: (text: string, tone?: 'bad') => void;
  readonly t: (key: string, vars?: Readonly<Record<string, string | number>>) => string;
  /** the listeners go with it (the shell this runs on) */
  readonly scope: Scope;
}

/** Whether this window has Android's share bridge. */
export const sharesHere = (w: ShareWindow): boolean => typeof w.wispAndroid?.takeShare === 'function';

interface Taken {
  readonly title?: string;
  readonly text?: string;
  readonly url?: string;
  readonly left?: number;
  readonly files?: readonly { readonly name?: string; readonly type?: string; readonly size?: number }[];
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

function bytesOf(b64: string): Uint8Array<ArrayBuffer> {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** One file through the bridge, a piece at a time, the page handed back between pieces. Short of its size is a
 *  failure, not the end: a newer share took over, or the phone couldn't read it. */
async function readFile(app: ShareBridge, token: string, i: number, f: NonNullable<Taken['files']>[number]): Promise<File> {
  const size = typeof f.size === 'number' && f.size > 0 ? f.size : 0;
  const parts: Uint8Array<ArrayBuffer>[] = [];
  for (let at = 0; at < size; ) {
    const bytes = bytesOf(app.shareChunk?.(token, i, at) ?? '');
    if (!bytes.length) throw new Error('short');
    parts.push(bytes);
    at += bytes.length;
    await new Promise((r) => setTimeout(r));
  }
  return new File(parts, str(f.name) || `shared-${i + 1}`, { type: str(f.type) });
}

export function followShares(o: ShareInOptions): void {
  const app = o.window.wispAndroid;
  if (!app || !sharesHere(o.window)) return;
  const ready = (): boolean => o.ready?.() ?? true;
  let waiting: string | undefined; // the token of a share left for a computer that can take it
  let taking = false;

  const claim = async (token: string): Promise<void> => {
    const raw = app.takeShare?.(token) ?? '';
    if (!raw) return; // another share's, taken already, or not ours: nothing is read
    taking = true;
    try {
      let got: Taken;
      try {
        got = JSON.parse(raw) as Taken;
      } catch {
        got = {};
      }
      const files: File[] = [];
      let failed = false;
      for (const [i, f] of (got.files ?? []).entries()) {
        try {
          files.push(await readFile(app, token, i, f));
        } catch {
          failed = true;
        }
      }
      app.shareDone?.(token); // Android's copy goes: nothing shared stays on the phone
      const c = o.stuff();
      if (!o.scope.alive || !c) return;
      const left = receiveShared(c, {
        title: str(got.title),
        text: str(got.text),
        url: str(got.url),
        ...(files.length > 0 && { files: files.length }),
        ...(typeof got.left === 'number' && got.left > 0 && { left: got.left }),
      });
      o.attach(files);
      if (failed) o.say(o.t('phone.share.unread'), 'bad');
      else if (left) o.say(left, 'bad');
    } finally {
      taking = false;
    }
  };

  const offered = (e: Event): void => {
    const token = str((e as CustomEvent<{ token?: unknown } | null>).detail?.token);
    if (!token || taking) return;
    if (!ready()) {
      // kept on the phone (not claimed) until a computer can take it: said once per share
      if (waiting !== token) o.say(o.t('phone.share.pick'));
      waiting = token;
      return;
    }
    waiting = undefined;
    void claim(token);
  };
  o.window.addEventListener('wisp-share', offered);
  o.scope.add(() => o.window.removeEventListener('wisp-share', offered));
  // a computer that can take it now: the share is offered again (MainActivity answers with the event)
  if (o.ready) o.scope.add(subscribe(o.ready, (now) => void (now && waiting !== undefined && app.askShare?.())));
  app.askShare?.(); // one that came before this page listened
}
