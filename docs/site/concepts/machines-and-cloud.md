---
title: Machines and wisp Cloud
description: Run tasks on your other computers and on a server, and reach your wisp from anywhere.
sidebar:
  order: 7
---

Every computer that runs wisp is a **machine**. Your wisp can run tasks on the others, and they all show in one
list.

## Ways to reach a machine

| How | When |
| --- | --- |
| **wisp Cloud (the grid)** | Each computer signs in to the grid. No SSH, no open ports: the grid connects them. |
| **SSH** | A computer you can already `ssh` to. wisp runs over that connection. |
| **Local** | This computer. |

Add one in the app (your avatar → Machines), or declare it:

```yaml
machines:
  studio: { transport: ssh, address: me@studio.local }
```

Then pick it under New task, or from a terminal:

```sh
wisp "run the full test suite" --on studio
wisp ls --on studio
```

## wisp Cloud

[grid.trywisp.dev](https://grid.trywisp.dev) gives you your own wisp on a server. Sign in with GitHub or Google and
your agents keep working after you close the laptop. Use it from the browser, from your phone, or as one more
machine in your app.

```sh
wisp cloud signin           # sign this computer in to wisp Cloud
wisp computer on            # put this computer on the grid, so your other wisps and your phone reach it
```

You can run your own grid on your server with `wisp grid serve` (see [Machines, the Grid and wisp Cloud](../../guides/run/machines/)).

## Pinning work to a machine

A crew member can be tied to a machine (`machine: studio`), so its jobs always run there, and a delegate can run on
another computer than its lead, on a copy of the lead's work.
