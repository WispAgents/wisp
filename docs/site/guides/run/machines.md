---
title: Run on your other computers and in the cloud
description: Add a computer over SSH or wisp Cloud, start tasks there, and reach your wisp from your phone.
sidebar:
  order: 4
---

## Add a computer

**Over wisp Cloud** (no SSH, no open ports): install wisp on the other computer, then on it:

```sh
wisp cloud signin
wisp computer on
```

It shows up in your app under your avatar → Machines.

**Over SSH**: a computer you can already `ssh` to, with wisp installed. Add it in Machines with its SSH target, or:

```yaml
machines:
  studio: { transport: ssh, address: me@studio.local }
```

## Run there

Pick the machine under New task, or:

```sh
wisp "run the full test suite" --on studio
wisp ls --on studio
```

Every wisp command takes `--on <machine>`. The task's row shows where it runs.

## Keep work on one machine

- A crew member's jobs: `machine: studio` in its spec.
- A workflow step: `on: studio`.
- A delegate can run on another computer than its lead, on a copy of the lead's work, and its result comes back.

## wisp Cloud

Sign in at [grid.trywisp.dev](https://grid.trywisp.dev) to get your own wisp on a server, that keeps working when
your laptop sleeps. Use it in the browser, from the phone app, or as one more machine in your app. Run your own grid
with `wisp grid init`.
