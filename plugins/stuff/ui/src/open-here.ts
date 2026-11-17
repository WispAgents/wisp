// Opening a kept thing inside wisp, as v1's My Stuff did (stuff.js openItem, launchApp, copyItem, mentionInNew):
// pictures and videos in the viewer, docs and code in the reader, an app in its window, links and PDFs in a new tab,
// anything else downloads. Split from page-run.ts, which runs every action; this is the part that reads files and opens
// windows.
import { ownerOf, type PluginClient } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import { copyImage } from '@wisp/ui';
import { launchApp } from './commands.ts';
import { fileUrl as itemFile } from './card.ts';
import { ext, texty } from './model.ts';
import { appAddress } from './opened.ts';
import type { Entry } from './items.ts';
import './i18n/lazy.ts';

/** What the browser page opens over itself (v1 openItem, launchApp). */
export type Shown =
  | { readonly kind: 'media'; readonly list: readonly { src: string; alt: string; video?: boolean }[]; readonly at: number }
  | { readonly kind: 'doc'; readonly e: Entry; readonly text: string }
  | { readonly kind: 'app'; readonly e: Entry; readonly url?: string; readonly src?: string };

export interface OpenDeps {
  readonly c: PluginClient;
  readonly shell: Pick<ShellContext, 'platform' | 'toast' | 'prompts' | 'previewAddresses'>;
  readonly show: (s: Shown | undefined) => void;
  /** the pictures and videos shown now, in order (← → in the viewer go through them) */
  readonly media?: () => readonly Entry[];
}

export const win = (url: string): void => void window.open(url, '_blank', 'noopener');

export function openers(d: OpenDeps) {
  const { c, shell } = d;
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const fail = (err: unknown): void => void shell.toast.error(err instanceof Error ? err.message : String(err));
  /** a file's text from its computer (a JSON file parses on the way: it's written back out) */
  const textOf = async (e: Entry): Promise<string> => {
    const body = await c.http(ownerOf(e.key)).send({ method: 'GET', path: `/stuff/${encodeURIComponent(e.it.id)}/file?r=${e.it.rev ?? 1}` });
    if (body === null) throw new Error(t('stuff.toast.gone'));
    return typeof body === 'string' ? body : JSON.stringify(body, null, 2);
  };
  const download = (e: Entry): void => {
    const a = document.createElement('a');
    a.href = itemFile(e.it.id, e.it.rev, true);
    a.download = e.it.name;
    a.click();
  };
  // the item's own computer runs it: a mapping keyed by the item is asked about that computer (ownerOf)
  const maps = (e: Entry): ((url: string) => string | undefined)[] => (shell.previewAddresses?.()() ?? []).map((p) => p.map({ client: p.client, task: e.key }));
  // Launch (v1 launchApp): its window opens at once ("Starting…"); wisp serves it on a port of its own and it loads
  // there. A page that can't reach that port, with no preview address for it: the address, copied, and where it runs.
  const launch = (e: Entry): void => {
    d.show({ kind: 'app', e });
    c.run(launchApp, { key: e.key, id: e.it.id }).then(
      (l) => {
        const src = appAddress(l.url, document.URL, maps(e));
        if (src) return d.show({ kind: 'app', e, url: l.url, src });
        d.show(undefined);
        void shell.platform.copy(l.url).then(() => shell.toast.show(t('stuff.app.elsewhere', { url: l.url })));
      },
      () => d.show(undefined),
    );
  };
  const open = (e: Entry): void => {
    const it = e.it;
    if (it.pending) return;
    if (it.kind === 'app') return launch(e);
    if (it.kind === 'link') return win(it.url ?? '');
    if (it.kind === 'image' || it.kind === 'video') {
      const media = (d.media?.() ?? [e]).filter((x) => (x.it.kind === 'image' || x.it.kind === 'video') && !x.it.pending);
      const list = media.map((x) => ({ src: itemFile(x.it.id, x.it.rev), alt: x.it.title, ...(x.it.kind === 'video' && { video: true }) }));
      return d.show({ kind: 'media', list, at: Math.max(0, media.findIndex((x) => x.key === e.key)) });
    }
    if (texty(it)) {
      const fence = (text: string): string => `\`\`\`${it.lang || ext(it).toLowerCase()}\n${text.replace(/\n$/, '')}\n\`\`\``;
      textOf(e).then((text) => d.show({ kind: 'doc', e, text: it.kind === 'code' ? fence(text) : text }), fail);
      return;
    }
    if (it.mime === 'application/pdf') return win(itemFile(it.id, it.rev));
    download(e);
  };
  // v1 copyItem: a link's address, a picture as a picture, else the file's text (not just its opening)
  const copy = (e: Entry): void => {
    if (e.it.kind === 'link') return void shell.platform.copy(e.it.url ?? '');
    if (e.it.kind === 'image') return void copyImage(itemFile(e.it.id, e.it.rev));
    textOf(e).then((text) => void shell.platform.copy(text), fail);
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
  return { open, copy, download, mention };
}
