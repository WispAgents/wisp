---
title: Move from wisp v1
description: Bring your v1 projects, recipes, crew, automations, machines and task history into wisp v2 with one command.
sidebar:
  order: 1
---

wisp v2 is a new wisp: faster, with every v1 feature, and your setup can now live in your repo as a formula. Your
v1 setup and history come with you in one command, which shows what it will do before it does anything.

## Import

```sh
wisp import --dry-run      # the plan only
wisp import                # the plan, then "Import it? [y/N]"
```

It reads v1's home (`~/.wisp`, or the folder you name: `wisp import /path/to/v1-home`) and brings in:

- **Projects**, **defaults**, **recipes** (as declared tasks), **automations and schedules** (as triggers), **crew
  members** and **teams**, **machines** and **connectors**: shown as a plan, as `wisp apply` would show it.
- **Task history**: every v1 task with its transcript and its changes per turn. Tasks are history, so they are
  imported, never planned. A task already imported is skipped, so you can run it again.

## Secrets

Tokens and keys are never written into a plan. A connector's environment or a channel's token that wisp already has
becomes a `${secret:NAME}` reference. Any other is named in the plan's notes. Set it with:

```sh
wisp secret set NAME       # typed hidden, or piped: echo "$TOKEN" | wisp secret set NAME
```

## What changed

| v1 | v2 |
| --- | --- |
| Spells | [Plugins](../../concepts/plugins/). A spell isn't imported: [Make wisp your own](../../plugins/make-wisp-your-own/) shows how to rebuild one, or ask an agent to. |
| Settings only in the app | The same settings, plus a formula in your repo (`.wisp/*.wisp.yml`) with `wisp plan` and `wisp apply`. |
| Task history only on your machine | Optionally kept in the repo (`.wisp/`), so the team sees why the code is the way it is. |

Everything you did in v1's app is in the same place in v2's, and looks the same.

## Keep your setup in the repo

After importing, write it out as a formula you can review and commit:

```sh
wisp export > .wisp/setup.wisp.yml
```

See [Formulas](../../concepts/formulas/).
