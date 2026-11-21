// Bridges @wisp/client's signals (alien-signals) into Solid. The client store stays framework
// independent; a component reads it through `useSignal(selector)` and re-renders when it changes.
import { effect, setActiveSub } from 'alien-signals';
import { createComputed, createMemo, createSignal, getOwner, onCleanup, type Accessor } from 'solid-js';

/** Something with a current value that says when it changes. */
export interface Subscribable<T> {
  get(): T;
  subscribe(listener: (value: T) => void): () => void;
}

/** An alien-signals signal or computed (a getter that tracks), or a subscribable store. */
export type Readable<T> = (() => T) | Subscribable<T>;

const isSubscribable = <T>(r: Readable<T>): r is Subscribable<T> => typeof r !== 'function';

/** A writable alien-signals signal. Its own overload, because TS infers from an overloaded
 * function type's last signature (the setter) and would make T `void`. */
export type WritableSignal<T> = { (): T; (value: T): void };

/** A Solid accessor that follows `readable`. Stops following when the owning component unmounts. */
export function useSignal<T>(readable: WritableSignal<T>): Accessor<T>;
export function useSignal<T>(readable: Readable<T>): Accessor<T>;
export function useSignal<T>(readable: Readable<T>): Accessor<T> {
  const first = isSubscribable(readable) ? readable.get() : undefined;
  // `() => v`: a function value must not be taken for an updater.
  const [value, setValue] = createSignal<T>(first as T, { equals: Object.is });
  // alien's effect runs once at once, so the accessor holds the current value before first paint.
  // a component may be made while an alien effect runs (Solid renders at once from another useSignal's setValue):
  // made inside it, this effect would be that one's child and die on its next run, leaving the accessor stuck. The
  // Solid owner frees it instead, so it's made on its own.
  const outer = getOwner() ? setActiveSub(undefined) : undefined;
  let stop: () => void;
  try {
    stop = isSubscribable(readable)
      ? readable.subscribe((v) => setValue(() => v))
      : effect(() => {
          const v = readable();
          setValue(() => v);
        });
  } finally {
    if (getOwner()) setActiveSub(outer);
  }
  if (getOwner()) onCleanup(stop);
  // a getter may read Solid signals too (a setting source a form builds over its own draft): follow those as well,
  // or such a value is read once and the control never shows the change
  if (!isSubscribable(readable) && getOwner()) createComputed(() => setValue(() => readable()));
  return value;
}

/**
 * Added 2026-11-16 (round 323): a Solid memo over `fn` that also follows the alien signals it reads (the client's entity
 * store, settings, a `signal`), for a derivation that reads both kinds, such as the palette's sources: a plain
 * createMemo misses the store, so a task that lands, is renamed or is deleted while it's open never shows. One run per
 * change, with both kinds of reads tracked on it: an alien change marks the memo stale and its next run follows a fresh
 * set of reads, so a branch that stops or starts reading a signal is followed as it is now (useSignal's getter form runs
 * `fn` twice, once per kind).
 */
export function useDerived<T>(fn: () => T): Accessor<T> {
  const [stale, setStale] = createSignal(0);
  let watch: (() => void) | undefined;
  onCleanup(() => watch?.());
  return createMemo(() => {
    stale();
    watch?.();
    let out: { v: T } | { err: unknown } | undefined;
    // made on its own, not as a child of an alien effect this memo may run inside (see useSignal)
    const outer = setActiveSub(undefined);
    try {
      watch = effect(() => {
        if (out === undefined) {
          // a throw is kept and thrown by the memo: out of effect() it would leave this watcher made and never freed
          try {
            out = { v: fn() };
          } catch (err) {
            out = { err };
          }
          return;
        }
        // something it read in the store changed: run it again as a whole (this watcher is replaced, its reads dropped);
        // with no alien sub active, so what that pass's Solid code reads doesn't link to this watcher as it goes
        const was = setActiveSub(undefined);
        try {
          setStale((n) => n + 1);
        } finally {
          setActiveSub(was);
        }
      });
    } finally {
      setActiveSub(outer);
    }
    const got = out as { v: T } | { err: unknown };
    if ('err' in got) throw got.err;
    return got.v;
  });
}
