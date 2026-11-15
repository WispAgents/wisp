// What My Stuff's page does with a kept thing, for the browser page (StuffPage) and the phone apps' screen (StuffPhone)
// alike: one action, one implementation (v1 stuff.js itemMenu). Each write is the plugin's command, whose optimistic
// patch shows at once and whose refusal rolls back with the shell's toast. The phone screen hands in `launched` (an
// app opens in its own frame inside wisp, F-stuff-10) and has Share (the platform's share sheet).
import { entityKey, ownerOf, type PluginClient, type SessionId } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import type { ActionId } from './actions.ts';
import { launchApp, publishApp, removeItem, revealItem, stopApp, unpublishApp, updateItem } from './commands.ts';
import type { Entry } from './items.ts';
import type { Pins } from './pins.tsx';
import './i18n/lazy.ts';

/** a link without its scheme, as v1 shows it */
const bare = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

export const fileUrl = (e: Entry): string => `/api/stuff/${encodeURIComponent(e.it.id)}/file`;

export interface PageRunDeps {
  readonly c: PluginClient;
  readonly shell: Pick<ShellContext, 'platform' | 'toast'>;
  readonly pins: Pins;
  readonly entries: () => readonly Entry[];
  /** an app launched: where it runs (default: a new window, the browser's) */
  readonly launched?: (e: Entry, url: string) => void;
  /** what opening anything else does (default: a new window) */
  readonly opened?: (url: string) => void;
}

/** What Share hands the platform: a link's address, an app's public link, else its words. */
export function shareData(e: Entry): { title: string; url?: string; text?: string } {
  const title = e.it.title || e.it.name;
  const url = e.it.kind === 'link' ? e.it.url : e.it.published?.url;
  return url ? { title, url } : { title, text: e.it.excerpt || title };
}

export function pageRun(d: PageRunDeps): { run: (e: Entry, id: ActionId) => void; upload: (files: FileList | null) => void } {
  const { c, shell } = d;
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const win = (url: string): void => void window.open(url, '_blank', 'noopener');
  const opened = d.opened ?? win;
  const quiet = (p: Promise<unknown>): void => {
    p.catch(() => {}); // the shell's onError shows the rollback toast
  };
  const items = () => d.entries().map((x) => x.it);
  const run = (e: Entry, id: ActionId): void => {
    const key = e.key;
    const args = { key, id: e.it.id };
    switch (id) {
      case 'open':
        if (e.it.kind === 'app') {
          c.run(launchApp, args).then((l) => (d.launched ?? ((_e: Entry, u: string) => win(u)))(e, l.url), () => {});
        } else opened(e.it.kind === 'link' && e.it.url ? e.it.url : fileUrl(e));
        return;
      case 'share': {
        const data = shareData(e);
        void shell.platform.share(data).then((r) => {
          if (r !== 'unsupported') return;
          // no share sheet here: the same words on the clipboard
          void shell.platform.copy(data.url ?? data.text ?? '').then((ok) => shell.toast.show(t(ok ? 'stuff.page.shareCopied' : 'stuff.page.copyFailed')));
        });
        return;
      }
      case 'stop':
        return quiet(c.run(stopApp, args));
      // Publish (again): a link anyone can open on your grid; its row says publishing until the grid answers
      case 'publish':
      case 'update': {
        const again = !!e.it.published;
        c.run(publishApp, args).then(
          (next) => {
            const url = next.published?.url;
            if (!url) return;
            void shell.platform.copy(url).then((copied) =>
              shell.toast.success(t(again ? 'stuff.page.updated' : 'stuff.page.published', { at: bare(url) }), { ...(copied && { description: t('stuff.page.linkCopied') }), action: { label: t('stuff.page.openLink'), onClick: () => win(url) } }),
            );
          },
          () => {},
        );
        return;
      }
      case 'unpublish': {
        const was = e.it.published?.url ?? '';
        c.run(unpublishApp, args).then(
          () => shell.toast.show(t('stuff.page.unpublished', { at: bare(was) })),
          () => {},
        );
        return;
      }
      case 'pubopen':
        if (e.it.published?.url) win(e.it.published.url);
        return;
      case 'pubcopy':
        void shell.platform.copy(e.it.published?.url ?? '').then((ok) => shell.toast.show(t(ok ? 'stuff.page.copied' : 'stuff.page.copyFailed')));
        return;
      case 'copy':
        void navigator.clipboard.writeText(e.it.kind === 'link' ? (e.it.url ?? '') : (e.it.excerpt ?? '')).catch(() => {});
        return;
      case 'download': {
        const a = document.createElement('a');
        a.href = fileUrl(e);
        a.download = e.it.name;
        a.click();
        return;
      }
      case 'star':
        return quiet(c.run(updateItem, { ...args, starred: !e.it.starred }));
      case 'task':
        if (e.it.from?.session) c.navigate({ t: 'openTask', id: entityKey(ownerOf(key), e.it.from.session) as unknown as SessionId });
        return;
      case 'folder':
        return quiet(c.run(revealItem, args));
      case 'remove':
        return quiet(c.run(removeItem, args));
      // Pin on home (v1 panes.js pinApp): the Allow card first when it asks to use wisp
      case 'pin-home':
        return quiet(d.pins.pin(key, e.it, 'home', items()));
      case 'unpin-home':
        return quiet(d.pins.unpin(e.it, 'home', items()));
      // Pin beside tasks: a tab in the panel beside every task (v1 panel.js)
      case 'pin-task':
        return quiet(d.pins.pin(key, e.it, 'task', items()));
      case 'unpin-task':
        return quiet(d.pins.unpin(e.it, 'task', items()));
      default:
    }
  };

  const upload = (files: FileList | null): void => {
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
