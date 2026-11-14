// The compose button (design/mobile.md §A.1, §A.7): on iOS a `Fab` is not drawn; its action becomes the
// navigation bar's trailing compose button, as in Mail and Notes. The Fab hands its action to the nearest
// `ComposeHost` (every `Frame` is one); a root `TopAppBar` under that host draws it, and with none the
// frame's own top bar does. Android and the browser never use it.
import { createContext, createSignal, onCleanup, Show, useContext, type Accessor, type JSX } from 'solid-js';
import { IconButton } from '../controls/Button.tsx';
import type { IconName } from '../content/icons.ts';
import type { PartName } from '../core/props.ts';

export interface ComposeAction {
  readonly label: string;
  readonly icon: IconName;
  readonly onSelect: () => void;
  readonly part: PartName;
}

interface Host {
  readonly action: Accessor<ComposeAction | undefined>;
  /** A Fab offers its action; the returned function takes it back. */
  offer(a: ComposeAction): () => void;
  /** How many root bars draw it now (the frame's top bar draws it only when none does). */
  readonly bars: Accessor<number>;
  claim(): () => void;
}

const HostContext = createContext<Host>();

export function useComposeHost(): Host | undefined {
  return useContext(HostContext);
}

export function ComposeHost(props: { readonly children: JSX.Element }): JSX.Element {
  const [action, setAction] = createSignal<ComposeAction | undefined>();
  const [bars, setBars] = createSignal(0);
  const host: Host = {
    action,
    offer(a) {
      setAction(() => a);
      return () => setAction((now) => (now === a ? undefined : now));
    },
    bars,
    claim() {
      setBars((n) => n + 1);
      let done = false;
      return () => {
        if (done) return;
        done = true;
        setBars((n) => n - 1);
      };
    },
  };
  return <HostContext.Provider value={host}>{props.children}</HostContext.Provider>;
}

/** The trailing compose button, when the host has an action and `when` holds. */
export function ComposeButton(props: { readonly when: boolean }): JSX.Element {
  const host = useComposeHost();
  return (
    <Show when={props.when ? host?.action() : undefined}>
      {(a) => <IconButton icon={a().icon} label={a().label} variant="ghost" onClick={() => a().onSelect()} part={a().part} />}
    </Show>
  );
}

/** Registers a root bar for as long as the calling component lives (when `on`). */
export function claimCompose(host: Host | undefined, on: boolean): void {
  if (!host || !on) return;
  onCleanup(host.claim());
}
