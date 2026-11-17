// An app's own actions (v1 stuff.js: stopApp, publishApp, unpublishApp, copyPublic, resetData, Take back access): each a
// command whose refusal rolls back with the shell's toast, and the toasts v1 showed. Split from page-run.ts.
import type { PluginClient } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import type { ActionId } from './actions.ts';
import { allowPane, publishApp, resetData, stopApp, unpublishApp } from './commands.ts';
import { bare } from './model.ts';
import { win } from './open-here.ts';
import type { Entry } from './items.ts';
import './i18n/lazy.ts';

const APP: ReadonlySet<ActionId> = new Set(['stop', 'publish', 'update', 'unpublish', 'pubopen', 'pubcopy', 'revoke', 'resetdata']);
export const isAppAction = (id: ActionId): boolean => APP.has(id);

export function runApp(c: PluginClient, shell: Pick<ShellContext, 'platform' | 'toast'>, e: Entry, id: ActionId): void {
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const args = { key: e.key, id: e.it.id };
  const quiet = (p: Promise<unknown>): void => void p.catch(() => {}); // the shell's onError shows the rollback toast
  switch (id) {
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
      c.run(unpublishApp, args).then(() => shell.toast.show(t('stuff.page.unpublished', { at: bare(was) })), () => {});
      return;
    }
    case 'pubopen':
      if (e.it.published?.url) win(e.it.published.url);
      return;
    case 'pubcopy':
      void shell.platform.copy(e.it.published?.url ?? '').then((ok) => shell.toast.show(t(ok ? 'stuff.page.copied' : 'stuff.page.copyFailed')));
      return;
    case 'revoke':
      c.run(allowPane, { ...args, uses: [] }).then(() => shell.toast.show(t('stuff.toast.revoked', { title: e.it.title })), () => {});
      return;
    case 'resetdata':
      c.run(resetData, args).then(() => shell.toast.show(t(e.it.published ? 'stuff.toast.resetPublished' : 'stuff.toast.reset', { title: e.it.title })), () => {});
      return;
    default:
  }
}
