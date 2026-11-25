# ADR-0122: the phone app loads an answer's pictures, videos and attachments through its computer's transport, the grid's by signed URL

- Status: accepted
- Date: 2026-11-25
- Source: owner, 2026-11-25: "Why images dont show up in mobile app" → "Fix it with short-lived signed file URLs the
  server accepts in the query string, so <img> works on the phone directly."
- With ADR-0065 (the phone apps), ADR-0010 (peers and transports), ADR-0058 (the grid's edge sign-in).

## Context

An answer's `image`/`video` block and a prompt's attachment load from the task's computer with a relative URL
(`/api/sessions/<id>/file?path=…`, `/api/uploads/<id>`). The desktop and the web app are served by that wisp, so it
works there. The phone app's page is the app's own bundle at `https://localhost`: the relative URL hit the bundle (404),
and every one showed "This picture can't be shown here." An absolute URL isn't enough on the grid: its edge lets a
request in only with the device's bearer token, a cookie or an API key, and a plain `<img>`/`<video>` sends none.

## Decision

1. **A transport may say where a page loads a file** (`Transport.mediaUrl(path)`, `PeerHttp.media`): an absolute URL
   that needs no header. None (the desktop, the web app): the page's own relative URL stays.
2. **The phone's transport:** over SSH, the tunnel's local address (`http://127.0.0.1:<port>`; wisp trusts loopback).
   Through the grid, a URL the grid signed.
3. **The grid signs** (`POST /_grid/media {paths}` at the wisp's or computer's own address, with the device's token,
   up to 100 paths): `?_ge=<expiry>&_gd=<device>&_gs=<HMAC>` over the address, the account, the device, the expiry and
   the exact path and query. Only `GET`/`HEAD` of an answer's file or an upload (on the wisp or a machine through it,
   `/api/peer/<m>/…`); **an hour**; the device revoked, its links stop at once. The edge checks it last (after the
   token, key and cookie), takes the signature off and lets the request through as the user. The wisp itself is
   unchanged: the grid's edge is the only gate an `<img>` can't pass.
4. The phone asks for the pictures of one answer in one POST, keeps each link 50 minutes, and shows the block's
   skeleton while it asks (`BlockContext.media` answers `null`: on its way), "can't be shown" only on a refusal.

## Consequences

- A signed link is a bearer credential for one file for at most an hour: copied out of the app, it opens that file and
  nothing else. The query (not a header) carries it, so it can show in the grid's access logs; the grid logs no query.
- Places it reaches (ADR-0040): the phone app (Android now, iOS with the same transport). Agents, CLI and channels
  never needed it: they read files on the computer.
- Not covered: My Stuff thumbnails on the phone use the same relative pattern (`/api/stuff/<id>/file`) and are next.
