// The Android app's native side, as the bundle sees it: the WispSsh Capacitor plugin
// (mobile/android/.../WispSshPlugin.java). Computers are kept on the device (Computers.java), each
// reached by id: an SSH one through its own sshj tunnel (Reach.java), a grid one straight over
// HTTPS with its device token. Tokens (device tokens, computer tokens) and keys live in the Android
// Keystore (ADR-0065 §2): a saved Computer never carries one; save() hands a new grid computer's token
// down once, and only connect's Reached brings a token back, per connect, never to web storage.
// Typed structurally against the global Capacitor injects into the
// WebView, so this package has no @capacitor/core dependency.

/** A computer saved on this phone. `peer` is its wisp's peer id once learnt from a welcome. */
export type Computer = SshComputer | GridComputer;

interface Saved {
  readonly id: string;
  readonly name: string;
  readonly peer?: string;
  /** a push channel is paired for it: the foreground service isn't needed for its alerts (ADR-0065 §3.6) */
  readonly push?: boolean;
}
export interface SshComputer extends Saved {
  readonly kind: 'ssh';
  /** `you@host` or `you@host:2222` */
  readonly target: string;
  readonly sshPort?: number;
  /** wisp's port there; the phone's end may differ (Reach picks a free one) */
  readonly port?: number;
  readonly keyId?: string;
  /** a custom start command; '' runs the wisp binary the bootstrap installs */
  readonly command?: string;
  /** the host key pinned from the desktop's QR code (`SHA256:…`): no other key is accepted, even the first time */
  readonly fingerprint?: string;
}
export interface GridComputer extends Saved {
  readonly kind: 'grid';
  /** its wisp's own address: https://<handle>.<grid>, or <name>--<handle>.<grid> for your computer */
  readonly url: string;
  readonly grid?: string;
  /** set for one of your computers on the grid (it can't be woken from here) */
  readonly computer?: string;
  /** the wisp Cloud account it came with (`<grid>#<handle>`, cloud.ts): its card on the Computers screen */
  readonly account?: string;
}

/**
 * What save() adds: a grid computer comes with the device token from the grid's sign-in (RFC 8628),
 * which goes straight into the Keystore. It is never read back (list() has none).
 */
export type NewComputer = SshComputer | (GridComputer & { readonly token: string });

/** Fields merged into a saved computer. `token` (a renewed device token) is write-only, as above. */
export interface ComputerPatch {
  readonly id: string;
  readonly peer?: string;
  readonly name?: string;
  readonly token?: string;
  readonly push?: boolean;
}

/** How to reach a computer now. A tunnel may come back on another port: ask on every connect. */
export type Reached =
  | { readonly kind: 'ssh'; readonly port: number; readonly token: string; readonly name: string; readonly peer?: string }
  | { readonly kind: 'grid'; readonly base: string; readonly token: string; readonly name: string; readonly peer?: string };

export type TunnelState = 'connecting' | 'ready' | 'failed' | 'idle';
export interface TunnelEvent {
  readonly id: string;
  readonly state: TunnelState;
  /** the `@@wisp …` line that moved it ('' when stopped) */
  readonly line: string;
}

export interface NativeBridge {
  /** the saved computers, never with a token */
  list(): Promise<readonly Computer[]>;
  /** adds one, or merges the fields given into the saved one (a learnt peer id, a renewed token) */
  save(computer: NewComputer | ComputerPatch): Promise<void>;
  forget(id: string): Promise<void>;
  /** opens (or reuses) the way to it; rejects with why it can't (SSH refused, the script failed) */
  connect(id: string, opts?: { readonly password?: string }): Promise<Reached>;
  disconnect(id: string): Promise<void>;
  /**
   * *Trust the new key*, after the page's confirmation: keeps the fingerprint the last refused connect
   * showed (`hostKeyRefused`), and replaces a QR pin. Rejects for any other fingerprint.
   */
  trustHostKey(id: string, fingerprint: string): Promise<void>;
  /** Settings → Phone → *Stay connected* (off by default): the foreground service may hold SSH computers without push. Answers the switch. */
  setStayConnected(on: boolean): Promise<boolean>;
  /** sends the app behind (Back with nothing left to close); its tunnels stay */
  leave(): Promise<void>;
  onTunnel(fn: (e: TunnelEvent) => void): () => void;
  /** the saved list changed (added, forgotten, a peer id learnt) */
  onComputers(fn: () => void): () => void;
  /** Android's Back. While anything listens, the app no longer handles it itself */
  onBack(fn: () => void): () => void;
  /** a notification the bundle posted was tapped: its `extra` (a navigation Target) */
  onOpen(fn: (target: unknown) => void): () => void;
}

type Handle = { remove(): unknown };
/** The part of `window.Capacitor` (its native bridge) used here. */
export interface CapacitorLike {
  isNativePlatform?(): boolean;
  nativePromise?(plugin: string, method: string, args?: object): Promise<unknown>;
  addListener?(plugin: string, event: string, fn: (data: unknown) => void): Handle | Promise<Handle>;
  Plugins?: Record<string, Record<string, ((args?: unknown) => Promise<unknown>) | undefined> | undefined>;
}

export const PLUGIN = 'WispSsh';

// The two refusals HostTrust.java says, in the key exchange before anything is sent.
const CHANGED = 'This computer’s key changed: it is now ';
const NOT_PINNED = 'This computer’s key isn’t the one its QR code showed: it is now ';

/** A refused host key in a connect's rejection (or a tunnel line): the new fingerprint, for *Trust the new key*. */
export function hostKeyRefused(message: string): { readonly fingerprint: string; readonly pinned: boolean } | undefined {
  const m = message.replace(/^@@wisp fail ssh /, '');
  for (const [p, pinned] of [[CHANGED, false], [NOT_PINNED, true]] as const) {
    if (m.startsWith(p) && m.endsWith('.')) return { fingerprint: m.slice(p.length, -1), pinned };
  }
  return undefined;
}

const record = (v: unknown): Record<string, unknown> => (v && typeof v === 'object' ? (v as Record<string, unknown>) : {});
const str = (v: unknown): string => (typeof v === 'string' ? v : '');

function computerOf(v: unknown): Computer | undefined {
  const o = record(v);
  const id = str(o['id']);
  if (!id) return undefined;
  // a token is never taken from the list, even if a native side sent one
  const base = { id, name: str(o['name']) || id, ...(str(o['peer']) && { peer: str(o['peer']) }), ...(o['push'] === true && { push: true }) };
  if (o['kind'] === 'grid')
    return { ...base, kind: 'grid', url: str(o['url']), ...(str(o['grid']) && { grid: str(o['grid']) }), ...(str(o['computer']) && { computer: str(o['computer']) }), ...(str(o['account']) && { account: str(o['account']) }) };
  if (o['kind'] !== 'ssh') return undefined; // a kind this app doesn't know: left alone
  const num = (k: string): object => (typeof o[k] === 'number' ? { [k]: o[k] } : {});
  return { ...base, kind: 'ssh', target: str(o['target']), ...num('sshPort'), ...num('port'), ...(str(o['keyId']) && { keyId: str(o['keyId']) }), ...(str(o['command']) && { command: str(o['command']) }), ...(str(o['fingerprint']) && { fingerprint: str(o['fingerprint']) }) };
}

function reachedOf(v: unknown): Reached {
  const o = record(v);
  const common = { token: str(o['token']), name: str(o['name']), ...(str(o['peer']) && { peer: str(o['peer']) }) };
  if (o['kind'] === 'grid') return { kind: 'grid', base: str(o['base']).replace(/\/+$/, ''), ...common };
  const port = o['port'];
  if (typeof port !== 'number' || port <= 0) throw new Error('the tunnel gave no port');
  return { kind: 'ssh', port, ...common };
}

/** The bridge over Capacitor's injected global (window.Capacitor). */
export function capacitorBridge(cap: CapacitorLike | undefined): NativeBridge {
  const call = async (method: string, args: object = {}): Promise<unknown> => {
    if (!cap?.nativePromise) throw new Error(`${PLUGIN}.${method}: not in the Android app`);
    return cap.nativePromise(PLUGIN, method, args);
  };
  const on = (event: string, fn: (data: unknown) => void): (() => void) => {
    const h = cap?.addListener?.(PLUGIN, event, fn);
    return () => void Promise.resolve(h).then((x) => x?.remove());
  };
  return {
    list: async () => {
      const raw = record(await call('list'))['computers'];
      return (Array.isArray(raw) ? raw : []).map(computerOf).filter((c): c is Computer => c !== undefined);
    },
    save: async (computer) => void (await call('saveComputer', { computer })),
    forget: async (id) => void (await call('forgetComputer', { id })),
    connect: async (id, opts = {}) => reachedOf(await call('connect', { id, ...opts })),
    disconnect: async (id) => void (await call('disconnect', { id })),
    trustHostKey: async (id, fingerprint) => void (await call('trustHostKey', { id, fingerprint })),
    setStayConnected: async (on) => record(await call('setStayConnected', { on }))['on'] === true,
    leave: async () => void (await call('leave')),
    onTunnel: (fn) =>
      on('tunnel', (d) => {
        const o = record(d);
        fn({ id: str(o['id']), state: str(o['state']) as TunnelState, line: str(o['line']) });
      }),
    onComputers: (fn) => on('computers', () => fn()),
    onBack: (fn) => on('back', () => fn()),
    onOpen: (fn) =>
      on('open', (d) => {
        const t = str(record(d)['target']);
        try {
          fn(JSON.parse(t));
        } catch {
          // not one of ours: nothing to open
        }
      }),
  };
}

/**
 * Fills `Capacitor.Plugins` for the native plugins the shell's androidPlatform calls (WispSsh,
 * LocalNotifications): without @capacitor/core's registerPlugin that map stays empty.
 */
export function exposePlugins(cap: CapacitorLike | undefined, names: readonly string[] = [PLUGIN, 'LocalNotifications']): void {
  const native = cap?.nativePromise;
  if (!cap || !native) return;
  const plugins = (cap.Plugins ??= {});
  for (const name of names) {
    if (plugins[name]) continue;
    plugins[name] = new Proxy({} as Record<string, (args?: unknown) => Promise<unknown>>, {
      get: (_, method) => (typeof method === 'string' ? (args?: unknown) => native.call(cap, name, method, record(args)) : undefined),
    });
  }
}
