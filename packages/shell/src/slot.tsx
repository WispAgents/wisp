// Rendering resolved extensions: one at a time (ExtView, which applies `if` against the render's
// props), a whole input as a kit ExtensionSlot (Slot), or the one extension that fills an area
// (Area: the last one registered, so an override or a later plugin replaces the screen).

import { Show, createContext, createMemo, createSignal, useContext, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import type { PluginClient, Readable } from '@wisp/client';
import { ExtensionSlot, useSignal } from '@wisp/ui';
import type { ExtensionProps, PaletteSourceDef, ResolvedExtension, ShortcutDef } from './contract.ts';
import { useShell } from './context.tsx';

/** Which PluginClient an extension of `plugin` gets: provided by the shell's render. */
/** What the shell's own screens need from the host beyond ShellContext (not part of the contract). */
export interface Clients {
  client(plugin: string): PluginClient;
  /** the plugin that registered a view kind */
  viewOwner(kind: string): string | undefined;
  readonly palette: Readable<readonly (PaletteSourceDef & { plugin: string })[]>;
  readonly shortcuts: Readable<readonly (ShortcutDef & { plugin: string })[]>;
}
const ClientsCtx = createContext<Clients>();
export const ClientsProvider = ClientsCtx.Provider;
export function useClients(): Clients {
  const c = useContext(ClientsCtx);
  if (!c) throw new Error('usePluginClient outside the shell');
  return c;
}
export function usePluginClient(plugin: string): PluginClient {
  return useClients().client(plugin);
}
export function useViewClient(kind: string): PluginClient {
  const c = useClients();
  return c.client(c.viewOwner(kind) ?? 'shell');
}

export interface Scoped {
  readonly session?: ExtensionProps['session'];
  readonly id?: string;
  readonly form?: 'full' | 'icon';
  readonly pickAgent?: (agent: string) => void;
  /** added 2026-11-18: on Home's inputs, the harness in the prompt box now */
  readonly agent?: () => string;
}

export function extensionProps(ext: ResolvedExtension, client: PluginClient, s: Scoped): ExtensionProps {
  return {
    client,
    config: ext.config,
    ...(s.session && { session: s.session }),
    ...(s.id !== undefined && { id: s.id }),
    ...(s.form && { form: s.form }),
    ...(s.pickAgent && { pickAgent: s.pickAgent }),
    ...(s.agent && { agent: s.agent }),
  };
}

export function ExtView(props: { readonly ext: ResolvedExtension } & Scoped): JSX.Element {
  const client = usePluginClient(props.ext.plugin);
  const p = createMemo(() => extensionProps(props.ext, client, props));
  const shown = (): boolean => {
    const test = props.ext.def.if;
    if (!test) return true;
    try {
      return test(p());
    } catch {
      return false;
    }
  };
  return (
    <Show when={shown()}>
      <Dynamic component={props.ext.def.component} {...p()} />
    </Show>
  );
}

export function useExtensions(input: string): () => readonly ResolvedExtension[] {
  return useSignal(useShell().extensions(input));
}

/** Whether one of the input's extensions shows for these props (its `if` holds, or it has none): an input that takes
 *  another's place while it has something (`shell.task:screen`). */
export function useAnyShown(input: string, s: Scoped): () => boolean {
  const list = useExtensions(input);
  const clients = useClients();
  return () =>
    list().some((ext) => {
      const test = ext.def.if;
      if (!test) return true;
      try {
        return test(extensionProps(ext, clients.client(ext.plugin), s));
      } catch {
        return false;
      }
    });
}

/** The same extension, drawn the same way: its code, its plugin and its config. Its `order` is left out: a plugin's later
 *  part (registry.extend) renumbers every later plugin's default order, and a new object for an unchanged extension
 *  remounted what it drew (the task list rebuilt twice after load, and WebKit's "ResizeObserver loop" on every page:
 *  audit B #4). */
export function sameExtension(a: ResolvedExtension | undefined, b: ResolvedExtension | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.id === b.id && a.def === b.def && a.plugin === b.plugin && JSON.stringify(a.config) === JSON.stringify(b.config);
}

/** Every extension on one input, in order, each isolated by an error boundary. */
export function Slot(props: { readonly input: `${string}.${string}`; readonly layout?: 'inline' | 'wrap' | 'stack' | 'contents' } & Scoped): JSX.Element {
  const list = useExtensions(props.input);
  const shell = useShell();
  // one output per extension for as long as it's the same one (sameExtension), so the slot's keyed rows keep their DOM
  let drawn = new Map<string, { readonly ext: ResolvedExtension; readonly out: { readonly id: string; readonly render: () => JSX.Element } }>();
  const outputs = createMemo(() => {
    const next = new Map<string, { readonly ext: ResolvedExtension; readonly out: { readonly id: string; readonly render: () => JSX.Element } }>();
    for (const ext of list()) {
      const was = drawn.get(ext.id);
      next.set(ext.id, was && sameExtension(was.ext, ext) ? was : { ext, out: { id: ext.id, render: () => <ExtView ext={ext} {...props} /> } });
    }
    drawn = next;
    return [...next.values()].map((d) => d.out);
  });
  return (
    <ExtensionSlot
      input={props.input}
      extensions={outputs()}
      layout={props.layout ?? 'inline'}
      onError={(id, err) => shell.client.devtools.record({ kind: 'warn', message: `${id} failed: ${String(err)}` })}
    />
  );
}

/** The one extension that fills an area: the last registered (an override, or a later plugin). */
/** `loading`: drawn while the extension's own code loads (a lazy component), so the area is never blank (ADR-0017,
 *  QA round 1 #12: the task list was empty for ~0.2–1 s after Home already showed its rows). */
export function Area(props: { readonly input: string; readonly fallback: JSX.Element; readonly loading?: JSX.Element } & Scoped): JSX.Element {
  const list = useExtensions(props.input);
  // the same extension stays mounted when the registry hands out a new object for it (sameExtension)
  const last = createMemo(() => list().at(-1), undefined, { equals: sameExtension });
  return (
    <Show when={last()} keyed fallback={props.fallback}>
      {(ext) => {
        // a lazy component (Solid's lazy(): its `preload`) draws `loading` until its code is there, then itself at
        // once. Not a Suspense: one round the whole area would also fall back each time anything lazy inside it loads
        const pre = (ext.def.component as { preload?: () => Promise<unknown> }).preload;
        const [ready, setReady] = createSignal(!pre || props.loading === undefined);
        if (!ready()) void pre?.().then(() => setReady(true), () => setReady(true));
        return (
          <Show when={ready()} fallback={props.loading}>
            <ExtView ext={ext} {...props} />
          </Show>
        );
      }}
    </Show>
  );
}
