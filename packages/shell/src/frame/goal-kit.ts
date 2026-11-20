// Goal…'s code and the peer's declared goals, fetched once the first screen has drawn and again as the composer's `+` is
// warmed (the pointer on it, its focus, each time it opens), never with the first screen (ADR-0105: r192's named goals
// pushed New task's first screen over its ratchet). Fetched before `+` opens, so Goal… is already the menu it will be:
// a row that turned into a submenu in an open menu was a jump (stream-steady). Until they're here Goal… opens Done when,
// as it does with no goal declared (./goal-menu.ts askGoals).
import { createSignal, type Accessor } from 'solid-js';
import type { ShellContext } from '../contract.ts';
import type { DeclaredGoal } from './goal-gate.ts';

export type GoalKit = typeof import('./goal-menu.ts');

export function useGoalKit(
  shell: Pick<ShellContext, 'client' | 'afterFirstScreen'>,
  peer: Accessor<string>,
): { readonly kit: Accessor<GoalKit | undefined>; readonly declared: Accessor<readonly DeclaredGoal[]>; readonly warm: () => void; readonly reset: () => void } {
  const [kit, setKit] = createSignal<GoalKit>();
  const [declared, setDeclared] = createSignal<readonly DeclaredGoal[]>([]);
  const warm = (): void =>
    void import('./goal-menu.ts').then(
      (m) => (setKit(() => m), m.askGoals(shell.client, peer, setDeclared)),
      () => undefined,
    );
  shell.afterFirstScreen?.(warm);
  return { kit, declared, warm, reset: () => setDeclared([]) };
}
