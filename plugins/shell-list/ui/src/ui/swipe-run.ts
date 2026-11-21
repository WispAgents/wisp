// What a swiped row runs (audit A #8): its side's action, as the row's menu runs it. Mark done, Save for later and
// Archive at once, each toasting its Undo; Delete task only after asking (the list's own confirm, as the desktop's
// Delete asks: it can't be undone).
import type { EntityKey } from '@wisp/client';
import { toggleArchived, toggleDone, toggleForLater, type ActionCtx } from '../actions.ts';
import type { SwipeKind } from '../model/swipe.ts';

export interface SwipeRunEnv {
  readonly actions: ActionCtx;
  readonly askDelete: (key: EntityKey) => void;
}

/** the actions each side runs (tests hand their own) */
const RUN = {
  done: (c: ActionCtx, k: EntityKey) => toggleDone(c, k),
  later: (c: ActionCtx, k: EntityKey) => toggleForLater(c, k),
  archive: (c: ActionCtx, k: EntityKey) => toggleArchived(c, k),
};

export function runSwipe(kind: SwipeKind, env: SwipeRunEnv, key: EntityKey, run: { readonly [K in keyof typeof RUN]: (c: ActionCtx, k: EntityKey) => unknown } = RUN): void {
  if (kind === 'delete') return env.askDelete(key);
  run[kind](env.actions, key);
}
