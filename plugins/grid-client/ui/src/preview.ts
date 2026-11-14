// Where a `preview` block's page shows when this wisp runs in a grid box (ADR-0064, v1 preview-spec.js boxUrl):
// "http://localhost:5173/a?b#c" with the box's template "https://p{port}--ada.grid.example" →
// "https://p5173--ada.grid.example/a?b#c". DOM-free. undefined (the block shows the URL as written) when it isn't a
// loopback page, the template isn't a grid's (`http(s)://p{port}--<host>`), the port is one the grid never shows
// (below 1024), or this window is itself on loopback (its localhost is the box's: a tunnel, the box's own browser).
import { entityKey, ownerOf, type PluginClient } from '@wisp/client';
import { KIND, TOPIC, type CloudRow } from './commands.ts';

/** The ports the grid shows a box's page from (apps/grid previews.rs `PORTS`). */
export const PREVIEW_PORTS: readonly [number, number] = [1024, 65_535];
const TEMPLATE = /^https?:\/\/p\{port\}--[a-z0-9][a-z0-9.:-]*$/i;
const LOOPBACK = /^(localhost|127\.\d+\.\d+\.\d+|\[::1\]|0\.0\.0\.0)$/i;

const hostOf = (u: string): string | undefined => {
  try {
    return new URL(u).hostname;
  } catch {
    return undefined;
  }
};

export function boxPreviewUrl(url: string, template: string | undefined, here: string): string | undefined {
  if (!template || !TEMPLATE.test(template)) return undefined;
  if (LOOPBACK.test(hostOf(here) ?? '')) return undefined;
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return undefined;
  }
  if ((u.protocol !== 'http:' && u.protocol !== 'https:') || !LOOPBACK.test(u.hostname)) return undefined;
  const port = Number(u.port || (u.protocol === 'https:' ? 443 : 80));
  if (port < PREVIEW_PORTS[0] || port > PREVIEW_PORTS[1]) return undefined;
  return `${template.replace('{port}', String(port))}${u.pathname}${u.search}${u.hash}`;
}

const watched = new WeakSet<PluginClient>();

/** The shell's `previewAddresses` entry: a task that ran on this wisp, read where this wisp's row says its pages show. */
export function previewAddress(ctx: { client: PluginClient; task: string }): (url: string) => string | undefined {
  const c = ctx.client;
  // another machine's task: its localhost isn't this box's (v1: a task id with @machine)
  if (ownerOf(ctx.task) !== c.home) return () => undefined;
  if (!watched.has(c)) {
    watched.add(c);
    c.watch(c.home, TOPIC, c.scope);
  }
  const row = c.entities.row<CloudRow>(KIND, entityKey(c.home, 'this'));
  // where this window is (as the kit's Preview reads it): its own address, not navigation state
  const here = (): string => (typeof document === 'undefined' ? '' : document.baseURI);
  return (url) => boxPreviewUrl(url, (row() as CloudRow | undefined)?.preview, here());
}
