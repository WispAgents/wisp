// *Add to your phone* (ADR-0065 §2): wisp on the desktop shows a QR code with how to reach it over
// SSH and its host key's fingerprint, `wisp+ssh://you@host:port?fp=SHA256:…&name=Studio`. Scanning it
// saves the computer with that fingerprint pinned, so the first connect already refuses another key
// (before the password is sent). Typed by hand there is no pin: the first key is shown and kept.

import type { SshComputer } from './bridge.ts';

const FINGERPRINT = /^SHA256:[A-Za-z0-9+/]{43}$/;

/** The computer a scanned code describes (without its saved id), or undefined for anything else. */
export function fromPairingCode(text: string): Omit<SshComputer, 'id'> | undefined {
  let u: URL;
  try {
    u = new URL(text.trim());
  } catch {
    return undefined;
  }
  if (u.protocol !== 'wisp+ssh:') return undefined;
  const user = decodeURIComponent(u.username);
  const host = u.hostname.replace(/^\[(.*)\]$/, '$1');
  // read raw: URLSearchParams turns the fingerprint's base64 `+` into a space
  const raw = /[?&]fp=([^&#]*)/.exec(u.search)?.[1] ?? '';
  let fp = '';
  try {
    fp = decodeURIComponent(raw);
  } catch {
    return undefined;
  }
  if (!user || !host || !FINGERPRINT.test(fp)) return undefined; // a code without its pin is not trusted as one
  const port = u.port ? Number(u.port) : 22;
  if (!Number.isInteger(port) || port < 1 || port > 65535) return undefined;
  const at = host.includes(':') ? `[${host}]` : host;
  const name = (u.searchParams.get('name') ?? '').trim() || host;
  return { kind: 'ssh', name, target: port === 22 ? `${user}@${at}` : `${user}@${at}:${port}`, ...(port !== 22 && { sshPort: port }), fingerprint: fp };
}
