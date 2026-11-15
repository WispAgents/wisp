# wisp v2: where things stand

Updated by the lead every turn. The plan is `docs/v2-queue.md`; feature progress is `docs/v1-features.md`; bugs are
`docs/qa/bugs.md`; decisions are `docs/adr/`. This file is the live state: what runs now, what waits, what's next.

_Last updated: 2026-11-15 01:25 (round 234)._

## Progress

- **v1 features:** 315 built · 6 partial · 6 missing (all missing ones are the phone apps). Regressed: F-comp-1, the
  harness picker (owner, 2026-11-15).
- **CI:** last full run (2790d627) failed one test on every Linux and macOS job (`serve_orphans`
  `a_probe_whose_caller_hung_up_is_ended`); Windows jobs were cancelled; earlier runs had Linux and macOS green.
- **Budgets:** initial JS 117.9 KB br (≤ 120), CSS 29.7 KB (≤ 30), checked in CI.
- **Weekly usage:** 85% (the owner applies the reset); 3 coders while it allows.

## Running now

| Coder | Job |
|---|---|
| coder-r233 | Harness picker back to v1: one grouped pill, one two-pane popover (owner: "two pickers for one thing"); review against `design/v1-reference/home.runner` before commit |
| coder-r234 | CI: `a_probe_whose_caller_hung_up_is_ended` (round 211 vs round 212's probe memo) |

## Waiting for the lead

- **Verify, then commit** (these coders ended before their own reviews):
  - round 227: gesture seams + phone step 5 (overlays: Sheet, Dialog, Menu, Popover, Toast, Tooltip)
  - round 228: phone step 11 (push: `plugins/push`, grid relay, `hpke`)
  - round 229's shell hookups (PeerBanner, Waiting to send on rows) — mixed with round 227's files
- **Push:** local commits since 2790d627 (bundle under budget, phone steps 7 and 9, phone web, DX: `npm run dev`,
  `dev:desktop`, `doctor`, the commands check, offline client/sync), after coder-r234's fix.

## Blocked

- **Phone step 10 (iPhone app):** needs Xcode on this Mac (only the command-line tools are installed).

## Next

1. Harness picker parity (r233), then CI green on every OS (Windows has open groups).
2. Phone steps 6 (controls), 8 (the app's frontend), 13–15 (screens, store).
3. Developer experience items 4–5 (`docs/dev.md`, no collisions everywhere).
4. Task history redesign: waiting on the owner's pick (A / B / C).
5. Cutover, docs and site, the batched verification pass (queue items 9–11).
6. After v2 is 100%: Linear TODO tickets, then continuous QA by fresh agents (owner's order).
