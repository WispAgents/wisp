#!/bin/sh
# The boxes' harnesses, kept up to date (wisp-grid-harnesses.timer, daily). A box can't update its own: the CLIs
# live in the image (/usr/local, /opt/agents), read-only for the box's user. So this rebuilds wisp-grid-user with
# today's harnesses (npm's @latest Claude Code, Codex and Copilot; Cursor, Grok and Antigravity from their install
# scripts; the newest opencode release) and the same wisp version the live image has, runs every CLI in it under
# gVisor as a box does, and only then makes it the image boxes start from. A box moves to it when it next starts
# (boxes idle-stop after 10 minutes), so a running task is never cut.
#
# Runs as root on the grid host, from the deploy folder's copy (/opt/wisp-grid/deploy). By hand:
#   systemctl start wisp-grid-harnesses && journalctl -u wisp-grid-harnesses -f
set -eu

IMAGE=wisp-grid-user
DEPLOY=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# the build context the Dockerfile expects (repo-relative COPY paths), from the deploy folder alone
CTX=$(mktemp -d /tmp/wisp-harnesses.XXXXXX)
HOME_DIR=$(mktemp -d /tmp/wisp-harnesses-home.XXXXXX)
trap 'rm -rf "$CTX" "$HOME_DIR"' EXIT
mkdir -p "$CTX/apps/grid"
cp -r "$DEPLOY" "$CTX/apps/grid/deploy"

# the wisp the boxes run now: a harness refresh never changes it (a wisp release goes out through deploy.sh)
WISP=$(docker run --rm --entrypoint wisp "$IMAGE" --version | awk '{print $NF}')

# the newest opencode release (vX.Y.Z, not a pre-release, nor a side project's tag like process-daemon-v0.6.0)
OPENCODE=$(curl -fsSL "https://api.github.com/repos/anomalyco/opencode/releases?per_page=40" | python3 -c '
import json, re, sys
tags = [r["tag_name"][1:] for r in json.load(sys.stdin) if not r["prerelease"] and not r["draft"] and re.fullmatch(r"v\d+\.\d+\.\d+", r["tag_name"])]
print(max(tags, key=lambda t: tuple(int(x) for x in t.split("."))))')

echo "refreshing harnesses: wisp $WISP, opencode $OPENCODE"
docker build -q \
  --build-arg "WISP_VERSION=$WISP" \
  --build-arg "OPENCODE_VERSION=$OPENCODE" \
  --build-arg "HARNESSES_AT=$(date -u +%F-%H%M)" \
  -f "$CTX/apps/grid/deploy/image/Dockerfile" -t "$IMAGE:next" "$CTX" >/dev/null

# as a box runs it: gVisor, uid 1000, an empty home. Any CLI that doesn't start keeps the old image.
chown 1000:1000 "$HOME_DIR"
if ! VERSIONS=$(docker run --rm --runtime=runsc -v "$HOME_DIR:/home/wisp" "$IMAGE:next" sh -c '
  set -e
  echo "wisp $(wisp --version)"
  echo "claude $(claude --version)"
  echo "codex $(codex --version)"
  echo "copilot $(copilot --version | head -1)"
  echo "opencode $(opencode --version)"
  echo "cursor $(cursor-agent --version)"
  echo "grok $(grok --version)"
  echo "agy $(agy --version)"
  echo "gh $(gh --version | head -1)"' 2>&1); then
  echo "a CLI didn't start in the new image; boxes keep the current one:" >&2
  echo "$VERSIONS" >&2
  docker rmi "$IMAGE:next" >/dev/null 2>&1 || true
  exit 1
fi
echo "$VERSIONS"

docker tag "$IMAGE:next" "$IMAGE"
docker rmi "$IMAGE:next" >/dev/null
docker image prune -f >/dev/null
echo "boxes start on the new image from their next start"
