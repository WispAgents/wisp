#!/usr/bin/env bash
# action/run.sh against a stub `wisp` (and `gh`) on PATH: the job's exit for each way a wait ends, the formula alias,
# a launch that fails, and the check mode. Run it directly: bash action/test/run.test.sh (needs jq).
set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
run_sh="$here/../run.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"

# the stub: `run` prints an attach line (or fails with STUB_RUN_FAIL), `wait` exits STUB_WAIT_RC, `show` reports
# STUB_STATE, `validate`/`plan` exit STUB_VALIDATE_RC/STUB_PLAN_RC; every call is logged
cat > "$tmp/bin/wisp" << 'EOF'
#!/usr/bin/env bash
echo "wisp $*" >> "$STUB_LOG"
case "$1" in
  run)
    if [ -n "${STUB_RUN_FAIL:-}" ]; then echo "  ✗ $STUB_RUN_FAIL" >&2; exit 1; fi
    echo "  ✓ started: wisp attach t-123"
    ;;
  wait) exit "${STUB_WAIT_RC:-0}" ;;
  show) if [ "${2:-}" != "" ] && [ "${3:-}" = "--json" ]; then echo "{\"state\":\"${STUB_STATE:-review}\"}"; else echo "the report"; fi ;;
  validate) echo "  ✓ 3 resources"; exit "${STUB_VALIDATE_RC:-0}" ;;
  grid) echo "  key=${WISP_HUB_KEY:-none} $2 $3 $4"; echo "  + trigger/triage  (new)"; exit "${STUB_GRID_RC:-0}" ;;
  plan) echo "  + agent/critic  (new)"; exit "${STUB_PLAN_RC:-0}" ;;
  *) echo '{"id":"t-9"}' ;;
esac
EOF
printf '#!/usr/bin/env bash\necho "gh $*" >> "$STUB_LOG"\n' > "$tmp/bin/gh"
chmod +x "$tmp/bin/wisp" "$tmp/bin/gh"

fails=0
# run.sh with these WISP_*/STUB_* variables; sets $rc and $out
go() {
  : > "$tmp/log"
  out=$(env -i PATH="$tmp/bin:$PATH" HOME="$tmp" STUB_LOG="$tmp/log" GITHUB_OUTPUT="$tmp/out" GITHUB_STEP_SUMMARY="$tmp/summary" "$@" bash "$run_sh" 2>&1)
  rc=$?
}
expect() { # <name> <want rc> [text the output holds]
  if [ "$rc" -ne "$2" ] || { [ -n "${3:-}" ] && ! grep -qF -- "$3" <<< "$out"; }; then
    echo "FAIL $1: exit $rc (want $2)"
    printf '%s\n' "$out" | sed 's/^/    /'
    fails=$((fails + 1))
  else
    echo "ok   $1"
  fi
}

go WISP_PROMPT="fix it" STUB_WAIT_RC=0
expect "a task that ends well passes" 0
go WISP_PROMPT="fix it" STUB_WAIT_RC=1
expect "wait 1 fails the job" 1 "::error::wisp: the task failed|was blocked|stopped"
go WISP_PROMPT="fix it" STUB_WAIT_RC=3
expect "wait 3 fails the job" 1 "::error::wisp: the task is waiting on input nobody in CI will answer"
go WISP_PROMPT="fix it" STUB_WAIT_RC=124
expect "wait 124 fails the job" 1 "::error::wisp: timed out"
go WISP_PROMPT="fix it" STUB_WAIT_RC=1 WISP_FAIL_ON=never
expect "fail-on never passes" 0
go WISP_PROMPT="fix it" STUB_STATE=error
expect "state error still fails" 1 "::error::wisp: the task failed"
go WISP_PROMPT="fix it" STUB_WAIT_RC=1
grep -q "^wisp show t-9" "$tmp/log" && grep -q "the report" "$tmp/summary" && echo "ok   the report is written before failing" ||
  { echo "FAIL the report is written before failing"; fails=$((fails + 1)); }

go WISP_FORMULA=ci.task.yml
expect "formula is task-file's alias, with a warning" 0 "::warning::wisp: the formula input is now task-file"
grep -q "^wisp run -f ci.task.yml" "$tmp/log" && echo "ok   formula runs wisp run -f" || { echo "FAIL formula runs wisp run -f"; fails=$((fails + 1)); }
go WISP_TASK_FILE=ci.task.yml
expect "task-file runs" 0
grep -q "^wisp run -f ci.task.yml" "$tmp/log" && echo "ok   task-file runs wisp run -f" || { echo "FAIL task-file runs wisp run -f"; fails=$((fails + 1)); }
go WISP_TASK_FILE=ci.task.yml STUB_RUN_FAIL="ci.task.yml has no task"
expect "a launch that fails says wisp's words" 1 "::error::wisp: ✗ ci.task.yml has no task"

go WISP_CHECK=validate
expect "check validate passes" 0 "3 resources"
go WISP_CHECK=validate STUB_VALIDATE_RC=1
expect "check validate fails on problems" 1 "::error::wisp: the setup has problems"
go WISP_CHECK=plan
expect "check plan posts the plan" 0 "agent/critic"
grep -q "agent/critic" "$tmp/summary" && echo "ok   the plan is in the summary" || { echo "FAIL the plan is in the summary"; fails=$((fails + 1)); }
go WISP_CHECK=plan STUB_VALIDATE_RC=1
expect "check plan fails on missing secrets" 1
go WISP_CHECK=nope
expect "an unknown check is refused" 2

# deploy (ADR-0148): a hub's folder planned or deployed with the key, never without one
go WISP_DEPLOY=plan WISP_HUB_DIR=hub WISP_HUB_KEY=wgt_k
expect "deploy plan runs wisp grid plan with the key" 0 "key=wgt_k plan hub --yes"
grep -q "wisp hub plan: planned" "$tmp/summary" && echo "ok   the hub plan is in the summary" || { echo "FAIL the hub plan is in the summary"; fails=$((fails + 1)); }
go WISP_DEPLOY=apply WISP_HUB_DIR=hub WISP_HUB_KEY=wgt_k
expect "deploy apply runs wisp grid deploy" 0 "key=wgt_k deploy hub --yes"
go WISP_DEPLOY=apply WISP_HUB_KEY=wgt_k STUB_GRID_RC=1
expect "a failed deploy fails the job" 1 "::error::wisp: the hub deploy failed"
go WISP_DEPLOY=apply
expect "deploy without a key says so" 2 "deploy needs hub-key"
go WISP_DEPLOY=ship WISP_HUB_KEY=wgt_k
expect "deploy is plan or apply" 2 "deploy is plan or apply"

[ "$fails" -eq 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
