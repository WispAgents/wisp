// The toaster: what the app (and @wisp/client's command rollback) calls to say something happened.
// Rules from design/ux.md: short, past tense, at most one action (Undo); the same text twice
// shows once; at most three stacked, only the newest in front; errors are red.
// Zag's toast store loads apart from the first page (./lazy-part.ts) on the first toast: until then a toast waits for it
// (its id is known at once: the same tone and text, the same id).
import { createSignal, type Accessor } from 'solid-js';
import type * as toast from '@zag-js/toast';
import { lazyPart } from './lazy-part.ts';

/** Zag's store and group machine (./toast-machine.tsx). */
export const toastMachine = lazyPart(() => import('./toast-machine.tsx'));

export type ToastTone = 'info' | 'success' | 'error' | 'loading';

export interface ToastAction {
  readonly label: string;
  readonly onClick: () => void;
}

export interface ToastOptions {
  /** A second line under the message. */
  readonly description?: string;
  /** ms on screen; errors stay longer by default. */
  readonly duration?: number;
  readonly action?: ToastAction;
}

/** The slice of the toaster a command runner needs to roll back with a red toast. */
export interface RollbackToaster {
  error(message: string, options?: { action?: ToastAction }): string;
}

export interface Toaster extends RollbackToaster {
  show(message: string, options?: ToastOptions & { readonly tone?: ToastTone }): string;
  success(message: string, options?: ToastOptions): string;
  error(message: string, options?: ToastOptions): string;
  dismiss(id?: string): void;
  /** Clear all. */
  clear(): void;
  /** For <Toasts>: Zag's store, once the first toast (or the app's preload) has loaded it. */
  readonly store: Accessor<toast.Store | undefined>;
  /** the store, loading it if it isn't yet */
  ready(): Promise<toast.Store>;
}

export interface ToasterOptions {
  /** Most toasts on screen at once (default 3); more wait their turn. */
  readonly max?: number;
  /** Default ms on screen (default 4000; errors 6000). */
  readonly duration?: number;
}

const ERROR_DURATION = 6000;

/** Same tone and text, same id: Zag's store updates the toast in place instead of adding a second. */
const idFor = (tone: ToastTone, message: string): string => `toast:${tone}:${message}`;

export function createToaster(options: ToasterOptions = {}): Toaster {
  const duration = options.duration ?? 4000;
  const [store, setStore] = createSignal<toast.Store>();
  const waiting: ((s: toast.Store) => void)[] = [];
  let loading: Promise<toast.Store> | undefined;
  const ready = (): Promise<toast.Store> =>
    (loading ??= toastMachine.load().then(
      (m) => {
        const s = m.createStore({ placement: 'bottom', max: options.max ?? 3, overlap: true, duration, gap: 8 });
        setStore(() => s);
        for (const f of waiting.splice(0)) f(s);
        return s;
      },
      (err: unknown) => {
        loading = undefined;
        throw err;
      },
    ));
  const withStore = (f: (s: toast.Store) => void): void => {
    const s = store();
    if (s) return f(s);
    waiting.push(f);
    ready().catch(() => undefined);
  };
  const show = (message: string, opts: ToastOptions & { readonly tone?: ToastTone } = {}): string => {
    const tone = opts.tone ?? 'info';
    const id = idFor(tone, message);
    withStore((s) =>
      s.create({
        id,
        title: message,
        type: tone,
        duration: opts.duration ?? (tone === 'error' ? ERROR_DURATION : duration),
        ...(opts.description === undefined ? {} : { description: opts.description }),
        ...(opts.action === undefined ? {} : { action: { label: opts.action.label, onClick: opts.action.onClick } }),
      }),
    );
    return id;
  };
  return {
    show,
    success: (message, opts) => show(message, { ...opts, tone: 'success' }),
    error: (message, opts) => show(message, { ...opts, tone: 'error' }),
    dismiss: (id) => withStore((s) => s.dismiss(id)),
    clear: () => withStore((s) => s.dismiss()),
    store,
    ready,
  };
}
