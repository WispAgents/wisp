// What My Stuff's page does with a kept thing, for the browser page (StuffPage) and the phone apps' screen (StuffPhone)
// alike: one action, one implementation (v1 stuff.js itemMenu). Each write is the plugin's command, whose optimistic
// patch shows at once and whose refusal rolls back with the shell's toast. The phone screen hands in `launched` (an
// app opens in its own frame inside wisp, F-stuff-10) and has Share (the platform's share sheet).
import { entityKey, ownerOf, type PluginClient, type SessionId } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import { copyImage } from '@wisp/ui';
import { reachable } from './preview/spec.ts';
import type { ActionId } from './actions.ts';
import { allowPane, launchApp, publishApp, removeItem, resetData, restoreItem, revealItem, stopApp, unpublishApp, updateItem } from './commands.ts';
import { fileUrl as itemFile } from './card.ts';
import { texty, ext } from './model.ts';
import type { Entry } from './items.ts';
import type { Pins } from './pins.tsx';
import './i18n/lazy.ts';

/** a link without its scheme, as v1 shows it */
const bare = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

export const fileUrl = (e: Entry): string => `/api/stuff/${encodeURIComponent(e.it.id)}/file`;

export interface PageRunDeps {
  readonly c: PluginClient;
  readonly shell: Pick<ShellContext, 'platform' | 'toast' | 'prompts'>;
  readonly pins: Pins;
  readonly entries: () => readonly Entry[];
  /** an app launched: where it runs (default: a new window, the browser's) */
  readonly launched?: (e: Entry, url: string) => void;
  /** what opening anything else does (default: a new window) */
  readonly opened?: (url: string) => void;
  /** the browser page: opens things inside wisp as v1 did (the viewer, the reader, an app's window); without it, a new
   *  window (the phone screen hands in `launched` and `opened`) */
  readonly show?: (s: Shown | undefined) => void;
  /** the browser page's Edit details… form */
  readonly edit?: (e: Entry) => void;
  /** Remove with Undo (v1): the page hides it at once and removes it once the toast has gone */
  readonly removeSoon?: (e: Entry, removeNow: () => void) => void;
  /** the pictures and videos shown now, in order (← → in the viewer go through them) */
  readonly media?: () => readonly Entry[];
}

/** What the browser page opens over itself (v1 openItem, launchApp). */
export type Shown =
  | { readonly kind: 'media'; readonly list: readonly { src: string; alt: string; video?: boolean }[]; readonly at: number }
  | { readonly kind: 'doc'; readonly e: Entry; readonly text: string }
  | { readonly kind: 'app'; readonly e: Entry; readonly url?: string; readonly src?: string };

/** What Share hands the platform: a link's address, an app's public link, else its words. */
export function shareData(e: Entry): { title: string; url?: string; text?: string } {
  const title = e.it.title || e.it.name;
  const url = e.it.kind === 'link' ? e.it.url : e.it.published?.url;
  return url ? { title, url } : { title, text: e.it.excerpt || title };
}

export function pageRun(d: PageRunDeps): { run: (e: Entry, id: ActionId) => void; upload: (files: FileList | readonly File[] | null) => void } {
  const { c, shell } = d;
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const win = (url: string): void => void window.open(url, '_blank', 'noopener');
  const opened = d.opened ?? win;
  const quiet = (p: Promise<unknown>): void => {
    p.catch(() => {}); // the shell's onError shows the rollback toast
  };
  const items = () => d.entries().map((x) => x.it);
  /** a file's text from its computer (a JSON file parses on the way: it's written back out) */
  const textOf = async (e: Entry): Promise<string> => {
    const body = await c.http(ownerOf(e.key)).send({ method: 'GET', path: `/stuff/${encodeURIComponent(e.it.id)}/file?r=${e.it.rev ?? 1}` });
    if (body === null) throw new Error(t('stuff.toast.gone'));
    return typeof body === 'string' ? body : JSON.stringify(body, null, 2);
  };
  const whenWords = (at: number): string => new Date(at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  // Open (v1 openItem): pictures and videos in the viewer, docs and code in the reader, an app in its window inside
  // wisp, links and PDFs in a new tab, anything else downloads
  const openHere = (e: Entry, show: (s: Shown | undefined) => void): void => {
    const it = e.it;
    if (it.pending) return;
    if (it.kind === 'app') return launchHere(e, show);
    if (it.kind === 'link') return win(it.url ?? '');
    if (it.kind === 'image' || it.kind === 'video') {
      const media = (d.media?.() ?? [e]).filter((x) => (x.it.kind === 'image' || x.it.kind === 'video') && !x.it.pending);
      const list = media.map((x) => ({ src: itemFile(x.it.id, x.it.rev), alt: x.it.title, ...(x.it.kind === 'video' && { video: true }) }));
      return show({ kind: 'media', list, at: Math.max(0, media.findIndex((x) => x.key === e.key)) });
    }
    if (texty(it)) {
      textOf(e).then(
        (text) => show({ kind: 'doc', e, text: it.kind === 'code' ? `\`\`\`${it.lang || ext(it).toLowerCase()}\n${text.replace(/\n$/, '')}\n\`\`\`` : text }),
        (err: unknown) => shell.toast.error(err instanceof Error ? err.message : String(err)),
      );
      return;
    }
    if (it.mime === 'application/pdf') return win(itemFile(it.id, it.rev));
    download(e);
  };
  // Launch (v1 launchApp): its window opens at once ("Starting…"); wisp serves it on a port of its own and it loads
  // there. A page that can't reach that port (another computer's wisp, no preview address): the address, copied.
  const launchHere = (e: Entry, show: (s: Shown | undefined) => void): void => {
    show({ kind: 'app', e });
    c.run(launchApp, { key: e.key, id: e.it.id }).then(
      (l) => {
        const src = reachable(l.url, document.URL, null);
        if (src) return show({ kind: 'app', e, url: l.url, src });
        show(undefined);
        void shell.platform.copy(l.url).then(() => shell.toast.show(t('stuff.app.elsewhere', { url: l.url })));
      },
      () => show(undefined),
    );
  };
  const download = (e: Entry): void => {
    const a = document.createElement('a');
    a.href = itemFile(e.it.id, e.it.rev, true);
    a.download = e.it.name;
    a.click();
  };
  // Use in a new task (v1 mentionInNew): New task, with the thing mentioned in its prompt box (the agent gets its file)
  const mention = (e: Entry): void => {
    c.navigate({ t: 'home' });
    let tries = 0;
    const fill = (): void => {
      const box = shell.prompts?.get('launcher');
      if (!box) return void (tries++ < 30 && requestAnimationFrame(fill));
      const was = box.text();
      const title = (e.it.title || e.it.name).replace(/[[\]()]/g, ' ').trim();
      box.set(`${was}${was && !/\s$/.test(was) ? ' ' : ''}@[${title}](stuff:${e.it.id}) `);
      box.focus();
    };
    requestAnimationFrame(fill);
  };
  const run = (e: Entry, id: ActionId): void => {
    const key = e.key;
    const args = { key, id: e.it.id };
    switch (id) {
      case 'open':
        if (d.show) return openHere(e, d.show);
        if (e.it.kind === 'app') {
          c.run(launchApp, args).then((l) => (d.launched ?? ((_e: Entry, u: string) => win(u)))(e, l.url), () => {});
        } else opened(e.it.kind === 'link' && e.it.url ? e.it.url : fileUrl(e));
        return;
      case 'mention':
        return mention(e);
      case 'edit':
        return d.edit?.(e);
      case 'restore': {
        const was = e.it.prev?.at ?? e.it.at;
        c.run(restoreItem, args).then(
          () => shell.toast.show(t('stuff.toast.restored', { when: whenWords(was) }), { action: { label: t('stuff.toast.undo'), onClick: () => run(e, 'restore') } }),
          () => {},
        );
        return;
      }
      case 'revoke':
        c.run(allowPane, { ...args, uses: [] }).then(() => shell.toast.show(t('stuff.toast.revoked', { title: e.it.title })), () => {});
        return;
      case 'resetdata':
        c.run(resetData, args).then(() => shell.toast.show(t(e.it.published ? 'stuff.toast.resetPublished' : 'stuff.toast.reset', { title: e.it.title })), () => {});
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
      // v1 copyItem: a link's address, a picture as a picture, else the file's text (not just its opening)
      case 'copy':
        if (e.it.kind === 'link') return void shell.platform.copy(e.it.url ?? '');
        if (e.it.kind === 'image') return void copyImage(itemFile(e.it.id, e.it.rev));
        textOf(e).then((text) => void shell.platform.copy(text), (err: unknown) => shell.toast.error(err instanceof Error ? err.message : String(err)));
        return;
      case 'download':
        return download(e);
      case 'star':
        return quiet(c.run(updateItem, { ...args, starred: !e.it.starred }));
      case 'task':
        if (e.it.from?.session) c.navigate({ t: 'openTask', id: entityKey(ownerOf(key), e.it.from.session) as unknown as SessionId });
        return;
      case 'folder':
        return quiet(c.run(revealItem, args));
      case 'remove':
        if (d.removeSoon) return d.removeSoon(e, () => quiet(c.run(removeItem, args)));
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
