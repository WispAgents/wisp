# desktop: wisp's Tauri shell

The window loads the page `wisp serve` serves; the server is the same `wisp` binary Homebrew and npm ship, run as a
child (`src/server.rs`) or attached to when one already answers on the port (`WISP_PORT`, default 4477).

## Dev mode

```sh
npm run dev:desktop               # from the repo root (tools/dev.ts --desktop)
npm run dev:desktop -- --help     # --port, --vite-port, --home, --no-watch
```

It builds and starts `wisp serve` from the checkout on port 4800 (the next free one if busy) with its own home,
`~/.wisp-dev`, starts Vite with HMR proxying `/api` to it, then `cargo run`s this shell with:

- `TAURI_CONFIG` = `tauri.dev.conf.json`: identifier `dev.wisp.dev`, so its single-instance lock and data dir never
  meet an installed wisp's (`dev.wisp.app`);
- `WISP_PORT` = the dev server's port: the shell attaches to it and never starts or reaches the one on 4477;
- `WISP_DEV_URL` = Vite's URL: the window loads it, and IPC is granted to that origin only. Debug builds only; a
  release build ignores it.

The shell always builds into its own target dir (`$TMPDIR/wisp-target-desktop-dev`, or `WISP_DEV_DESKTOP_TARGET`):
its binary is also called `wisp` and would overwrite the server's, and the dev identifier is compiled in.

UI edits hot reload in the window. A change under `crates/` or `plugins/*/host` rebuilds the server and restarts it
on the same port (the page reconnects); `r` + Enter does it by hand. A change to this shell's Rust needs a new run.
Closing the window, `q` + Enter or Ctrl-C stops everything the run started, by pid.
