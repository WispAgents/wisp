# QA

End-to-end scenarios for every surface wisp ships on, and scripts that run the automatable ones.
`npm test` checks the parts; this checks the product the way a person uses it, before a release or
after a change that touches more than one surface.

| Surface | Scenarios | Script | What it drives |
|---|---|---|---|
| Desktop app (every OS) | [scenarios/desktop.md](scenarios/desktop.md) | `node qa/desktop-v2.mjs` | the Tauri shell (`desktop/src-tauri`) on a v2 `wisp serve`: `tauri.yml`, `tauri-package.yml`, `tauri-windows.yml` |
| Android app | [scenarios/mobile.md](scenarios/mobile.md) (`MOB-*`) | `node qa/platform.mjs` | the phone app (mobile/web) as the app's WebView sees it (qa/phone.mjs); APK, Gradle and emulator steps are in the scenarios |
| Looks like v1 | `design/v1-reference/` | `node qa/parity.mjs` | every view, width, theme and state of v2 against v1's frozen shots and metrics (`qa/views.mjs` openers, `qa/v1-parts.mjs`) |
| Nothing wraps | `design/layout.md` | `node qa/layout.mjs` | every bar and row at 320–1440 px |
| Answers within a frame | `design/feedback.md` | `node qa/feedback.mjs` | every interactive element: a visible change within 100 ms |

`scenarios/web.md`, `grid.md` and `install.md` are v1's scenarios, kept as the list of what v2's surfaces must do;
their scripts (`web.mjs`, `grid.mjs`, `install.mjs`, `desktop-tauri*.mjs`, `buttons.mjs`, `ui-audit.mjs`, the perf
runs) drove v1 and left the repo with it on 2026-11-27 (they are at a7c98f73).

Each scenario says how it's covered: **Automated** (the script), **Suite** (`npm test`),
**Gradle** / **Emulator** (Android steps in the scenario), or **Manual**.

## Setup

```sh
mkdir -p /tmp/wisp-qa-pw && npm i --prefix /tmp/wisp-qa-pw playwright-core
(cd mobile && npm install)                         # Capacitor's JS, for the mobile preview
```

Playwright stays out of the repo (wisp has no dependencies): the scripts load it from `PW_DIR`
(default `/tmp/wisp-qa-pw`) and drive the system Chrome (`CHROME` to use another).

`QA_ENGINE=webkit` runs the gates on Playwright's WebKit instead, the engine of the Tauri app on a Mac (`cd /tmp/wisp-qa-pw && npx playwright-core install webkit`). What's
known to differ, and the lowest version, is in [scenarios/desktop.md](scenarios/desktop.md).

## Isolation

Nothing touches your own wisp. The gates drive apps/web's mock or a dev wisp you start on its own port and home
(`docs/dev.md`); `qa/desktop-v2.mjs` starts its own `wisp serve` with a temp `WISP_HOME`. A wisp left on a port from
an earlier run would answer with its own state: `lsof -ti tcp:PORT -sTCP:LISTEN | xargs kill`.

## Nothing appears and goes

`node qa/flicker.mjs` starts apps/web's mock (or `FLICKER_URL=` a running one) and does 37 actions at 1440 and
390 px (open a task, Transcript, Mark done, a follow-up, New task, the palette, Settings and a pane, the crew rail
and editor, My Stuff, archive, Done). In the page, every element added is watched; one seen on screen and gone
within 1 s fails (a node made again: hover, focus and entry animation lost; or something that showed and went), and
so does an action that changed nothing (no DOM change, no focus move). Results in `qa/out/flicker/summary.json`.
`--widths=390 --only=follow-up` narrows it.

## Results

Each script prints a line per scenario id and exits non-zero on a failure. Screenshots,
logs and `<surface>.json` (id, pass/fail, error, time) go to `qa/out/` (`QA_OUT` to move it;
gitignored). Look at the screenshots, not just the exit code: layout regressions at phone width or in dark mode don't throw.
