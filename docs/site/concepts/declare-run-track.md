---
title: Declare, Run, Track
description: The three things wisp does, and how the rest of these docs map onto them.
sidebar:
  order: 1
---

Infrastructure as code put servers in the repo. wisp puts **the way your software gets built** there: who builds
it, how, and what was asked along the way. It does three things.

## Declare

Your agents, your crew, the machines they run on, their skills and MCP servers, your rules and your workflows are
files in `.wisp/` ([formulas](../formulas/)). You change them in a pull request, review them like code, and share
them across repos as modules. Everything you set up in the app can be written back to files with `wisp export`,
and `wisp plan` shows what changed in the app since.

## Run

Any coding agent, many at once, each [task](../tasks/) in its own git worktree, on this computer, your
[other computers or the cloud](../machines-and-cloud/). You drive it from wherever you are: the desktop app, a
browser, the terminal (`wisp`), your phone, Slack or Telegram, your own agent over MCP, or your own code through
the SDK. A [crew](../crew/) of long-lived agents can check in on a schedule and wake on events.

## Track

Every task is a file: what was asked, the agent's answer, its transcript and its commits. The repo keeps the ones
you choose in [`.wisp/tasks`](../wisp-folder/). `wisp blame` tells you which task wrote a line and why,
`wisp resume` carries on an old task on today's code, and `wisp rerun` replays one from the commit it started on.

## One engine, many ways in

The app, the TUI, the CLI, the SDKs, the MCP tools and the chat channels all call the same commands
([reference](../../reference/commands/)), so anything you can do in one you can do in the others. Everything in
wisp, the built-in parts included, is a [plugin](../plugins/).
