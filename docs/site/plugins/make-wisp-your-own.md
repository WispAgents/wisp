---
title: Make wisp your own
description: Change how wisp works, from a setting to a plugin an agent writes for you, with the same API wisp's own parts use.
sidebar:
  order: 1
---

wisp's own parts are plugins, and yours plug in the same way. Pick the smallest step that does what you want.

## 1. A setting

Most changes are a setting: your avatar → Settings, or `wisp settings list` and `wisp settings set`. Every one is listed in
[the settings reference](../../reference/settings/).

## 2. A formula

Agents, skills, MCP servers, tasks, workflows, triggers and the crew can be declared in your repo, reviewed and
shared like code. See [Formulas](../../concepts/formulas/).

## 3. Switch a built-in off

Don't want a part of wisp? Switch it off in Settings → Plugins, or:

```sh
wisp plugin                       # every plugin, on or off
wisp plugin disable wisp.slack
```

Everything it made goes with it (its timers, processes and watchers). Switch it back on any time.

## 4. Ask an agent to write a plugin

Settings → Plugins → **Cast a plugin**. Describe the change ("a floating button that opens all my PRs") and send.
A task starts whose agent has four extra tools:

- `wisp_plugins`: the plugins you have, and each one's manifest.
- `wisp_plugin_schema`: the whole plugin reference, with worked examples.
- `wisp_plugin_data`: a sample of the real answer of a command the plugin will call, so it reads the right fields.
- `wisp_plugin_check`: wisp's own checker, run on the plugin before it's installed.

The agent writes the plugin and installs it **off**. You read what it asks for, then switch it on.

## 5. Write one yourself

A plugin is a folder with a `wisp-plugin.json`:

| Kind | What's in the folder | Good for |
| --- | --- | --- |
| Declarative | the manifest only | settings, a theme, commands it calls on events |
| With UI | the manifest and a `ui.js` | a Settings page, a pane, a button, a card in an answer |
| Exec | the manifest and a program in any language | new commands, reacting to events, talking to other tools |

[Your first plugin](../first-plugin/) builds an exec plugin step by step.
[Plugin examples](../../reference/plugin-examples/) has complete UI plugins to copy, and the
[manifest reference](../../reference/plugin-api/) lists every key and component.

## Install, share, remove

```sh
wisp plugin install ./my-plugin                        # a folder
wisp plugin install https://github.com/you/wisp-hello  # a git repo
wisp plugin enable user.hello
wisp plugin remove user.hello
```

What you install starts off, and runs only with the permissions its manifest asks for, never yours. Installing a
newer version replaces the old one; until you switch it on again it stays off.

## Safe by default

- A plugin reaches only what its `permissions` name. A call outside them is refused.
- A UI plugin runs in a sandbox: no page, no network, no storage. It draws with wisp's components only, so it can't
  look like something it isn't.
- A plugin talks to the rest of wisp only through public commands and events, so the built-ins it uses can change
  inside without breaking it.
