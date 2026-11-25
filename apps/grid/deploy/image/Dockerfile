# One user's wisp (v2), as wisp Grid's boxd runs it (apps/grid/boxd/src/run.rs container_run).
# Build from the repo root:  docker build -f apps/grid/deploy/image/Dockerfile -t wisp-grid-user .
FROM node:22-bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates curl ripgrep python3 openssh-client procps less \
 && rm -rf /var/lib/apt/lists/*

# GitHub CLI: Ship it pushes and opens pull requests with it; each person signs in from Settings → Agents.
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
 && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list \
 && apt-get update && apt-get install -y --no-install-recommends gh && rm -rf /var/lib/apt/lists/*

# The agents. Each person signs in to theirs inside their own wisp (or uses opencode's free models).
# HARNESSES_AT busts Docker's cache from here down: without a new value, a rebuild reuses the last layers and keeps
# old harnesses (the image of 2026-11-25 still had Claude Code 2.1.306 from weeks before). Pass the build's date:
#   docker build --build-arg HARNESSES_AT=$(date +%F) …
ARG HARNESSES_AT=2026-11-25
RUN echo "harnesses as of ${HARNESSES_AT}" \
 && npm install -g @anthropic-ai/claude-code@latest @openai/codex@latest @github/copilot@latest && npm cache clean --force \
 && claude --version && codex --version && copilot --version

# opencode v2 ships only as GitHub release binaries: npm's `opencode-ai` is still the 1.x line. A version, never
# GitHub's "latest" (it can point back at a 1.x patch release): refresh-harnesses.sh passes the newest one each day.
# Checked against the release's SHA256SUMS. v2 prints `opencode v2.4.1` (v1 printed a bare `1.19.3`).
ARG OPENCODE_VERSION=2.4.1
RUN arch="$(dpkg --print-architecture | sed 's/amd64/x64/')" && f="opencode-linux-${arch}.tar.gz" && cd /tmp \
 && curl -fsSLO "https://github.com/anomalyco/opencode/releases/download/v${OPENCODE_VERSION}/${f}" \
 && curl -fsSL "https://github.com/anomalyco/opencode/releases/download/v${OPENCODE_VERSION}/SHA256SUMS" | grep "  ${f}\$" | sha256sum -c - \
 && tar xzf "${f}" -C /usr/local/bin opencode && rm "${f}" && chmod 755 /usr/local/bin/opencode \
 && opencode --version | grep -qF "v${OPENCODE_VERSION}"

# Cursor, Grok and Antigravity only ship install scripts that write to $HOME, which is the user's
# own folder at run time. Install them under /opt (read-only for the user) and put them on PATH.
RUN mkdir -p /opt/agents \
 && HOME=/opt/agents bash -c 'curl -fsSL https://cursor.com/install | bash' \
 && HOME=/opt/agents bash -c 'curl -fsSL https://x.ai/cli/install.sh | bash' \
 && HOME=/opt/agents bash -c 'curl -fsSL https://antigravity.google/cli/install.sh | bash -s -- --dir /opt/agents/bin' \
 && ln -sf "$(readlink -f /opt/agents/.local/bin/cursor-agent)" /usr/local/bin/cursor-agent \
 && ln -sf "$(readlink -f /opt/agents/.grok/bin/grok)" /usr/local/bin/grok \
 && ln -sf /opt/agents/bin/agy /usr/local/bin/agy \
 && rm -rf /opt/agents/.cache /opt/agents/.bashrc /opt/agents/.profile \
 && chmod -R a+rX /opt/agents \
 && cursor-agent --version && grok --version && agy --version

# A browser agents drive and you watch (wisp-desktop: Xvfb, Chromium, noVNC on 6080), Playwright's
# MCP server for it (wisp-browser-mcp, the Browser connector in a sandbox), pip, and sudo. sudo only
# works in a sandbox: boxd starts one as root under gVisor (wisp-sandbox-init), whose root helper
# (rootd.mjs) is what this sudo talks to; gVisor doesn't honour setuid binaries. A cloud computer's
# box has no helper, no capabilities and no-new-privileges.
RUN apt-get update \
 && apt-get install -y --no-install-recommends chromium xvfb x11vnc x11-xserver-utils xdotool novnc websockify fonts-liberation fonts-noto-core fonts-wqy-microhei fonts-noto-color-emoji python3-pip python3-venv \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /tmp/.X11-unix && chmod 1777 /tmp/.X11-unix
RUN npm install -g @playwright/mcp && npm cache clean --force && command -v playwright-mcp
# apt and apt-get remember what a sandbox installs (secure_path puts /usr/local/sbin first under sudo).
COPY apps/grid/deploy/image/sandbox/apt apps/grid/deploy/image/sandbox/apt-get /usr/local/sbin/
COPY apps/grid/deploy/image/sandbox/wisp-desktop apps/grid/deploy/image/sandbox/wisp-browser-mcp apps/grid/deploy/image/sandbox/sudo /usr/local/bin/
COPY apps/grid/deploy/image/sandbox/wisp-sandbox-init /usr/local/sbin/
COPY apps/grid/deploy/image/sandbox/rootd.mjs /usr/local/lib/wisp/
COPY apps/grid/deploy/image/sandbox/wisp.html /usr/share/novnc/
# What the agent installs without root lands in the home, which stays: npm -g, pip.
ENV NPM_CONFIG_PREFIX=/home/wisp/.local PIP_USER=1 PIP_BREAK_SYSTEM_PACKAGES=1 PATH=/home/wisp/.local/bin:$PATH

# v2 wisp (the Rust binary, through the npm launcher), on PATH as `wisp`: boxd starts `wisp serve --port <port>` with
# WISP_HOST=0.0.0.0 and WISP_BOX=1. Into /usr/local, not NPM_CONFIG_PREFIX: the user's home is mounted over /home/wisp.
ARG WISP_VERSION=1.0.0-alpha.2
RUN NPM_CONFIG_PREFIX=/usr/local npm install -g "wisp-agents@${WISP_VERSION}" && npm cache clean --force \
 && wisp --version

# uid 1000 (the image's "node" user), home on the user's own folder from the host.
RUN usermod -d /home/wisp node && mkdir -p /home/wisp && chown node:node /home/wisp
USER node
ENV HOME=/home/wisp
WORKDIR /home/wisp
