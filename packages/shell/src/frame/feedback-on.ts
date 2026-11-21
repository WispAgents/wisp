// Whether feedback can be sent to the wisp team (v1 feedback.js feedbackOn: config.feedback): wisp.telemetry's state
// row says it (`feedback`, false with WISP_FEEDBACK=0). A turn's 👍 👎 show only then, as in v1. The shell already
// sends ratings through that plugin's `telemetry.feedback`; this reads the row the same plugin publishes for it.
import { entityKey, type Client, type PeerId } from '@wisp/client';

export const TELEMETRY_TOPIC = 'plugin:wisp.telemetry:state';
export const TELEMETRY_ROW = 'telemetry.state';

/** On unless the peer's row says off (no row yet, or an older wisp without the field: on, as v1's default). */
export function feedbackOn(client: Pick<Client, 'entities'>, peer: PeerId): () => boolean {
  const row = client.entities.row<{ feedback?: boolean }>(TELEMETRY_ROW, entityKey(peer, 'this'));
  return () => (row() as { feedback?: boolean } | undefined)?.feedback !== false;
}
