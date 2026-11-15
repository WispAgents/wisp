# Contributing to wisp

Thanks for helping. Bug reports, harness adapters, spells and fixes are all welcome.

## Before you start

- **Bugs:** open an [issue](https://github.com/WispAgents/wisp/issues/new/choose) with the version
  (`wisp --version`), your OS, the harness and what you expected to happen.
- **Bigger changes:** open an issue first so we can agree on the shape before you write it.
- **Security issues:** don't open an issue; see [SECURITY.md](SECURITY.md).

## Working on the code

```sh
git clone https://github.com/WispAgents/wisp && cd wisp
npm install
npm run dev:web     # the server with the page, restarts on server changes
npm test            # everything
```

[AGENTS.md](AGENTS.md) explains how the code is laid out and the rules a change has to follow
(it's written for coding agents, and for people). In short:

- No new runtime dependencies. The server is plain Node, the page is vanilla JS with no build step.
- Every piece of UI you add must be reachable from a spell (`web/js/spell-schema.js`).
- New test files go in `test/`, named `*.test.js`: `npm test` finds them.
- User-facing changes get a line under *Unreleased* in [CHANGELOG.md](CHANGELOG.md).

## Agent tooling

If you work with a coding agent, install the two tools that keep it in line with what's already
been decided:

```sh
brew install wispagents/tap/decision-ledger wispagents/tap/ingrain
decisions init && ingrain init     # in your wisp checkout: adds the git pre-commit hooks
```

The Claude Code hooks are already in `.claude/settings.json`, and they do nothing until the tools
are installed. Once they are, the agent sees the relevant decisions from `decisions/` before it edits
a file, and the learned rules in `.corrections/` are checked as it writes.

To record a decision (a convention, a constraint, a choice future code must follow), propose it:

```sh
decisions add --propose "Settings panes use container queries, not fixed widths." \
  --reason "they must work at phone widths" --scope "web/css/**"
decisions review                   # accept, edit or reject; then commit decisions/
```

To change an accepted decision, don't edit its file: run
`decisions supersede D-0012 --decision "..." --reason "..."` and review it the same way. CI checks
every PR against both (`.github/workflows/ledger.yml`).

## Pull requests

`main` is protected: work on a branch and open a PR. CI runs the suite on Linux; label it `ci:full`
when it touches process launching, paths, shells or terminals. Keep a PR to one change, and say
how you tested it.

By contributing you agree your work is released under the [MIT licence](LICENSE).
