# ADR-0130: Unlisted shares open by capability, not by name

- Status: accepted
- Date: 2026-11-26
- Source: the grid audit, round 4 (item `doc1`): `GET /api/shares/by/<handle>/<key>` opened any `anyone` share by
  name, published or not. A share's key is what it is on the sharer's wisp (`crew` for the whole crew, a setup's
  name, `crew:<ids>`), so a handle was enough to read someone's unlisted crew or setup.

## Context

ADR-0038 and ADR-0041 give every share two addresses: its id (`<grid>/s/<id>`, `wisp://open/<id>`), 16 random
characters, and its name (`grid://<handle>/<key>`), which a formula pins as a module source and which sharing the same
thing again keeps. The name is the useful one for something published to the gallery: it reads well, and a module source
names it. For a link shared with a few people it is a leak. Handles are public (they are in every box's address), and
keys are guessable by design, so "anyone with the link" quietly became "anyone who can guess the link". The id never
had this problem: nobody guesses it.

## Decision

1. **A published share stays name-addressable.** `grid://<handle>/<key>` opens it as before (with its access checked
   as before: a published password share still asks for the password).
2. **An unlisted share opens by id, or by name with its capability.** Every share has a `cap`: 128 random bits in hex,
   a new grid.db migration's column (`shares.cap`), backfilled for every share there is and made with each new one. Its
   name is `grid://<handle>/<key>#<cap>`. `GET /api/shares/by/<handle>/<key>` takes the cap as `?cap=` or an
   `x-wisp-cap` header, compared in constant time. Without it (or with a wrong one), an unlisted share answers **404
   with the same body as a name nobody has**, so a guess can't tell an unlisted share from no share. Its sharer needs no
   cap, and neither does a reader a `people` or `teams` share is for (they're named; nothing is guessed). An unlisted
   `password` share needs the cap before the password is asked.
3. **What the grid shows carries the name that opens it.** The sharer's view, a reader's card, and the `/s/<id>` page
   show `grid://<handle>/<key>` once published and `grid://<handle>/<key>#<cap>` otherwise. A card is only shown to a
   reader who opened the share (by id, or by name with its cap) or, published, in the gallery, so the cap it carries tells
   them nothing their link didn't.
4. **Clients keep the fragment.** grid-client's `link()` reads `#<cap>` (`Link::Name { cap }`) wherever `?ref=` sits, and
   sends it to the grid. A `grid://` module source in a formula keeps it: `wisp init` pins `?ref=` before it, and
   lock.yml records the source as written. A crew import keeps the pasted link whole. An old cap-less name that now
   404s says so: "There's no grid://<handle>/<key>, or this link is unlisted: ask its sharer for the new link".
5. **Sharing again keeps who opens it.** A put that names no access keeps the share's access (people, teams, a password);
   only a password typed, or an explicit Open, changes it. A first share is the grid's default, anyone with its link,
   which this ADR makes mean the id or the capability name.

## Consequences

- An unlisted `anyone` share is as private as its id: knowing a handle and guessing a key opens nothing.
- Links already handed out keep working by id (`/s/<id>`, `wisp://open/<id>`, the url every share dialog copies). A
  `grid://h/k` name that someone pinned for an unlisted share stops opening for others: the sharer publishes it or hands
  out the new name with its cap. The error says so.
- One more migration (16), additive: a grid.db without the column gets it, every row its own cap.
- Reaches: the grid (API, share page), grid-client (`wisp open`, `grid-client.fetch`, MCP through it), the CLI's
  `wisp init` modules and `wisp shares`, the crew's import and Share link, the task Share dialog.
