# QA bug list

Every bug found in the running app lands here, one line each: its id, what happens, where it was found, and who
fixes it. A fix comes with a test that fails first, and **the lead checks it in the real app (dev mode, desktop
window and browser, every state it touches) before the line is ticked and before the next job starts** (owner,
2026-11-16, D-0066). No new features while a line here is open. Screenshots in `target/qa/` (not committed).

| Id | Bug | Found | Fix | State |
|---|---|---|---|---|
| B1 | Sending a task from Home leaves the main pane blank: the page opens the client's id, the server made another | owner, v2 desktop preview | coder-r198 | fixed, to re-check |
| B2 | Harness picker shows "Claude Code \| Claude Code" after picking Claude Code (Auto's preview stays) | owner, v2 desktop preview | coder-r198 | fixed, to re-check |
| B3 | Task page toasts "Mark read needs a newer wisp.runtime on this computer" | lead, reproducing B1 | coder-r198 | fixed, to re-check |
| B4 | **Near the plan limit every send fails**: the task is created "Failed"/"Blocked" with text meant for agents ("Send it again with confirmLimit: true (the CLI: --yes)"); v1 asked in the composer with Send anyway. Send anyway doesn't work either | owner, 2026-11-16 (`target/qa/limit-task.png`) | | open |
| B5 | **The 5-hour window shows for a Claude plan that has none** (Ultra: weekly only). v1 fixed it in #556 (a window the latest report leaves out is gone once its reset passes) | owner, 2026-11-16 | | open |
| B6 | **The update pill spins forever** in the top bar: the UI reads the update row flat, the server sends `{id, body: {...}}`, so `status` is missing and counts as busy; tooltip "wisp undefined →". The Updates pane likely has the same mismatch | owner, 2026-11-16 (`target/qa/home-spinner.png`) | | open |
| B7 | **Adding a crew member is buggy** (owner); seen so far: the dialog's first row of "Start from" templates is cut off under the title | owner + lead, 2026-11-16 (`target/qa/crew-add-1.png`) | | open: reproduce fully |
| B8 | Top bar: the machine chip ("61%") and the Machines label overlap (the spinner sits on "Machines") | lead, 2026-11-16 | | open |
| B9 | A turn that never started shows a metrics row (0 · 0 · 0 · 5ms · 0 tokens) and its own "Not started" heading per follow-up; the list says Failed while the page says Blocked | lead, 2026-11-16 | | open |
| B10 | A crew check-in held at the limit reads "test2 · Check-in / at 0% / Held" ("at 0%" means nothing to the user) | lead, 2026-11-16 | | open |
| B11 | The task page URL carries the client's id (`/task/local%3A…`): check B1 isn't back | lead, 2026-11-16 | | open: check |
