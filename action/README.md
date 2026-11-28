# wisp in CI

The GitHub Action (ADR-0040). It runs a wisp task in a job, posts the report on the pull request and fails the job when
the task doesn't end well. It can also check the repo's `.wisp/` setup and post its plan without running anything.

```yaml
- uses: wisp-agents/action@v1
  with:
    prompt: Fix the failing test
    agent: claude
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

```yaml
# on every pull request: is the setup valid, and what would applying it change?
- uses: wisp-agents/action@v1
  with:
    check: plan
```

```yaml
# a team hub's folder (ADR-0148): planned on every pull request, deployed on every merge to main
- uses: wisp-agents/action@v1
  with:
    hub: ops/hub                     # the folder with grid.yml (wisp grid init writes this workflow for you)
    deploy: ${{ github.event_name == 'push' && 'apply' || 'plan' }}
    hub-key: ${{ secrets.WISP_HUB_KEY }}   # wisp team key <team> ci, or the team page's Deploy keys
```

## Inputs

| input | what it does | default |
| --- | --- | --- |
| `prompt` | what the agent should do (`wisp "<prompt>"`) | |
| `task` | a declared task to run (`wisp run <name>`), instead of a prompt | |
| `task-file` | a task file to run (`wisp run -f <file>`, a `.task.yml`), instead of a prompt | |
| `formula` | deprecated: the old name of `task-file`; still works, with a warning | |
| `check` | `validate` or `plan`: runs `wisp validate` (and `wisp plan`) on `.wisp/`, posts the output as the pull request comment and step summary, and fails on problems or secrets that aren't set. Needs no prompt or task | |
| `deploy` | `plan` or `apply`: plans the hub folder (`wisp grid plan`) or deploys it to its team's hub (`wisp grid deploy --yes`) with `hub-key`, straight to the hub's address; posts the output as the pull request comment and step summary. Needs no prompt or task | |
| `hub` | the hub's folder (it holds `grid.yml`), for `deploy` | `.` |
| `hub-key` | a deploy key for the hub (`wgt_…`); `plan` takes a plan-only key | |
| `project` | the folder it runs in | `.` |
| `agent` | the harness (claude, codex, opencode, …); the task's or wisp's default when left out | |
| `model` | the model | |
| `mode` | `plan`, `edits` or `auto` | `edits` |
| `version` | the wisp-agents version to install | `latest` |
| `comment` | post the report on the pull request (`true` or `false`) | `true` |
| `fail-on` | `failed`: fail the job when the task failed, was blocked or stopped, waits on input, or timed out; `never`: always pass | `failed` |
| `github-token` | the token `gh` posts the comment with | `${{ github.token }}` |

Give one of `prompt`, `task`, `task-file` or `check`.

## Outputs

| output | what it is |
| --- | --- |
| `task` | the task's id |
| `state` | how it ended (`review`, `done`, `error`); with `check`, `ok` or `problems`; with `deploy`, `planned`, `deployed` or `failed` |

## How the job ends

With `fail-on: failed`, the step writes the summary and the comment first, then exits 1 with the reason:

| `wisp wait` | the job |
| --- | --- |
| 0 (done or review) | passes, unless the task's state is `error` |
| 1 | `the task failed\|was blocked\|stopped` |
| 3 | `the task is waiting on input nobody in CI will answer` |
| 124 | `timed out` |

A launch that fails (a task file with no task in it, say) fails the step with wisp's own message.

`test/run.test.sh` checks all of this against a stub `wisp`: `bash action/test/run.test.sh`.
