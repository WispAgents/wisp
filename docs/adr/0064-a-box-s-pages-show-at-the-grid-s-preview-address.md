# ADR-0064: A box's pages show at the grid's preview address; grid-client publishes the template, the shell asks plugins

- Status: accepted
- Date: 2026-11-14
- Source: F-page-31 (docs/v1-features.md): "Given a `preview` block, when it shows, then the live page is in the answer
  (cloud: the cloud's page)." v1: grid/server.js `previewUrl`, `previewLink`, `previewAuth`, `previewEdge`,
  `previewSocket`; grid/hostd.js `WISP_PREVIEW_URL`; server/routes/settings.js `preview`; web/js/preview-spec.js
  `boxUrl`; web/js/preview-frame.js. With ADR-0003 (plugins talk through api packages), ADR-0014 (the grid's trust
  table), ADR-0049 (Preview's proxy, local), ADR-0058 (keys reach the box only).

## Context

An agent's answer may hold a `preview` block with its dev server's address (`http://localhost:5173`). On a laptop the
frame loads it. When wisp runs in a grid box, that localhost is the box's, not the browser's: v1 showed it at
`p<port>--<handle>.<domain>`, which the grid signs in and passes to the box's wisp (`x-wisp-preview: <port>`), which
passes it to the port on its loopback. v2 had the box half (`wisp-api` preview.rs, edge case 10) but the grid answered
every box preview with 404, never told a box its template, and nothing told the UI. The kit's block already takes a
host mapping (`BlockContext.previewUrl`), but the shell can't know about grids (ADR-0003).

## Decision

1. **The grid serves `p<port>--<handle>`** (`apps/grid/src/edge_preview.rs`, `previews.rs`), when `hostd.previews` is on
   (containers by default, as before; `HostLookups::previews` now reads it). Its own sign-in, never another's:
   - `GET <handle>.<domain>/_grid/preview?port=&path=` by the box's owner (whatever already reaches the box, except an
     API key: the trust table's "a key opens its user's own wisp, never a preview") answers `302` to
     `p<port>--<handle>/_grid/auth?t=<ticket>&r=<path>`. The ticket is one-time, 60 s, scoped `preview:<handle>`
     (the `tickets.scope` column v1 added).
   - `/_grid/auth` there spends it for a host-only, framed cookie `wisp_preview` (`HttpOnly; SameSite=None; Secure;
     Partitioned`, 7 days), a session scoped `p:<handle>`. `Accounts::session` now refuses `p:` sessions, and
     `Accounts::preview_session` accepts only its own label's.
   - Signed in, the request goes to the box's wisp with its proxy secret and `x-wisp-preview`, the page's own cookies
     kept and the preview's dropped; `x-wisp-*` and `x-forwarded-*` replaced; websockets piped. A `Set-Cookie` with
     `Domain=` is dropped (cookie tossing). Signed out, a browser page goes to the box's address for a link (and back);
     a fetch hears 401.
   - **Not an open proxy:** one box, its owner only; ports `1024..=65535` and never the grid's own listen port (a
     process box shares the grid's loopback); the box must be running (a preview never wakes it; 503 `asleep`); a
     non-GET with a foreign `Origin` is 403. Sandboxes' previews stay off (404).
2. **hostd tells each box its template** as `WISP_PREVIEW_URL=<scheme>://p{port}--<handle>.<domain>[:port]`
   (`hostd::preview_template`), only with previews on and never to a sandbox.
3. **wisp.grid-client publishes it**: `grid-client.status` and the `grid-client.cloud` row (`id: this`) gain `preview`
   (additive; migration 3, `''` default). It's set only inside a box (`WISP_BOX=1`, not `WISP_SANDBOX`) and only from a
   well-formed `http(s)://p{port}--<host>` (`host/src/preview.rs`); anything else is `""` (fail closed).
4. **The shell asks plugins, not grids** (`@wisp/shell` contract, additive): `PluginUiDef.previewAddresses:
   PreviewAddressDef[]`, each `map({client, task}) → (url) → string | undefined`, set up once per answer and read
   reactively. `answerContext` gives the block the first answer. wisp.grid-client's UI maps a task that ran on this wisp
   (`ownerOf(task) === client.home`) with its row's template (`boxPreviewUrl`, pure): loopback URLs only, ports the grid
   shows only, never when this window is itself on loopback. The kit's `PreviewBlock` now reads `previewUrl` live.

## Consequences

- In a box, a `preview` block shows the running page in the answer; Open in a new tab goes there too (signing in at the
  box's address first if needed). Elsewhere nothing changes: no plugin maps, the plain URL loads.
- Any plugin can map previews the same way (a tunnel, an SSH port forward): no private API for grid-client.
- A frame inside the desktop app (another site) gets no `SameSite=Lax` cookie at the box's address on the bounce, so a
  signed-out preview there stops at 401 until the page is opened once in a browser tab (v1's phone used a POST link;
  not ported here).
- Previews on with the process driver are only for one-person dev grids: a box's loopback is the grid machine's.

## Edge cases

| Case | What happens | Test |
|---|---|---|
| Someone else's session asks for a link at ada's address | 403 (the box's owner check) | `apps/grid/tests/preview.rs` `a_box_s_page_is_its_owner_s_alone_through_a_one_time_link_grid_preview_owner_only` |
| Nobody signed in asks for a link | 401 | same |
| ada's API key asks for a link | 403, "never a preview" | same |
| A ticket for ada's label at bob's preview address | 401 | same |
| A ticket used twice | the second is 401 | same |
| The preview cookie set | `wisp_preview`, HttpOnly, SameSite=None, Secure, Partitioned, no `Domain` | same |
| ada's preview cookie at bob's preview, or bob's session value as a preview cookie | 401 | same |
| The grid's own session at a preview address | 401: the preview has its own sign-in | same |
| The preview cookie's value as the grid's session (at the box, at the grid) | 401 / sent to sign in | same |
| Signed in but the box is asleep | 503 `asleep`, the box stays asleep | same |
| A POST with a foreign `Origin` / its own | 403 `forbidden origin` / through | same |
| Signed out: a browser page / a fetch | 302 to `<handle>/_grid/preview?port=&path=` (the same path and query) / 401 | same |
| Port 22, 80, 1023, 0, 65536, `abc`, empty, the grid's own | link 400; address 404 | `…_grid_preview_port_allowlist` |
| Port 1024 | shown | same |
| Previews off | link 400; `p5173--ada` isn't a preview address | `…_grid_preview_off` |
| hostd's template from the box's address | `https://p{port}--ada.grid.example`, trailing `/` dropped, nonsense → `""` | `…_grid_preview_template`; `hostd` env by inspection |
| `WISP_PREVIEW_URL` outside a box, in a sandbox, not `http(s)://p{port}--<host>`, with a path/query/user | `preview: ""` | `plugins/grid-client/host/src/preview.rs` `a_box_publishes_its_grid_s_template_and_nothing_else_does` |
| The row carries the template | `grid-client.cloud.preview` | `view.rs` `the_cloud_row_carries_the_sign_in_under_way_and_who_is_signed_in` |
| `localhost`, `127.0.0.1`, `[::1]`, `0.0.0.0` with path, query, hash | mapped onto the template | `plugins/grid-client/ui/test/preview.test.ts` `a_box_s_localhost_shows_at_its_grid_preview_address` |
| A remote URL, no template, port 80/443/1023, a template that isn't a grid's, this window on loopback | the plain URL | same, `anything_else_keeps_the_plain_url` |
| A task from another machine | not mapped (its localhost isn't this box's) | by inspection (`previewAddress`) |
| The cloud row arrives after the block shows | the frame moves to the preview address (`previewUrl` read live) | by inspection |
| A page's live-reload websocket | piped both ways after the box's 101 | by inspection (`box_preview`) |
