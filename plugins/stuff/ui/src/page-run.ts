// What My Stuff's page does with a kept thing, for the browser page (StuffPage) and the phone apps' screen (StuffPhone)
// alike: one action, one implementation (v1 stuff.js itemMenu). Each write is the plugin's command, whose optimistic
// patch shows at once and whose refusal rolls back with the shell's toast. The phone screen hands in `launched` (an
// app opens in its own frame inside wisp, F-stuff-10) and has Share (the platform's share sheet). Opening inside wisp
// is open-here.ts, an app's own actions app-run.ts.
import { entityKey, ownerOf, type PluginClient, type SessionId } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import type { ActionId } from './actions.ts';
import { isAppAction, runApp } from './app-run.ts';
import { launchApp, removeItem, restoreItem, revealItem, updateItem } from './commands.ts';
import { openers, win, type Shown } from './open-here.ts';
import type { Entry } from './items.ts';
import type { Pins } from './pins.tsx';
import './i18n/lazy.ts';
import { opensHere } from './opened.ts';

export type { Shown } from './open-here.ts';

export const fileUrl = (e: Entry): string => `/api/stuff/${encodeURIComponent(e.it.id)}/file`;

export interface PageRunDeps {
  readonly c: PluginClient;
  readonly shell: Pick<ShellContext, 'platform' | 'toast' | 'prompts' | 'previewAddresses'>;
  readonly pins: Pins;
  readonly entries: () => readonly Entry[];
  /** an app launched: where it runs (default: a new window, the browser's) */
  readonly launched?: (e: Entry, url: string) => void;
  /** what opening anything else does (default: a new window) */
  readonly opened?: (url: string) => void;
  /** opens things inside wisp as v1 did (the viewer, the reader, an app's window); without it, a new window. The
   *  phone screen hands it in too, with `launched` for its apps */
  readonly show?: (s: Shown | undefined) => void;
  /** the browser page's Edit details…, by the card */
  readonly edit?: (e: Entry) => void;
  /** Remove with Undo (v1): the page hides it at once and removes it once the toast has gone */
  readonly removeSoon?: (e: Entry, removeNow: () => void) => void;
  /** the pictures and videos shown now, in order (← → in the viewer go through them) */
  readonly media?: () => readonly Entry[];
  /** what an action came from (⋯ on the card): the Allow card sits by it, as in v1 */
  readonly anchor?: (e: Entry) => HTMLElement | undefined;
}

/** What Share hands the platform: a link's address, an app's public link, else its words. */
export function shareData(e: Entry): { title: string; url?: string; text?: string } {
  const title = e.it.title || e.it.name;
  const url = e.it.kind === 'link' ? e.it.url : e.it.published?.url;
  return url ? { title, url } : { title, text: e.it.excerpt || title };
}

const whenWords = (at: number): string => new Date(at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });

export function pageRun(d: PageRunDeps): { run: (e: Entry, id: ActionId) => void; upload: (files: FileList | readonly File[] | null) => void } {
  const { c, shell } = d;
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const quiet = (p: Promise<unknown>): void => void p.catch(() => {}); // the shell's onError shows the rollback toast
  const items = () => d.entries().map((x) => x.it);
  const here = openers({ c, shell, show: (s) => d.show?.(s), ...(d.media && { media: d.media }) });
  // the phone screen: an app in its own frame, anything else in a new window
  const openThere = (e: Entry): void => {
    if (e.it.kind !== 'app') return (d.opened ?? win)(e.it.kind === 'link' && e.it.url ? e.it.url : fileUrl(e));
    c.run(launchApp, { key: e.key, id: e.it.id }).then((l) => (d.launched ?? ((_e: Entry, u: string) => win(u)))(e, l.url), () => {});
  };
  const share = (e: Entry): void => {
    const data = shareData(e);
    void shell.platform.share(data).then((r) => {
      if (r !== 'unsupported') return;
      // no share sheet here: the same words on the clipboard
      void shell.platform.copy(data.url ?? data.text ?? '').then((ok) => shell.toast.show(t(ok ? 'stuff.page.shareCopied' : 'stuff.page.copyFailed')));
    });
  };
  const restore = (e: Entry): void => {
    const was = e.it.prev?.at ?? e.it.at;
    c.run(restoreItem, { key: e.key, id: e.it.id }).then(() => shell.toast.show(t('stuff.toast.restored', { when: whenWords(was) }), { action: { label: t('stuff.toast.undo'), onClick: () => restore(e) } }), () => {});
  };
  const pin = (e: Entry, where: 'home' | 'task', on: boolean): void =>
    quiet(on ? d.pins.pin(e.key, e.it, where, items(), d.anchor?.(e)) : d.pins.unpin(e.it, where, items()));
  const run = (e: Entry, id: ActionId): void => {
    if (isAppAction(id)) return runApp(c, shell, e, id);
    const args = { key: e.key, id: e.it.id };
    switch (id) {
      case 'open':
        // the phone screen shows its apps over the whole screen (`launched`); everything else opens here when it can
        return opensHere(e.it.kind, d) ? here.open(e) : openThere(e);
      case 'mention':
        return here.mention(e);
      case 'edit':
        return d.edit?.(e);
      case 'restore':
        return restore(e);
      case 'share':
        return share(e);
      case 'copy':
        return here.copy(e);
      case 'download':
        return here.download(e);
      case 'star':
        return quiet(c.run(updateItem, { ...args, starred: !e.it.starred }));
      case 'task':
        if (e.it.from?.session) c.navigate({ t: 'openTask', id: entityKey(ownerOf(e.key), e.it.from.session) as unknown as SessionId });
        return;
      case 'folder':
        return quiet(c.run(revealItem, args));
      case 'remove':
        if (d.removeSoon) return d.removeSoon(e, () => quiet(c.run(removeItem, args)));
        return quiet(c.run(removeItem, args));
      // Pins (v1 panes.js pinApp): on home, or a tab beside every task; the Allow card first when it asks to use wisp
      case 'pin-home':
      case 'unpin-home':
        return pin(e, 'home', id === 'pin-home');
      case 'pin-task':
      case 'unpin-task':
        return pin(e, 'task', id === 'pin-task');
      default:
    }
  };

  const upload = (files: FileList | readonly File[] | null): void => {
    const all = Array.from(files ?? []);
    all.forEach((f, i) => {
      // the bytes go to My Stuff's upload route on this computer; its rows follow on the stuff topic. The first says how
      // many came together (`of`): v1's one stuff_add per drop
      const of = i === 0 ? `&of=${all.length}` : '';
      c.http(c.home)
        .send({ method: 'POST', path: `/stuff/upload?name=${encodeURIComponent(f.name)}&type=${encodeURIComponent(f.type)}${of}`, body: f, contentType: 'application/octet-stream' })
        .catch((e: unknown) => shell.toast.error(e instanceof Error ? e.message : String(e)));
    });
  };
  return { run, upload };
}
