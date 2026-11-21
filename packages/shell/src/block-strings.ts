// The answer blocks' copy (`block.*`, the kit's BLOCK_STRINGS, spells may override it): registered by the screens that
// show blocks (the task page, Compare) as they load, and fetched by the shell just after its start for any other, so the
// first page's bundle doesn't carry it (budget, ADR-0011). Registering is once per client: a second caller finds it.
import type { Client } from '@wisp/client';
import { BLOCK_STRINGS } from '@wisp/ui';

export function blockStrings(client: Client): void {
  if (client.i18n.has(Object.keys(BLOCK_STRINGS)[0] ?? '')) return;
  client.i18n.register('block', BLOCK_STRINGS, client.scope);
}
