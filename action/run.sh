#!/usr/bin/env bash
# The action's one step (action.yml): run it, write the report to the job summary and the pull request, and say how
# it ended. Inputs come as WISP_* variables; the harness's own keys (ANTHROPIC_API_KEY, OPENAI_API_KEY, …) pass
# through the job's env as they are.
set -euo pipefail

args=()
[ -n "${WISP_AGENT:-}" ] && args+=(--agent "$WISP_AGENT")
[ -n "${WISP_MODEL:-}" ] && args+=(--model "$WISP_MODEL")
[ -n "${WISP_MODE:-}" ] && args+=(--mode "$WISP_MODE")
cd "${WISP_PROJECT:-.}"

pr=$(jq -r '.pull_request.number // empty' "${GITHUB_EVENT_PATH:-/dev/null}" 2>/dev/null || true)

# the report on the pull request (when asked and there is one), never failing the step
comment() {
  if [ "${WISP_COMMENT:-true}" = "true" ] && [ -n "$pr" ]; then
    gh pr comment "$pr" --body-file "$1" || echo "::warning::wisp: couldn't comment on #$pr"
  fi
}

# `check: validate|plan`: the .wisp/ folder checked and its plan posted, nothing run; problems or missing secrets fail it
if [ -n "${WISP_CHECK:-}" ]; then
  case "$WISP_CHECK" in
    validate | plan) ;;
    *)
      echo "::error::wisp: check is validate or plan, not $WISP_CHECK"
      exit 2
      ;;
  esac
  rc=0
  checked=$(wisp validate 2>&1) || rc=$?
  shown=$checked
  if [ "$WISP_CHECK" = "plan" ]; then
    prc=0
    planned=$(wisp plan 2>&1) || prc=$?
    shown=$(printf '%s\n\n%s\n' "$planned" "$checked")
    [ "$rc" -eq 0 ] && rc=$prc
  fi
  verdict=$([ "$rc" -eq 0 ] && echo ok || echo problems)
  {
    echo "### wisp $WISP_CHECK: $verdict"
    echo
    echo '```'
    printf '%s\n' "$shown"
    echo '```'
  } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  echo "state=$verdict" >> "${GITHUB_OUTPUT:-/dev/null}"
  body=$(mktemp)
  {
    echo "**wisp $WISP_CHECK** ($verdict)"
    echo
    echo '```'
    printf '%s\n' "$shown"
    echo '```'
  } > "$body"
  comment "$body"
  printf '%s\n' "$shown"
  if [ "$rc" -ne 0 ]; then
    echo "::error::wisp: the setup has problems or secrets that aren't set (wisp $WISP_CHECK)"
    exit 1
  fi
  exit 0
fi

# `deploy: plan|apply` (ADR-0147): a team hub's folder planned or deployed to its hub with a deploy key, straight to the
# hub's address (no wisp runs here); the plan or what was applied goes on the pull request
if [ -n "${WISP_DEPLOY:-}" ]; then
  case "$WISP_DEPLOY" in
    plan) verb=plan ;;
    apply | deploy) verb=deploy ;;
    *)
      echo "::error::wisp: deploy is plan or apply, not $WISP_DEPLOY"
      exit 2
      ;;
  esac
  if [ -z "${WISP_HUB_KEY:-}" ]; then
    echo "::error::wisp: deploy needs hub-key (a deploy key: wisp team key <team> <name>, kept in the repo's secrets)"
    exit 2
  fi
  rc=0
  said=$(WISP_HUB_KEY="$WISP_HUB_KEY" wisp grid "$verb" "${WISP_HUB_DIR:-.}" --yes 2>&1) || rc=$?
  verdict=$([ "$rc" -ne 0 ] && echo failed || { [ "$verb" = plan ] && echo planned || echo deployed; })
  {
    echo "### wisp hub $verb: $verdict"
    echo
    echo '```'
    printf '%s\n' "$said"
    echo '```'
  } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  echo "state=$verdict" >> "${GITHUB_OUTPUT:-/dev/null}"
  body=$(mktemp)
  {
    echo "**wisp hub $verb** ($verdict)"
    echo
    echo '```'
    printf '%s\n' "$said"
    echo '```'
  } > "$body"
  comment "$body"
  printf '%s\n' "$said"
  if [ "$rc" -ne 0 ]; then
    echo "::error::wisp: the hub $verb failed"
    exit 1
  fi
  exit 0
fi

# `formula` is the old name of `task-file`
task_file=${WISP_TASK_FILE:-}
if [ -z "$task_file" ] && [ -n "${WISP_FORMULA:-}" ]; then
  echo "::warning::wisp: the formula input is now task-file; formula still works for now"
  task_file=$WISP_FORMULA
fi

# a launch that fails says wisp's own words, not nothing under set -e (a task file with no task in it); its output is
# left in $launched
launched=
launch() {
  local rc=0 err
  err=$(mktemp)
  launched=$("$@" 2> "$err") || rc=$?
  if [ "$rc" -ne 0 ]; then
    local said
    said=$( (cat "$err"; printf '%s\n' "$launched") | tr '\n' ' ' | sed 's/  */ /g; s/^ //; s/ $//')
    echo "::error::wisp: ${said:-wisp exited $rc}"
    exit 1
  fi
  cat "$err" >&2
}

# the task's id: from `wisp attach <id>` (a task file, a declared task) or the launch's JSON (a prompt)
attach_id() { printf '%s\n' "$launched" | grep -o 'wisp attach [^ ]*' | tail -1 | awk '{print $3}' || true; }
if [ -n "$task_file" ]; then
  launch wisp run -f "$task_file" ${args[@]+"${args[@]}"}
  id=$(attach_id)
elif [ -n "${WISP_TASK_NAME:-}" ]; then
  launch wisp run "$WISP_TASK_NAME" ${args[@]+"${args[@]}"}
  id=$(attach_id)
elif [ -n "${WISP_PROMPT:-}" ]; then
  launch wisp ${args[@]+"${args[@]}"} --json -- "$WISP_PROMPT"
  id=$(printf '%s\n' "$launched" | jq -r 'if type == "array" then .[0].id else .id end // empty')
else
  echo "::error::wisp: give it a prompt, a task, a task-file or a check"
  exit 2
fi
if [ -z "$id" ]; then
  echo "::error::wisp started nothing"
  exit 1
fi
# how the wait ended (0 done or review, 1 failed/blocked/stopped, 3 waits on someone, 124 timed out): kept for the end,
# after the report is out
rc=0
wisp wait "$id" > /dev/null || rc=$?
state=$(wisp show "$id" --json | jq -r '.state // "unknown"')
report=$(wisp show "$id" || true)

{
  echo "task=$id"
  echo "state=$state"
} >> "${GITHUB_OUTPUT:-/dev/null}"
{
  echo "### wisp: $state"
  echo
  echo '```'
  printf '%s\n' "$report"
  echo '```'
} >> "${GITHUB_STEP_SUMMARY:-/dev/null}"

body=$(mktemp)
{
  echo "**wisp** ran \`$id\` ($state)"
  echo
  printf '%s\n' "$report"
} > "$body"
comment "$body"

if [ "${WISP_FAIL_ON:-failed}" = "failed" ]; then
  case "$rc" in
    0) ;;
    1)
      echo "::error::wisp: the task failed|was blocked|stopped ($id, $state)"
      exit 1
      ;;
    3)
      echo "::error::wisp: the task is waiting on input nobody in CI will answer ($id)"
      exit 1
      ;;
    124)
      echo "::error::wisp: timed out ($id)"
      exit 1
      ;;
    *)
      echo "::error::wisp: wisp wait exited $rc ($id)"
      exit 1
      ;;
  esac
  if [ "$state" = "error" ]; then
    echo "::error::wisp: the task failed ($id)"
    exit 1
  fi
fi
