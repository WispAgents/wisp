---
title: Install wisp
description: Get wisp on macOS, Linux or Windows, with npm, Homebrew, the install script or the desktop app.
sidebar:
  order: 1
---

wisp runs the coding agents you already have. You need at least one of them installed and signed in:

| Agent | Command wisp runs |
| --- | --- |
| [Claude Code](https://claude.com/claude-code) | `claude` |
| [Codex](https://github.com/openai/codex) | `codex` |
| [opencode](https://opencode.ai) | `opencode` |
| [Cursor CLI](https://cursor.com/cli) | `cursor-agent` |
| [Antigravity CLI](https://antigravity.google/docs/cli/headless/) | `agy` |
| [Grok Build](https://x.ai/cli) | `grok` |
| [GitHub Copilot CLI](https://github.com/features/copilot/cli) | `copilot` |

No agent yet? Settings → Agents installs the ones wisp can install for you, and tells you how to get the others.

## Install

Pick one.

```sh
npm install -g wisp-agents                        # macOS, Windows or Linux, with Node.js 18 or later
curl -fsSL https://trywisp.dev/install.sh | bash  # macOS or Linux: checks Node, then installs with npm
brew install wispagents/tap/wisp-agents           # Homebrew: the command line and the browser app
brew install --cask wispagents/tap/wisp           # Homebrew: the desktop app (macOS 13.3 or later)
```

Or try it without installing: `npx wisp-agents`.

The desktop app for macOS, Windows and Linux is on the [latest release](https://github.com/WispAgents/wisp/releases/latest),
with `SHA256SUMS.txt`. The app updates itself: when a new version is out, its top bar says *Update to …*.

## Open it

```sh
wisp              # opens the desktop app (or a browser tab when there's no app)
wisp --browser    # in the browser: http://localhost:4477
wisp "fix the flaky login test"   # starts a task in this folder right away
```

wisp runs a small local server, `wisp serve`, that the app, the browser, the terminal and your phone all talk to.
The first `wisp` starts it for you.

## No computer to keep on?

[wisp Cloud](../../concepts/machines-and-cloud/) gives you your own wisp on a server. Sign in at
[grid.trywisp.dev](https://grid.trywisp.dev) and your agents keep working when your laptop sleeps.

## Next

- [Run your first task](../first-task/)
- [Give your repo a `.wisp/` folder](../your-repo/)
