// The overlay stack from navigation (plan §4.6.4): each overlay is a registered view drawn in a kit
// Dialog, a card on phones too, as v1's modals were; a viewer fills a phone, and a `sheet` comes up from the bottom. Closing one, by its X, a click outside or Esc, is the
// closeOverlay intent for its id, so the URL, Back and an Open on a toast all agree (#573). Keyed by
// id: reopening the same overlay never rebuilds it while it stays up.

import { For, Show, createMemo, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import type { ExtensionId, Overlay } from '@wisp/client';
import { Dialog, EmptyState, Sheet, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { useViewClient } from '../slot.tsx';

/** A view's dialog size on a phone: never `sm`, which the kit's phone looks draw as an alert (a confirm, its own Cancel
 * under centred lines, no ✕). An overlay view brings only its body, no Cancel, so it is the platform's dialog with its
 * way out: an iOS sheet with its ✕ and Close grabber, the M3 dialog with its ✕ (QA r14 #3: Share… had none). The size
 * only sets a card's width, which a phone doesn't use. */
const phoneSize = <S extends string>(size: S, phone: boolean): S | 'md' => (phone && size === 'sm' ? 'md' : size);

function OverlayView(props: { readonly overlay: Overlay }): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const def = shell.view(props.overlay.id);
  const client = useViewClient(props.overlay.id);
  const params = (): unknown => (def ? def.parse(props.overlay.params) : undefined);
  const close = (open: boolean): void => {
    if (!open) shell.navigate({ t: 'closeOverlay', id: props.overlay.id as ExtensionId });
  };
  const title = (): string => (def?.title ? def.title(params()) : shell.t('shell.palette.title'));
  const body = (): JSX.Element => (def ? <Dynamic component={def.component} client={client} params={params()} /> : <EmptyState icon="info" title={shell.t('shell.view.missing')} compact />);
  // what the view marks (`data-autofocus`: Settings' pane title, so opening a pane never lands on Back or in a field
  // that pops a phone's keyboard), else a text field inside (the palette's search), else the kit's default
  // Looked for in the newest overlay only (each mounts after those under it), so ⌘K over Settings focuses its own search.
  const initialFocus = (): HTMLElement | null => {
    const mine = [...document.querySelectorAll<HTMLElement>('[data-part~="shell.overlay"]')].at(-1);
    return mine?.querySelector<HTMLElement>('[data-autofocus]') ?? mine?.querySelector<HTMLElement>('input, textarea') ?? null;
  };
  // v1's modals stay cards on a phone (a 20px inset, scrolled inside); a viewer (Settings) takes the phone's whole
  // screen as before, and what's opened as a sheet comes up from the bottom; never a view placed at the top (⌘K)
  // the phone apps (ADR-0065 step 14): a viewer that brings its own head (Settings) is a pushed page over the whole
  // screen, the platform's bars inside it
  const page = (): boolean => shell.platform.phone && props.overlay.kind === 'viewer' && def?.bare === true && def.place !== 'top';
  const sheet = (): boolean => !page() && def?.place !== 'top' && (props.overlay.kind === 'sheet' || (props.overlay.kind === 'viewer' && nav().layout === 'stack'));
  return (
    <Show
      when={sheet()}
      fallback={
        <Dialog
          open
          title={title()}
          bare={def?.bare === true}
          {...(def?.place && { place: def.place })}
          onOpenChange={close}
          initialFocus={initialFocus}
          size={page() ? 'full' : props.overlay.kind === 'viewer' ? 'xl' : phoneSize(def?.size ?? 'md', shell.platform.phone)}
          part={(def?.part ? `shell.overlay ${def.part}` : 'shell.overlay') as `${string}.${string}`}
        >
          {body()}
        </Dialog>
      }
    >
      <Sheet
        open
        title={title()}
        bare={def?.bare === true}
        onOpenChange={close}
        initialFocus={initialFocus}
        {...(props.overlay.kind === 'viewer' && { size: 'xl' as const })}
        part={(def?.part ? `shell.overlay ${def.part}` : 'shell.overlay') as `${string}.${string}`}
      >
        {body()}
      </Sheet>
    </Show>
  );
}

export function Overlays(): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const ids = createMemo(() => nav().overlays.map((o) => o.id), undefined, { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) });
  const byId = (id: string): Overlay | undefined => nav().overlays.find((o) => o.id === id);
  return <For each={ids()}>{(id) => <Show when={byId(id)}>{(o) => <OverlayView overlay={o()} />}</Show>}</For>;
}
