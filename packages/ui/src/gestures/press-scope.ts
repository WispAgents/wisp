// The long-presses' timer (bind.ts, bind-vertical.ts), a scope helper: every press timer a binder starts is cleared
// when the binder stops (its component unmounts), so a gesture leaves nothing running (tools/check `state`).

export type Schedule = (fn: () => void, ms: number) => () => void;

/** A schedule whose pending timers all clear on `dispose`. */
export function pressTimers(): { readonly schedule: Schedule; readonly dispose: () => void } {
  const live = new Set<ReturnType<typeof setTimeout>>();
  return {
    schedule: (fn, ms) => {
      const id = setTimeout(() => {
        live.delete(id);
        fn();
      }, ms);
      live.add(id);
      return () => {
        clearTimeout(id);
        live.delete(id);
      };
    },
    dispose: () => {
      for (const id of live) clearTimeout(id);
      live.clear();
    },
  };
}
