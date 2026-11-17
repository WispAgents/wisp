---
title: Plugins
description: Everything in wisp is a plugin, the built-in parts included, with the same API and permissions as yours.
sidebar:
  order: 8
---

wisp's core knows no feature. Running Claude Code, the task list, Ship it, the crew, budgets, Slack: each is a
**plugin**. There are about fifty built in, and yours plug in the same way.

## What a plugin can add

- **Commands**: things wisp can do. Every command is reachable from the app, the CLI (`wisp <plugin> <verb>`), the
  SDKs and MCP.
- **Settings**, with their type and default.
- **UI**: a pane, a Settings page, a button in a toolbar, a card in an answer. UI plugins draw with wisp's own
  components, so they look like wisp.
- **Resource kinds**, so your setup can be declared in a formula.
- **Events and hooks**: react when something happens, or check a run before it starts.

## How plugins talk

A plugin only reaches another through that plugin's public API: its commands, events and extension points. It
never reaches into another's code. That's why a built-in can be switched off or replaced by yours.

## Permissions

Nothing is allowed by default. A plugin's manifest (`wisp-plugin.json`) lists what it may do
(`runtime:get`, `git:diff`, `channels:send`…). A plugin you install starts **off** and shows what it asks for;
switching it on is your consent.

## Kinds of plugin

| Kind | Runs as |
| --- | --- |
| Declarative | A manifest only: settings, events and the commands it calls. |
| With UI | Its manifest plus a `ui.js` that runs in a sandboxed worker: no DOM, no network. It describes wisp components and calls wisp as itself. |
| Exec | A program of yours, in any language, that talks to wisp over stdin/stdout (JSON-RPC). |
| Built-in | Rust, compiled into wisp. |

Start with [Make wisp your own](../../plugins/make-wisp-your-own/).
