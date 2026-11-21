// Adding a computer on the phone (ADR-0065 step 13, §2; design/mobile.md §A.2 Computers): *Scan from your computer*
// reads the desktop's *Add to your phone* QR through the native code scanner (platform.scanCode), which pins the
// host key's fingerprint; with no scanner (or the camera refused, or a code that isn't wisp's) the address is typed
// by hand, and the first key the computer shows is kept. A computer whose key changed since says so on its row, with
// *Trust the new key* behind a confirmation (v1 needed a hidden Forget). *Stay connected* (the Android foreground
// service) is ON by default while an SSH computer without a push channel is saved (it's the only way its alerts
// reach the phone), until the person turns it off. Pure.

import type { PeerStatus } from '@wisp/client';
import type { Scanned } from '@wisp/shell/platform';
import { hostKeyRefused, type Computer, type SshComputer } from './bridge.ts';
import { fromPairingCode } from './pair.ts';

export type NewSsh = Omit<SshComputer, 'id'>;

/** What a scan leads to: save the computer it names, type it in instead (and why), or nothing (cancelled). */
export type ScanOutcome = { readonly kind: 'save'; readonly computer: NewSsh } | { readonly kind: 'manual'; readonly why: 'unsupported' | 'denied' | 'notWisp' } | { readonly kind: 'none' };

export function scanOutcome(s: Scanned): ScanOutcome {
  if (s === 'cancelled') return { kind: 'none' };
  if (s === 'unsupported' || s === 'denied') return { kind: 'manual', why: s };
  const c = fromPairingCode(s.text);
  return c ? { kind: 'save', computer: c } : { kind: 'manual', why: 'notWisp' };
}

/** Why a typed address can't be saved: an i18n key's last part (`phone.add.error.<why>`). */
export type AddressError = 'empty' | 'noUser' | 'port';

const TARGET = /^([^@\s]+)@(\[[0-9a-fA-F:.]+\]|[^@\s:[\]]+)(?::(\d+))?$/;

/** A computer typed by hand: `you@host` or `you@host:2222` (IPv6 in brackets); its name, else the host. */
export function manualComputer(name: string, address: string): { readonly computer: NewSsh } | { readonly error: AddressError } {
  const a = address.trim();
  if (!a) return { error: 'empty' };
  const m = TARGET.exec(a);
  if (!m) return { error: 'noUser' };
  const [, user = '', host = '', portText] = m;
  const port = portText === undefined ? 22 : Number(portText);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return { error: 'port' };
  const bare = host.replace(/^\[(.*)\]$/, '$1');
  return {
    computer: { kind: 'ssh', name: name.trim() || bare, target: port === 22 ? `${user}@${host}` : `${user}@${host}:${port}`, ...(port !== 22 && { sshPort: port }) },
  };
}

/** The same computer is already saved (the same target): adding it again only opens it. */
export function alreadySaved(list: readonly Computer[], c: NewSsh): Computer | undefined {
  return list.find((x) => x.kind === 'ssh' && x.target === c.target);
}

/** A computer's refused host key, from its peer's status (the connect's rejection is the reason). */
export function keyChanged(status: PeerStatus | undefined): { readonly fingerprint: string; readonly pinned: boolean } | undefined {
  return status?.kind === 'unreachable' ? hostKeyRefused(status.reason) : undefined;
}

/** The person's own choice, kept on the phone ('1' / '0'); none yet: the default below. */
export const STAY_CONNECTED = 'wisp.phone.stayConnected';

/** Whether the phone holds its SSH computers in the background: the person's choice, else ON while an SSH computer
 *  without a push channel is saved (ADR-0065 §3.6), OFF with none (nothing to hold). */
export function stayConnected(list: readonly Computer[], chosen: string | null | undefined): boolean {
  if (chosen === '1') return true;
  if (chosen === '0') return false;
  return needsStay(list);
}

/** An SSH computer without push is saved: the switch matters (it's shown) and defaults ON. */
export function needsStay(list: readonly Computer[]): boolean {
  return list.some((c) => c.kind === 'ssh' && !c.push);
}
