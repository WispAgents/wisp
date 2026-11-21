# Security

wisp runs coding agents on your code, and wisp Cloud runs them on our server, so we take reports
seriously.

## Reporting a vulnerability

Report it privately, not in a public issue:

- **GitHub:** [Report a vulnerability](https://github.com/WispAgents/wisp/security/advisories/new) (private advisory).
- **Email:** [hello@trywisp.dev](mailto:hello@trywisp.dev), with "security" in the subject.

Include what you found, how to reproduce it, and the version (`wisp --version`) or, for wisp Cloud,
the time it happened. We reply within three working days, keep you posted while we fix it, and
credit you in the release notes if you'd like.

## What's in scope

- wisp itself: the local server, the page, the CLI, the desktop app and the Android app.
- wisp Grid and the hosted wisp Cloud at `grid.trywisp.dev`: sign-in, the edge, the tunnel and
  the isolation between users' boxes.
- The install script at `https://trywisp.dev/install.sh` and the release downloads.

Spells are meant to be sandboxed (no network, no page, no wisp API): a way out of that sandbox is in
scope too.

## What wisp's checks are, and aren't

wisp keeps an agent to its own task: with its own token it can't raise a task's mode, and a change to your setup it
asks for (a connector, a crew member, a project) is a card you allow or deny; nothing runs before you allow it, and the
agent can't allow it itself (ADR-0112, ADR-0113).

That isn't a wall against an agent that can run commands on your computer as you. wisp's server treats a request on
its own port with no token as you, and an agent with a shell can also edit your files or run `wisp` itself. So:

- In **Plan**, and in **Edits** where each command waits for your OK (Claude Code), the agent has no shell of its own.
- In **Full auto**, the agent runs commands as you: give it work you'd trust it with.
- For work you don't trust, use a **sandbox** (`/sandbox`, ADR-0047): it runs on a machine of its own, and what it can
  reach without a token is that sandbox's wisp, not your computer's.

A report that an agent got past these without a shell (or out of a sandbox) is in scope.

## Supported versions

Fixes land in the latest release. Update with `npm install -g wisp-agents@latest`, or let the app
update itself.
