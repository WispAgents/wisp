// Links drawn by whoever knows them (ADR-0145): a screen wraps text in <LinkRenderer> and every link inside (a
// Markdown answer's, a prompt's bare URL) asks it first. It returns its own element (wisp.ship's live PR chip), or
// the kit's own one (`fallback`), so with no renderer, or one that doesn't know the link, nothing changes.
import { createContext, useContext, type JSX } from 'solid-js';

/** Draws one link: its own element, or `fallback()` (the anchor, or a prompt's plain text). */
export type RenderLink = (href: string, fallback: () => JSX.Element) => JSX.Element;

const Ctx = createContext<RenderLink | undefined>(undefined);

export function LinkRenderer(props: { readonly render: RenderLink; readonly children: JSX.Element }): JSX.Element {
  return <Ctx.Provider value={props.render}>{props.children}</Ctx.Provider>;
}

/** The renderer around this component, or undefined (draw links as the kit does). */
export const useLinkRenderer = (): RenderLink | undefined => useContext(Ctx);

const URL_RE = /https?:\/\/[^\s<>]+/gi;

/** A bare URL's end: trailing sentence punctuation isn't part of it; a `)` stays only if it closes a `(`. */
function trimUrl(url: string): string {
  for (;;) {
    const last = url[url.length - 1] ?? '';
    if (/[.,:;!?*_~'"]/.test(last)) url = url.slice(0, -1);
    else if (last === ')' && (url.match(/\(/g)?.length ?? 0) < (url.match(/\)/g)?.length ?? 0)) url = url.slice(0, -1);
    else return url;
  }
}

/** Plain text in runs: text, and the bare URLs in it. Pure. */
export function textRuns(s: string): readonly { readonly url?: true; readonly v: string }[] {
  const out: { url?: true; v: string }[] = [];
  let at = 0;
  for (const m of s.matchAll(URL_RE)) {
    const url = trimUrl(m[0]);
    const i = m.index;
    if (i > at) out.push({ v: s.slice(at, i) });
    out.push({ url: true, v: url });
    at = i + url.length;
  }
  if (at < s.length) out.push({ v: s.slice(at) });
  return out;
}

/** Plain text (a prompt) whose bare URLs a renderer may draw; with none it is the text as it was, nothing else. */
export function LinkedText(props: { readonly text: string }): JSX.Element {
  const render = useLinkRenderer();
  if (!render) return <>{props.text}</>;
  return <>{() => textRuns(props.text).map((r) => (r.url ? render(r.v, () => r.v) : r.v))}</>;
}
