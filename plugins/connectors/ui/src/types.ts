// What Settings → Connectors reads and sends, until Rust generates it (plugins/connectors/api
// rows.rs / commands.rs, field for field, camelCase as v1's GET /api/connectors). One file, so
// switching to packages/protocol/src/generated is one edit.
//
// Rows, all on TOPIC from the home peer:
//   connectors.connector  id = connector id   ConnectorView (never a secret value: key names + set)
//   connectors.found      id = server name    FoundServer (in a harness's own config, not added yet)
//   connectors.reach      id = 'this'         { harnesses }: the harnesses that take connectors per turn
// The reach row is also how the pane knows the topic has arrived (no row yet: placeholders).

import type { PeerId } from '@wisp/client';
import type { Topic } from '@wisp/protocol';
import './i18n/lazy.ts';

export const TOPIC = 'plugin:wisp.connectors:list' as Topic;
export const KIND = {
  connector: 'connectors.connector',
  found: 'connectors.found',
  reach: 'connectors.reach',
} as const;
export const REACH_ID = 'this';

/** The harness rows (accounts plugin) the names come from; v1 read S.agents the same way. */
export const HARNESS_KIND = 'accounts.harness';
export interface Harness {
  id: string;
  name: string;
}

export type Transport = 'local' | 'remote';

export interface ProbeResult {
  at: number;
  ok: boolean;
  tools?: string[];
  count?: number;
  error?: string;
  /** refused for want of a sign-in: the row offers Sign in */
  auth?: boolean;
  /** its sign-in couldn't be refreshed just now (no network, the server's 5xx): tried again, no sign-in asked for */
  retrying?: boolean;
}

export interface KeyState {
  key: string;
  set: boolean;
}

export interface SignedIn {
  host: string;
  at: number | null;
}

export interface ConnectorView {
  id: string;
  name: string;
  transport: Transport;
  command: string;
  args: string[];
  url: string;
  env: KeyState[];
  headers: KeyState[];
  enabled: boolean;
  test: ProbeResult | null;
  signed_in: SignedIn | null;
  added_at: number;
  /** harnesses that keep their own server by this name (it is skipped for them) */
  own: string[];
  /** harnesses that can't speak its transport (an SSE server, on Codex): it is skipped for them */
  cant_reach?: string[] | null;
  /** a test is running: the row says Checking… */
  testing: boolean;
  /** only on an optimistic row: the command line as typed */
  line?: string;
}

export interface FoundServer {
  name: string;
  from: string[];
  transport: Transport;
  command?: string;
  args?: string[];
  url?: string;
  env?: string[];
  headers?: string[];
  /** "Not now" hid it: the card leaves it out, the review lists it under Show hidden */
  hidden?: boolean | null;
}

export interface Reach {
  harnesses: string[];
}

// ---------- command args (connectors-api commands.rs) ----------

/** `{ key, value }`; an empty value keeps a saved secret (the server's rule). */
export interface SecretField {
  key: string;
  value: string;
}

export interface ConnectorBody {
  name?: string;
  /** 'sse': remote over the 2024 SSE transport (a pasted config's `"type": "sse"`); 'remote' keeps what an edit was */
  transport?: Transport | 'sse';
  command?: string;
  url?: string;
  env?: SecretField[];
  headers?: SecretField[];
  enabled?: boolean;
}

/** Every command names the peer it runs on (as `settings.set` does); the server ignores it. */
export interface OnPeer {
  peer: PeerId;
}
