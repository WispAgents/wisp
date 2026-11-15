// The compose button (design/mobile.md §A.1, §A.7): on iOS a `Fab` is not drawn; its action becomes the
// navigation bar's trailing compose button, as in Mail and Notes. The Fab hands its action to the nearest
// `ComposeHost` (every `Frame` is one); a root `TopAppBar` under that host draws it, and with none the
// frame's own top bar does. Android and the browser never use it.
//
// Each of the frame's panes is a `ComposeScope` (r298): the Tasks list stays mounted, hidden, under My Stuff, so its
// Fab's action is still offered; a bar draws it only from the pane it came from (a root bar in its own pane, the
// frame's top bar while that pane is on screen), never on another destination's bar.
import { createContext, createSignal, onCleanup, Show, useContext, type Accessor, type JSX } from 'solid-js';
import { IconButton } from '../controls/Button.tsx';
import type { IconName } from '../content/icons.ts';
import type { PartName } from '../core/props.ts';

export interface ComposeAction {
  readonly label: string;
  readonly icon: IconName;
  readonly onSelect: () => void;
  readonly part: PartName;
  /** the pane it was offered from (the nearest ComposeScope), set by the Fab */
  readonly scope?: string | undefined;
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
const ScopeContext = createContext<string>();

/** The pane a Fab or a root bar sits in (the frame's list, main, side), or undefined outside a frame's panes. */
export function useComposeScope(): string | undefined {
  return useContext(ScopeContext);
}

export function ComposeScope(props: { readonly id: string; readonly children: JSX.Element }): JSX.Element {
  return <ScopeContext.Provider value={props.id}>{props.children}</ScopeContext.Provider>;
}

/** Whether a bar in `bar`'s pane draws an action offered from `from`: only its own pane's. */
export const composeFor = (from: string | undefined, bar: string | undefined): boolean => from === bar;

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

/** The trailing compose button, when the host has an action, `when` holds, and the action comes from a pane `accept`s
 *  (by default the button's own pane). */
export function ComposeButton(props: { readonly when: boolean; readonly accept?: (scope: string | undefined) => boolean }): JSX.Element {
  const host = useComposeHost();
  const own = useComposeScope();
  const shown = (): ComposeAction | undefined => {
    const a = props.when ? host?.action() : undefined;
    return a && (props.accept ? props.accept(a.scope) : composeFor(a.scope, own)) ? a : undefined;
  };
  return (
    <Show when={shown()}>
      {(a) => <IconButton icon={a().icon} label={a().label} variant="ghost" onClick={() => a().onSelect()} part={a().part} />}
    </Show>
  );
}

/** Registers a root bar for as long as the calling component lives (when `on`). */
export function claimCompose(host: Host | undefined, on: boolean): void {
  if (!host || !on) return;
  onCleanup(host.claim());
}
