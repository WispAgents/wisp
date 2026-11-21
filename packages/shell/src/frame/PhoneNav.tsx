// The phone apps' destinations (ADR-0065 step 8): the kit's NavigationBar (an M3 navigation bar or rail on Android,
// the tab bar on iOS) under the Stage. Selecting one navigates at once (navigate() never waits on data); the
// selected one follows from what's open (destinations.ts). Only the phone apps draw it (App passes it when
// `platform.phone`); the kit draws nothing for it in a browser either way.
import { createMemo, type JSX } from 'solid-js';
import { NavigationBar, useSignal, type IconName, type NavigationItem } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { destinationOf, destinationsOf, intentOf } from './destinations.ts';

export function PhoneNav(): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const offered = createMemo(() => destinationsOf((v) => shell.view(v) !== undefined));
  // Tasks waits for a computer: the phone app says when one you opened has connected (shell.tasksReady)
  const ready = shell.tasksReady ? useSignal(shell.tasksReady) : () => true;
  const off = (id: string): boolean => id === 'tasks' && !ready();
  const items = createMemo((): readonly NavigationItem[] =>
    offered().map((d) => ({
      id: d.id,
      label: shell.t(d.label),
      icon: d.icon as IconName,
      ...(d.iconSelected && { iconSelected: d.iconSelected as IconName }),
      ...(off(d.id) && { disabled: true }),
    })),
  );
  const select = (id: string): void => {
    const d = offered().find((x) => x.id === id);
    if (!d || off(d.id)) return;
    shell.platform.haptic('select');
    shell.navigate(intentOf(d));
  };
  return <NavigationBar aria-label={shell.t('shell.dest.label')} items={items()} selected={destinationOf(nav().main)} onSelect={select} />;
}
export default PhoneNav;
