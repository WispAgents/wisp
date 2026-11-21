// *Stay connected* on the phone (ADR-0065 §3.6, step 13): the native side holds SSH computers in the background (the
// Android foreground service) only while the switch is on. It follows add.ts `stayConnected`: the person's choice, else
// ON while an SSH computer without push is saved. Asked again whenever the saved list changes, and only when the answer
// does; a native side without it (iOS today) rejects, which changes nothing. Owned by `scope`.
import { signal, type Readable, type Scope } from '@wisp/client';
import { STAY_CONNECTED, stayConnected } from './add.ts';
import type { NativeBridge } from './bridge.ts';
import type { StorageLike } from './migrate.ts';

export interface StayConnected {
  /** the person flips the switch: kept on the phone, then the native side asked */
  set(on: boolean): void;
  /** what the switch shows */
  readonly on: Readable<boolean>;
  /** the native side has it (the Android app); false once it refused, so a phone without it shows no switch */
  readonly offered: Readable<boolean>;
}

export function followStayConnected(bridge: Pick<NativeBridge, 'list' | 'onComputers' | 'setStayConnected'>, storage: StorageLike | undefined, scope: Scope): StayConnected {
  let sent: boolean | undefined;
  // until the list is read: the person's choice, else ON (the switch shows only once an SSH computer without push is)
  const now = signal(storage?.getItem(STAY_CONNECTED) !== '0');
  const offered = signal(true);
  const apply = (want: boolean): void => {
    now(want);
    if (want === sent) return;
    sent = want;
    bridge.setStayConnected(want).catch(() => {
      sent = undefined;
      offered(false); // not on this phone (iOS today): no switch that does nothing
    });
  };
  const reload = (): void => {
    bridge.list().then(
      (list) => {
        if (scope.alive) apply(stayConnected(list, storage?.getItem(STAY_CONNECTED)));
      },
      () => {},
    );
  };
  reload();
  scope.add(bridge.onComputers(reload));
  return {
    set(on) {
      storage?.setItem(STAY_CONNECTED, on ? '1' : '0');
      apply(on);
    },
    on: () => now(),
    offered: () => offered(),
  };
}
