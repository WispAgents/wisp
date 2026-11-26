// What the installed web app's share sheet sent (v1 web/js/features/share-in.js fromServiceWorker): the worker kept the
// form (sw.ts, share-form.ts keep) and opened the app on `/?shared=<id>`; this reads it once, into New task, as the
// Android app's bridge does (mobile/web share-in.ts). Nothing here starts a task, and nothing leaves the phone before
// Send. Loaded with import() after the first screen, and only on a page opened with `?shared` (which the navigation
// takes off the URL at start: a reload never reads it again).

import type { PluginClient } from '@wisp/client';
import { take, type CacheStore } from '@wisp/shell/share-form';
import { receiveShared } from '@wisp-plugin/stuff-ui/share-in';

export interface SharedOptions {
  /** the `?shared=` the page opened with (Navigation.takeShared: it is gone from the URL already) */
  readonly id: string;
  readonly caches: CacheStore;
  /** wisp.stuff's client on this shell: New task's draft (receiveShared) */
  readonly stuff: () => PluginClient | undefined;
  /** the files, as New task's attachments (ShellContext.prompts.attach) */
  readonly attach: (files: readonly File[]) => void;
  /** the files left out, in words (the toast) */
  readonly say: (text: string) => void;
}

export async function readShared(o: SharedOptions): Promise<void> {
  // 'none': the worker couldn't read the form; the app opened all the same, with nothing to take
  if (!o.id || o.id === 'none') return;
  const c = o.stuff();
  if (!c) return;
  const got = await take(o.id, o.caches);
  if (!got) return;
  const left = receiveShared(c, {
    title: got.title,
    text: got.text,
    url: got.url,
    ...(got.files.length > 0 && { files: got.files.length }),
    ...(got.left > 0 && { left: got.left }),
  });
  if (got.files.length) o.attach(got.files);
  if (left) o.say(left);
}
