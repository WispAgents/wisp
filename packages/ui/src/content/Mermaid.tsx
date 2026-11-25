// Mermaid: a closed ```mermaid fence drawn as a diagram (ADR-0118). The library is a dependency of the kit, bundled,
// never fetched from a CDN, and loaded the first time a diagram mounts (one shared import, so a transcript of twenty
// diagrams loads it once); the first screen never carries it. It runs at securityLevel 'strict' (no click handlers,
// no HTML in labels, its output sanitised), takes the page's scheme and tokens as its theme, and draws again when the
// theme changes. SVGs are cached by source and scheme, so scrolling back or toggling the theme twice costs nothing.
// Until the chunk lands a skeleton of fixed height holds the place; a source that won't parse shows as code with a
// short note. The bar copies the source, switches between the diagram and its code, and opens it full size in the
// media viewer (pan and zoom).
import { createEffect, createSignal, lazy, onCleanup, Show, Suspense, type JSX } from 'solid-js';
import { sub, type PartName } from '../core/props.ts';
import { IconButton } from '../controls/Button.tsx';
import { CopyButton } from '../blocks/content-copy.tsx';
import { useBlockT } from '../blocks/context.tsx';
import { createTimer } from '../blocks/content-scope.ts';
import { readTheme, watchTheme, type FrameTheme } from '../frame/theme.ts';
import { Code } from './Code.tsx';
import { Skeleton } from './Skeleton.tsx';

const MediaViewer = lazy(() => import('../blocks/Viewer.tsx').then((m) => ({ default: m.MediaViewer })));

/** The slice of mermaid's API used here (its own types pull in d3's). */
export interface MermaidApi {
  readonly initialize: (config: Record<string, unknown>) => void;
  readonly render: (id: string, source: string) => Promise<{ readonly svg: string }>;
}

/** The library's one shared state on the page (not app state: a cache of what it loaded and drew). mermaid keeps one
 *  global config, so renders go one at a time (`queue`), each after its own initialize when the config changed. */
const lib: { loading: Promise<MermaidApi> | undefined; seq: number; configured: string; queue: Promise<unknown> } = {
  loading: undefined,
  seq: 0,
  configured: '',
  queue: Promise.resolve(),
};

/** The one import of mermaid, made on the first diagram's mount. */
export function loadMermaid(): Promise<MermaidApi> {
  lib.loading ??= import('mermaid').then((m) => m.default as unknown as MermaidApi);
  return lib.loading;
}

/** How long a source must hold still before it's drawn again (an edited or re-streamed fence). */
export const MERMAID_DEBOUNCE_MS = 150;
const CACHE_MAX = 64;
const cache = new Map<string, Promise<string>>();

const COLOR = /^(#[0-9a-f]{3,8}|rgba?\([^()]*\)|hsla?\([^()]*\))$/i;

/** mermaid's config for a theme: the scheme picks its theme, the page's tokens its colours and font. */
export function mermaidConfig(theme: FrameTheme): Record<string, unknown> {
  const v = theme.vars;
  const color = (name: string): string | undefined => (v[name] && COLOR.test(v[name]) ? v[name] : undefined);
  const vars: Record<string, string> = {};
  const put = (key: string, value: string | undefined): void => {
    if (value) vars[key] = value;
  };
  put('background', color('--surface'));
  put('primaryColor', color('--surface-2'));
  put('primaryTextColor', color('--ink'));
  put('primaryBorderColor', color('--line-2'));
  put('secondaryColor', color('--bg-2'));
  put('tertiaryColor', color('--surface'));
  put('lineColor', color('--muted'));
  put('textColor', color('--ink-2'));
  put('noteBkgColor', color('--surface-2'));
  put('noteTextColor', color('--ink-2'));
  put('fontFamily', v['--sans']);
  return {
    startOnLoad: false,
    securityLevel: 'strict',
    theme: theme.scheme === 'dark' ? 'dark' : 'default',
    darkMode: theme.scheme === 'dark',
    // plain SVG text, no HTML labels: the full-size view shows it as a picture, where HTML in SVG doesn't draw
    htmlLabels: false,
    // a bad source fails the promise (code and a note show); mermaid draws no error picture into the page
    suppressErrorRendering: true,
    flowchart: { htmlLabels: false },
    themeVariables: vars,
  };
}

/** The diagram's SVG for `source` in `theme`, from the cache or drawn. */
export function renderMermaid(source: string, theme: FrameTheme): Promise<string> {
  const config = mermaidConfig(theme);
  const key = `${theme.scheme}\u0000${JSON.stringify(config.themeVariables)}\u0000${source}`;
  // the promise is cached, so two copies of one diagram on screen draw it once; a failure isn't kept
  const hit = cache.get(key);
  if (hit !== undefined) return hit;
  const run = lib.queue.then(async () => {
    const api = await loadMermaid();
    const conf = JSON.stringify(config);
    if (conf !== lib.configured) {
      api.initialize(config);
      lib.configured = conf;
    }
    return (await api.render(`wisp-mermaid-${++lib.seq}`, source)).svg;
  });
  lib.queue = run.catch(() => undefined);
  if (cache.size >= CACHE_MAX) cache.delete(cache.keys().next().value ?? '');
  cache.set(key, run);
  run.catch(() => cache.get(key) === run && cache.delete(key));
  return run;
}

/** Forgets the cache and the import (tests). */
export function resetMermaid(): void {
  cache.clear();
  lib.loading = undefined;
  lib.configured = '';
  lib.queue = Promise.resolve();
}

const firstLine = (err: unknown): string => (err instanceof Error ? err.message : String(err)).split('\n').find((l) => l.trim() !== '')?.trim() ?? '';

type State = { readonly k: 'loading' } | { readonly k: 'ok'; readonly svg: string } | { readonly k: 'error'; readonly why: string };

// size-ok: one figure (bar, diagram or code, note, full-size view) whose parts share the same few signals
export function Mermaid(props: { readonly source: string; readonly part: PartName }): JSX.Element {
  const t = useBlockT();
  const [theme, setTheme] = createSignal(readTheme());
  onCleanup(watchTheme(setTheme));
  // the first source draws at once; later ones once they have held still a moment
  const [source, setSource] = createSignal(props.source);
  const debounce = createTimer();
  createEffect((first: boolean) => {
    const next = props.source;
    if (first) return false;
    debounce.after(MERMAID_DEBOUNCE_MS, () => setSource(next));
    return false;
  }, true);

  const [state, setState] = createSignal<State>({ k: 'loading' });
  let gen = 0;
  createEffect(() => {
    const src = source();
    const th = theme();
    const mine = ++gen;
    // a redraw keeps the diagram shown until the new one is ready: no flash back to the skeleton
    void renderMermaid(src, th).then(
      (svg) => mine === gen && setState({ k: 'ok', svg }),
      (err: unknown) => mine === gen && setState({ k: 'error', why: firstLine(err) }),
    );
  });
  onCleanup(() => void ++gen);

  const [code, setCode] = createSignal(false);
  const [full, setFull] = createSignal(false);
  const svg = (): string | undefined => {
    const s = state();
    return s.k === 'ok' ? s.svg : undefined;
  };
  const failed = (): string | undefined => {
    const s = state();
    return s.k === 'error' ? s.why : undefined;
  };
  const picture = (): string => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg() ?? '')}`;
  return (
    <figure data-kit="mermaid" data-part={props.part} data-state={state().k}>
      <div data-kit="mermaid-bar" data-part={sub(props.part, 'bar')}>
        <span data-kit="mermaid-label">{t('block.flow.label')}</span>
        <Show when={failed() === undefined}>
          <IconButton
            part={sub(props.part, 'toggle')}
            size="xs"
            icon={code() ? 'image' : 'text'}
            label={code() ? t('block.mermaid.viewDiagram') : t('block.mermaid.viewCode')}
            pressed={code()}
            onClick={() => setCode(!code())}
          />
          <IconButton part={sub(props.part, 'full')} size="xs" icon="expand" label={t('block.mermaid.full')} disabled={svg() === undefined} onClick={() => setFull(true)} />
        </Show>
        <CopyButton part={sub(props.part, 'copy')} text={() => props.source} />
      </div>
      <Show
        when={failed() === undefined && !code()}
        fallback={
          <>
            <Code block lang="mermaid" part={sub(props.part, 'code')}>
              {props.source}
            </Code>
            <Show when={failed()}>{(why) => <figcaption data-kit="mermaid-note">{t('block.mermaid.failed', { why: why() })}</figcaption>}</Show>
          </>
        }
      >
        <Show when={svg()} fallback={<Skeleton part={sub(props.part, 'loading')} shape="block" height="map" />}>
          {/* mermaid's own output at securityLevel 'strict', sanitised by it (DOMPurify): the one SVG the kit takes as
              markup (ADR-0118); the source itself never becomes HTML. */}
          {(markup) => <div data-kit="mermaid-svg" role="img" aria-label={t('block.flow.label')} innerHTML={markup()} />}
        </Show>
      </Show>
      <Show when={full() && svg() !== undefined}>
        <Suspense>
          <MediaViewer list={[{ src: picture(), alt: t('block.flow.label') }]} at={0} onClose={() => setFull(false)} />
        </Suspense>
      </Show>
    </figure>
  );
}
