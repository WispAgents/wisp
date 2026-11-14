# QA bug list

Every bug a QA round finds lands here, one line each: its id, what happens, where it was found, and who fixes it. A
fix comes with a test that fails first; a later QA round checks it again before the line is ticked. QA rounds are
run by fresh agents with no list of known bugs (owner, 2026-11-14); the lead never QAs.

| Id | Bug | Found | Fix | State |
|---|---|---|---|---|
| B1 | Sending a task from Home leaves the main pane blank: the page opens the client's id, the server made another | owner, v2 desktop preview | coder-r198 | fixed, to re-check |
| B2 | Harness picker shows "Claude Code \| Claude Code" after picking Claude Code (Auto's preview stays) | owner, v2 desktop preview | coder-r198 | fixed, to re-check |
| B3 | Task page toasts "Mark read needs a newer wisp.runtime on this computer" | lead, reproducing B1 | coder-r198 | fixed, to re-check |
