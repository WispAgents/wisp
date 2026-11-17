// The trusted themes' styles on the page (ADR-0007): one <style> in <head>, `@layer plugins` (part-styles.ts),
// recomputed whenever what's on or trusted may have changed — the home wisp's `plugins.changed` notice (a plugin
// switched, trusted, installed or removed) or the link coming back — and gone with the scope that owns it. An
// answer that comes after a newer request is dropped, and a failed read keeps what's on the page.

import { defineCommand, type Client, type PeerId, type PluginClient, type Scope } from '@wisp/client';
import { partStylesCss, type PluginStyles } from './part-styles.ts';

export const STYLE_ID = 'wisp-plugin-styles';
/** The notice the plugins plugin sends when what `plugins.styles` answers may have changed. */
export const CHANGED = 'plugins.changed';

export interface PluginStylesDeps {
  readonly document: Pick<Document, 'createElement' | 'getElementById' | 'head'>;
  readonly scope: Scope;
  load(): Promise<readonly PluginStyles[]>;
  /** calls `fn` whenever the styles should be read again; returns the unsubscribe */
  onChange(fn: () => void): () => void;
}

export interface PluginStylesHandle {
  refresh(): Promise<void>;
  /** the stylesheet on the page now ('' when no trusted plugin has styles) */
  css(): string;
}

export function mountPluginStyles(d: PluginStylesDeps): PluginStylesHandle {
  let asked = 0;
  let css = '';
  const write = (text: string): void => {
    css = text;
    const had = d.document.getElementById(STYLE_ID);
    if (text === '') return void had?.remove();
    const el = had ?? d.document.createElement('style');
    el.id = STYLE_ID;
    if (el.textContent !== text) el.textContent = text;
    if (!had) d.document.head.append(el); // last in <head>; the layer, not the order, decides who wins
  };
  const refresh = async (): Promise<void> => {
    const mine = ++asked;
    let list: readonly PluginStyles[];
    try {
      list = await d.load();
    } catch {
      return; // offline or refused: what's on the page stays
    }
    if (mine === asked && d.scope.alive) write(partStylesCss(Array.isArray(list) ? list : []));
  };
  d.scope.add(d.onChange(() => void refresh()));
  d.scope.add(() => {
    asked += 1; // an answer still on its way lands nowhere
    write('');
  });
  return { refresh, css: () => css };
}

// quiet: a peer without it (an older wisp, a remote peer) has no plugin styles, never an error toast (QA r2 phone #1)
const stylesCmd = defineCommand<{ peer: PeerId }, PluginStyles[]>({ id: 'plugins.styles', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'none: the page restyles when it lands', quiet: true });

/** Wired to the home wisp: read when its link comes up (and back), and on every `plugins.changed`; `run` is the
 * shell's own plugin client (the read runs as the shell). */
export function pluginStyles(client: Client, run: PluginClient, home: PeerId, document: PluginStylesDeps['document'], scope: Scope): PluginStylesHandle {
  return mountPluginStyles({
    document,
    scope,
    load: () => run.run(stylesCmd, { peer: home }),
    onChange(fn) {
      const offLive = client.peers.onLive((peer) => peer === home && fn());
      const offNotice = client.peers.onNotice((peer, n) => peer === home && n.kind === CHANGED && fn());
      return () => {
        offLive();
        offNotice();
      };
    },
  });
}
