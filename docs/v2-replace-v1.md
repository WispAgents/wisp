# What v2 still needs to replace v1

The owner's list (2026-11-16). Worked one by one, in order, by the lead (no coders: owner, 2026-11-16). Each item is
done only when checked in the real app (dev mode, desktop window and browser, real data) in every state it touches.
Bugs go to `docs/qa/bugs.md`.

1. [ ] **Fix the bugs a user hits**: limit send / Send anyway (B4), the 5-hour window on Claude Ultra (B5), the update
   spinner (B6), adding a crew member (B7), the top bar overlap (B8), not-started turns (B9), the held check-in text
   (B10), the task URL id (B11), and every one found while testing (`docs/qa/bugs.md`).
   *2026-11-16:* B4, B6–B10, B12–B18, B20–B24, B26–B28, B30–B36 fixed or closed, each checked in the running app
   (mock and real server, frames recorded); `qa/flicker.mjs` now gates "something appears then goes" (37 actions, two
   widths, all clean). B5 fixed too (a gone window's notice stayed). B24 fixed by a 50-task first page (ADR-0093). Nothing open.
2. [ ] **Test the running app screen by screen**, every state a user reaches, desktop window and browser, real data,
   against v1.
3. [ ] **Parity, layout and feedback checks against v1** on every view, width and theme (`qa/parity.mjs`,
   `qa/layout.mjs`, `qa/feedback.mjs`, `qa/phone-selection.mjs`), one full pass.
4. [ ] **Windows by hand once**: install, update, the desktop window.
5. [ ] **Real Claude Code and Codex runs** in a wisp-init'ed repo: MCP tools, the skill, the CI action.
6. [ ] **First npm release of v2**: the owner's one-time publish of the 7 `wisp-agents-<os>-<arch>` packages, trusted
   publishing for each, a `release.yml` dry run, the npm publisher re-pointed to WispAgents.
7. [ ] **Binary size**: 43.8 MB vs 15 MB (r327: opt z 19.7 MB but slower; z + 16 hot crates at 3: 24.6 MB): code
   changes (fewer generic copies, e.g. the kernel's typed dispatch: 382 copies, 1.8 MB) or an ADR for a new budget.
8. [ ] **Delete v1** from the repo (`server/`, `web/`, v1 grid, v1 desktop packaging, v1 tests and workflows).
9. [ ] **README and CHANGELOG for v2**.
10. [ ] **Docs and site** (docs.trywisp.dev, the site rework).
11. [ ] **v1 features still in progress**: long threads (after v1's PR merges), phone store release (step 15), phone tap
    presented time and long tasks while streaming.
12. [ ] **CI green several runs in a row on every OS.**

Not needed: iOS (owner, 2026-11-16: ignore it for now).
