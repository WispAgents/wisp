// `npm run dev`'s /api proxy (vite.config.ts devProxy): pure, so apps/web/test/dev-proxy.test.ts covers it.

/** The Origin the dev proxy hands the server: the server's own for Vite's page (the edge only takes its own,
 * crates/wisp-api/src/guard.rs), anything else as it came, so a pane or another local page is still refused. */
export function proxiedOrigin(origin: string | undefined, vitePort: number, target: string): string | undefined {
  if (origin === undefined) return undefined;
  return [`http://localhost:${vitePort}`, `http://127.0.0.1:${vitePort}`].includes(origin) ? target : origin;
}
