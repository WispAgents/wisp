#!/usr/bin/env node
// Builds trywisp.dev into docs/: the home page, Get the app, wisp Cloud, a page per harness under agents/,
// changelog and privacy from their markdown, a 404, the old guides' addresses (each sends its readers to
// the docs, which hold the guides now: ADR-0039), sitemap.xml, robots.txt and llms.txt. Every page is generated here:
// never edit the HTML in docs/. The look is docs/site.css, inlined into each page. The docs themselves are apps/docs.
// The Pages workflow runs it before upload.
//
//   node scripts/site.mjs [outDir]     default: docs/

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { MOVED_GUIDES } from './guides.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const DOCS = path.join(ROOT, 'docs');
const SITE = 'https://trywisp.dev';
const REPO = 'https://github.com/WispAgents/wisp';
const VERSION = JSON.parse(fs.readFileSync(path.join(ROOT, 'npm/wisp-agents/package.json'), 'utf8')).version;
// Markdown with \n line ends, whatever the checkout has (Windows checks out CRLF).
const readText = (f) => fs.readFileSync(f, 'utf8').replace(/\r\n?/g, '\n');

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const slug = (s) => s.toLowerCase().replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/<[^>]+>/g, '').replace(/[`*[\]]/g, '').replace(/[^\w\s.-]/g, '').trim().replace(/[\s.]+/g, '-');

// ---------- markdown (the subset integrations.md and CHANGELOG.md use) ----------

// base: where relative links in the file point (its folder on GitHub).
function inline(text, base) {
  const codes = [];
  let s = text.replace(/`([^`]+)`/g, (_, c) => `\u0000${codes.push(`<code>${esc(c)}</code>`) - 1}\u0000`);
  s = esc(s)
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, href) => `<a href="${esc(link(href.replace(/&amp;/g, '&'), base))}">${t}</a>`)
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^\w*])\*(?!\s)([^*]+?)\*(?![\w*])/g, '$1<em>$2</em>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => codes[i]);
}

function link(href, base) {
  if (/^(https?:|mailto:|#)/.test(href)) return href;
  const url = new URL(href, base).href;
  // Pages this site renders itself.
  if (url.startsWith(`${REPO}/blob/main/docs/integrations.md`)) return url.replace(/^.*integrations\.md/, '/integrations.html');
  if (url.startsWith(`${REPO}/blob/main/docs/privacy.md`)) return url.replace(/^.*privacy\.md/, '/privacy.html');
  if (url.startsWith(`${REPO}/blob/main/CHANGELOG.md`)) return url.replace(/^.*CHANGELOG\.md/, '/changelog.html');
  return url;
}

export function markdown(src, base = `${REPO}/blob/main/`) {
  const lines = src.replace(/\r/g, '').split('\n').filter((l) => !/^\[[^\]]+\]:\s+\S/.test(l));
  const out = [];
  const indent = (l) => l.match(/^ */)[0].length;
  const item = /^(\s*)([-*]|\d+\.)\s+(.*)$/;
  const startsBlock = (l) => /^(```|#{1,6}\s|\||>)/.test(l) || item.test(l);

  function list(i, depth) {
    const ordered = /^\s*\d+\./.test(lines[i]);
    const items = [];
    while (i < lines.length) {
      const l = lines[i];
      const m = l.match(item);
      if (m && indent(l) === depth) {
        items.push({ text: m[3], sub: '' });
        i++;
      } else if (m && indent(l) > depth && items.length) {
        const [html, next] = list(i, indent(l));
        items.at(-1).sub += html;
        i = next;
      } else if (l.trim() && indent(l) > depth && items.length) {
        items.at(-1).text += ` ${l.trim()}`; // a wrapped line of the item above
        i++;
      } else break;
    }
    const tag = ordered ? 'ol' : 'ul';
    return [`<${tag}>${items.map((x) => `<li>${inline(x.text, base)}${x.sub}</li>`).join('')}</${tag}>`, i];
  }

  for (let i = 0; i < lines.length; ) {
    const l = lines[i];
    if (!l.trim()) { i++; continue; }
    let m;
    if ((m = l.match(/^```(\w*)/))) {
      const body = [];
      for (i++; i < lines.length && !/^```/.test(lines[i]); i++) body.push(lines[i]);
      i++;
      out.push(`<pre${m[1] ? ` data-lang="${m[1]}"` : ''}><code>${esc(body.join('\n'))}</code></pre>`);
    } else if ((m = l.match(/^(#{1,6})\s+(.*)$/))) {
      const n = m[1].length;
      out.push(`<h${n} id="${slug(m[2])}">${inline(m[2], base)}</h${n}>`);
      i++;
    } else if (l.startsWith('|')) {
      const rows = [];
      for (; i < lines.length && lines[i].startsWith('|'); i++) rows.push(lines[i]);
      const cells = (r) => r.replace(/^\||\|$/g, '').split(/(?<!\\)\|/).map((c) => inline(c.trim().replace(/\\\|/g, '|'), base));
      const [head, , ...body] = rows;
      out.push(`<div class="table"><table><thead><tr>${cells(head).map((c) => `<th>${c}</th>`).join('')}</tr></thead><tbody>${body.map((r) => `<tr>${cells(r).map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table></div>`);
    } else if (l.startsWith('>')) {
      const body = [];
      for (; i < lines.length && lines[i].startsWith('>'); i++) body.push(lines[i].replace(/^>\s?/, ''));
      out.push(`<blockquote>${markdown(body.join('\n'), base)}</blockquote>`);
    } else if (item.test(l)) {
      const [html, next] = list(i, indent(l));
      out.push(html);
      i = next;
    } else {
      const body = [];
      for (; i < lines.length && lines[i].trim() && (!body.length || !startsBlock(lines[i])); i++) body.push(lines[i].trim());
      out.push(`<p>${inline(body.join(' '), base)}</p>`);
    }
  }
  return out.join('\n');
}

// ---------- the harnesses ----------
// What wisp passes each CLI (plugins/<id>/host). Keep in step when a runner changes.

const MODES = { plan: 'Plan (read-only)', edits: 'Edits', auto: 'Full auto' };

export const AGENTS = [
  {
    id: 'claude', slug: 'claude-code', name: 'Claude Code', vendor: 'Anthropic', bin: 'claude', logo: 'claudecode-color.svg',
    docs: 'https://claude.com/claude-code',
    lede: 'Hand Claude Code a task and close the tab. Wisp Agents runs it headless in its own git worktree, routes its questions and permission prompts to you, and hands back a one-screen report with the real diff and the test run.',
    run: 'claude -p --output-format stream-json \\\n  --permission-mode acceptEdits \\\n  --append-system-prompt "<wisp protocol>" \\\n  --mcp-config \'{"mcpServers":{"wisp":…}}\' --verbose',
    modes: { plan: '--permission-mode plan', edits: '--permission-mode acceptEdits', auto: '--permission-mode bypassPermissions' },
    ask: 'An MCP tool, <code>mcp__wisp__ask_user</code>, added with <code>--mcp-config</code>. The run pauses, you get a notification, and your answer goes straight back into the same session.',
    approve: 'In Edits mode, Claude Code’s permission prompts go to Wisp Agents’ <code>approve</code> tool (<code>--permission-prompt-tool</code>). The card asks <em>Allow Claude Code to run <code>npm publish</code>?</em> with <b>Allow</b>, <b>Always allow in this task</b> and <b>Deny</b>.',
    btw: 'answered on a copy of the conversation (<code>--resume &lt;id&gt; --fork-session</code>), so it knows what the run did and why',
    resume: 'claude --resume <id>',
    models: 'Opus, Sonnet or Haiku (or whatever Claude Code is set to), plus a thinking level passed as <code>--effort</code>.',
    extras: [
      'Several folders in one task: the others are passed as <code>--add-dir</code>.',
      'Claude in Chrome: when it is on by default in your Claude settings, Wisp Agents adds <code>--chrome</code>, which headless runs need.',
      'Delegate: a Claude Code lead can hand work to Codex, opencode or another Claude, here or on your other machines, through Wisp Agents’ <code>delegate</code> tool.',
    ],
  },
  {
    id: 'codex', slug: 'codex', name: 'Codex', vendor: 'OpenAI', bin: 'codex', logo: 'codex-color.svg',
    docs: 'https://github.com/openai/codex',
    lede: 'Run OpenAI’s Codex CLI on real tasks without watching it. Wisp Agents starts <code>codex exec</code> in a fresh worktree, keeps the sandbox you pick, and turns the JSON event stream into a readable report: what changed, what ran, what passed.',
    run: 'codex exec --json --skip-git-repo-check \\\n  -c sandbox_mode="workspace-write" \\\n  -c mcp_servers.wisp.command=…',
    modes: { plan: '-c sandbox_mode="read-only"', edits: '-c sandbox_mode="workspace-write"', auto: '--dangerously-bypass-approvals-and-sandbox' },
    ask: 'An MCP tool, <code>mcp__wisp__ask_user</code>, configured for the run. Codex pauses on it, and your answer comes back as the tool result.',
    approve: 'Codex has no “accept edits only” mode, so Edits uses the <code>workspace-write</code> sandbox: it edits the project and runs sandboxed commands. Anything the sandbox refuses shows up on the card with <b>Allow &amp; retry</b>.',
    btw: 'answered on a fork of the thread (<code>codex exec fork &lt;id&gt;</code>), so it knows what the run did and why',
    resume: 'codex resume <id>',
    models: 'Any model your Codex CLI can use, plus a reasoning level passed as <code>-c model_reasoning_effort</code>.',
    extras: [
      'Several folders in one task: <code>--add-dir</code> on the first turn, writable roots in the sandbox config after that.',
      'Pasted screenshots go in as images.',
      'Compare: run the same task on Codex and Claude Code side by side and keep the better branch.',
    ],
  },
  {
    id: 'cursor', slug: 'cursor', name: 'Cursor CLI', vendor: 'Cursor', bin: 'cursor-agent', logo: 'cursor.svg', mono: true,
    docs: 'https://cursor.com/cli',
    lede: 'Use the Cursor agent outside the editor. Wisp Agents drives <code>cursor-agent</code> in print mode on its own branch, reads its stream of tool calls, and gives you a report you can ship from, with any model your Cursor plan includes.',
    run: 'cursor-agent -p --output-format stream-json --trust \\\n  [--mode plan | --force] [--model <id>]',
    modes: { plan: '--mode plan', edits: '(its own permissions)', auto: '--force' },
    ask: 'Cursor CLI can’t load extra MCP servers per run, so Wisp Agents asks it to end the turn with a question block. The card shows the options; your pick resumes the session.',
    approve: 'Without <code>--force</code>, calls Cursor’s permissions refuse come back on the card with <b>Allow &amp; retry</b>.',
    btw: 'answered from Wisp Agents’ record of the task (requests, reports, actions and files changed), since Cursor CLI can’t fork a chat',
    resume: 'cursor-agent --resume <id>',
    models: 'The list from <code>cursor-agent --list-models</code>, so new models show up without a Wisp Agents update.',
    extras: [
      'Prompts go in on stdin, so long tasks and pasted logs aren’t cut by the shell.',
      'Subagent calls show on the card as <em>Delegating</em>.',
    ],
  },
  {
    id: 'opencode', slug: 'opencode', name: 'opencode', vendor: 'SST', bin: 'opencode', logo: 'opencode.svg', mono: true,
    docs: 'https://opencode.ai',
    lede: 'Point opencode at any provider and let it work in the background. Wisp Agents runs <code>opencode run</code> with the model and variant you pick, gives it a way to ask you questions, and turns the session into a short report.',
    run: 'opencode run --format json --print-logs --log-level ERROR \\\n  [--agent plan | --auto] [--model provider/model --variant <effort>]',
    modes: { plan: '--agent plan', edits: '(its own permissions)', auto: '--auto' },
    ask: 'An MCP tool, <code>wisp_ask_user</code>, added through opencode’s inline config, with a long MCP timeout so it can wait for you.',
    approve: 'Calls opencode refuses come back on the card with <b>Allow &amp; retry</b>, which re-runs the turn with just that call allowed.',
    btw: 'answered on a fork of the session (<code>--session &lt;id&gt; --fork</code>), so it knows what the run did and why',
    resume: 'opencode --session <id>',
    models: 'Every <code>provider/model</code> from <code>opencode models</code>, with each model’s variants as the thinking level.',
    extras: [
      'Attachments go in with <code>--file</code>.',
      'When a free model is rate-limited and opencode keeps retrying in silence, Wisp Agents stops waiting and says so on the card.',
      'If opencode isn’t on your PATH, Wisp Agents can install its own copy from the launcher.',
    ],
  },
  {
    id: 'antigravity', slug: 'antigravity', name: 'Antigravity CLI', vendor: 'Google', bin: 'agy', logo: 'antigravity-color.svg',
    docs: 'https://antigravity.google/docs/cli/headless/',
    lede: 'Run Google’s Antigravity agent headless on long tasks. Wisp Agents drives <code>agy</code> over stream-json with a long print timeout, so it isn’t cut off at five minutes, and turns its steps into a report.',
    run: 'agy --input-format stream-json --output-format stream-json \\\n  --print-timeout <long> --mode accept-edits',
    modes: { plan: '--mode plan', edits: '--mode accept-edits', auto: '--dangerously-skip-permissions' },
    ask: 'Wisp Agents asks it to end the turn with a question block. The card shows the options; your pick resumes the conversation.',
    approve: 'In headless mode shell commands are governed by Antigravity’s permissions; refused ones come back on the card with <b>Allow &amp; retry</b>.',
    btw: 'answered from Wisp Agents’ record of the task (requests, reports, actions and files changed)',
    resume: 'agy --conversation <id>',
    models: 'The list from <code>agy models</code>, plus a thinking level passed as <code>--effort</code>.',
    extras: [
      'Attachment folders are added with <code>--add-dir</code>.',
      'Browser steps and delegation to its own subagents show on the card as they happen.',
    ],
  },
  {
    id: 'grok', slug: 'grok', name: 'Grok Build', vendor: 'xAI', bin: 'grok', logo: 'grok.svg', mono: true,
    docs: 'https://x.ai/cli',
    lede: 'Put xAI’s Grok Build CLI to work in the background. Wisp Agents runs <code>grok</code> with your permission mode, gives it Wisp Agents’ tools through a throwaway agent file, and hands you the report when it’s done.',
    run: 'grok --prompt-file <task> --output-format streaming-messages-json \\\n  --permission-mode acceptEdits --rules "<wisp protocol>" \\\n  --agent <file with wisp’s MCP server> --no-auto-update',
    modes: { plan: '--permission-mode plan', edits: '--permission-mode acceptEdits', auto: '--permission-mode bypassPermissions' },
    ask: 'An MCP tool, reached as <code>wisp__ask_user</code>. Each turn writes an agent file that keeps Grok’s stock prompt and only adds Wisp Agents’ server.',
    approve: 'Calls its permission mode refuses come back on the card with <b>Allow &amp; retry</b>.',
    btw: 'answered from Wisp Agents’ record of the task (requests, reports, actions and files changed)',
    resume: 'grok --resume <id>',
    models: 'The list from <code>grok models</code>, with each model’s reasoning levels as <code>--reasoning-effort</code>.',
    extras: [
      'Grok’s own question tool is turned off so questions come to you through Wisp Agents.',
      'Auto-updates are off during a run, so a task never stalls on one.',
    ],
  },
  {
    id: 'copilot', slug: 'copilot', name: 'Copilot CLI', vendor: 'GitHub', bin: 'copilot', logo: 'githubcopilot.svg', mono: true,
    docs: 'https://docs.github.com/copilot/how-tos/copilot-cli',
    lede: 'Put the GitHub Copilot CLI to work in the background, with any model your Copilot plan includes. Wisp Agents hands <code>copilot</code> the task on stdin, adds Wisp Agents’ tools for the run, and turns its JSON event stream into a report you can ship from.',
    run: 'copilot --output-format json --allow-tool=write \\\n  --session-id <uuid> --no-ask-user \\\n  --additional-mcp-config \'{"mcpServers":{"wisp":…}}\' < task',
    modes: { plan: '--mode plan', edits: '--allow-tool=write', auto: '--allow-all' },
    ask: 'An MCP tool, reached as <code>wisp-ask_user</code>, added with <code>--additional-mcp-config</code> and allowed with <code>--allow-tool=wisp</code>. Copilot’s own question tool is off (<code>--no-ask-user</code>), so its questions come to you.',
    approve: 'Copilot can’t hand a permission prompt to Wisp Agents, so in Edits mode the commands its permissions refuse come back on the card with <b>Allow &amp; retry</b>, which re-runs the turn with exactly those commands allowed (<code>--allow-tool=shell(npm publish)</code>).',
    btw: 'answered from Wisp Agents’ record of the task (requests, reports, actions and files changed)',
    resume: 'copilot --resume=<id>',
    models: 'The models <code>copilot help config</code> lists (Claude, GPT, Gemini, Grok…), plus a thinking level passed as <code>--reasoning-effort</code>.',
    extras: [
      'The task goes in on stdin, so long prompts and pasted logs aren’t cut by the shell.',
      'Several folders in one task: the others are passed as <code>--add-dir</code>. Pasted screenshots go in with <code>--attachment</code>.',
      'Delegate: a Copilot lead hands work through Wisp Agents’ <code>delegate</code> tool, with its own <code>task</code> subagents turned off.',
    ],
  },
];

// ---------- page shell ----------

const CSS = minifyCss(readText(path.join(DOCS, 'site.css')));
function minifyCss(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, ' ').replace(/\s*([{};,>])\s*/g, '$1').replace(/:\s+/g, ':').replace(/;}/g, '}').trim();
}

const GH_ICON = '<svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>';
const COPY_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 0 1 2-2h9"/></svg>';
const DL_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v11M7 10.5l5 5 5-5M5 20h14"/></svg>';
const PHONE_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M11 18.5h2"/></svg>';
const ARROW = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg>';
const MARK = '<img src="/assets/favicon-48.png" alt="" width="26" height="26" />';

const DOCS_URL = '/docs/';
const DL = 'https://github.com/WispAgents/wisp/releases/latest/download';
const GRID = 'https://grid.trywisp.dev';

// Usage counts (docs/privacy.md, "This site"): PostHog, loaded on the reader's first scroll, tap or key, or 4 s after the
// page loaded, so its script never competes with the first paint or the first input (it was all of the home page's
// blocking time on a slow phone). The waitlist form sends nothing to it.
const ANALYTICS = `<script>
(function () { var done = false, evs = ['scroll', 'pointerdown', 'keydown', 'touchstart'];
function go() { if (done) return; done = true; evs.forEach(function (e) { removeEventListener(e, go); });
!function(t,e){var o,n,p,r;e.__SV||(window.posthog&&window.posthog.__loaded)||(window.posthog=e,e._i=[],e.init=function(i,s,a){function g(t,e){var o=e.split(".");2==o.length&&(t=t[o[0]],e=o[1]),t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)))}}p||((p=t.createElement("script")).type="text/javascript",p.crossOrigin="anonymous",p.async=!0,p.src=s.api_host.replace(".i.posthog.com","-assets.i.posthog.com")+"/static/array.js",p.onerror=function(){p=null},(r=t.getElementsByTagName("script")[0]).parentNode.insertBefore(p,r));var u=e;for(void 0!==a?u=e[a]=[]:a="posthog",u.people=u.people||[],o="init capture register register_once identify reset opt_in_capturing opt_out_capturing".split(" "),n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a])},e.__SV=1)}(document,window.posthog||[]);
posthog.init('phc_wowwZkbP8e4zYuwT67JtX6oxdPwkMWL5kKnrUmYYXhcD', { api_host: 'https://us.i.posthog.com', defaults: '2026-05-30', person_profiles: 'identified_only' });
}
evs.forEach(function (e) { addEventListener(e, go, { once: true, passive: true }); });
addEventListener('load', function () { setTimeout(go, 4000); }); })();
</script>`;

// The page's few behaviours, in one small script at the end: copy buttons, the Desktop app · Phone app · TUI · CLI switch (one choice for
// the whole site, kept), and the phone app waitlist.
const PAGE_JS = `<script>
(function () {
  var d = document, root = d.documentElement, WAY_KEY = 'starlight-synced-tabs__way';
  var LABEL = { desktop: 'Desktop app', phone: 'Phone app', tui: 'TUI', cli: 'CLI' };
  function track(e, p) { try { window.posthog && posthog.capture && posthog.capture(e, p); } catch (x) {} }
  d.addEventListener('click', function (ev) {
    var c = ev.target.closest('[data-copy]');
    if (c) { navigator.clipboard && navigator.clipboard.writeText(c.getAttribute('data-copy')).then(function () { c.setAttribute('data-done', ''); c.setAttribute('aria-label', 'Copied'); setTimeout(function () { c.removeAttribute('data-done'); c.setAttribute('aria-label', 'Copy command'); }, 1600); }); track('site copy', { what: c.getAttribute('data-copy') }); return; }
    var w = ev.target.closest('[data-way-pick]');
    if (w) { setWay(w.getAttribute('data-way-pick')); try { localStorage.setItem(WAY_KEY, LABEL[root.dataset.way]); } catch (x) {} return; }
    var a = ev.target.closest('a[data-dl]');
    if (a) track('site download', { what: a.getAttribute('data-dl') });
  });
  function setWay(k) {
    root.dataset.way = k;
    d.querySelectorAll('[data-way-pick]').forEach(function (b) { var on = b.getAttribute('data-way-pick') === k; b.setAttribute('aria-selected', on); b.tabIndex = on ? 0 : -1; });
  }
  d.addEventListener('keydown', function (ev) {
    var b = ev.target.closest && ev.target.closest('[data-way-pick]');
    if (!b || (ev.key !== 'ArrowRight' && ev.key !== 'ArrowLeft')) return;
    var all = [].slice.call(b.parentNode.querySelectorAll('[data-way-pick]')), i = all.indexOf(b);
    var n = all[(i + (ev.key === 'ArrowRight' ? 1 : all.length - 1)) % all.length]; n.click(); n.focus(); ev.preventDefault();
  });
  // the head's script already chose (the reader's pick, else Desktop app, Phone app on a phone): the tabs' state follows
  setWay(root.dataset.way || 'desktop');

  // the phone app waitlist: the email goes to wisp Cloud's list only (docs/privacy.md)
  var EMAIL = /^[^@\\s<>"']{1,64}@[^@\\s<>"']+\\.[^@\\s<>"']{2,}$/;
  d.querySelectorAll('form[data-waitlist]').forEach(function (f) {
    var input = f.querySelector('input[type=email]'), msg = f.querySelector('.msg'), btn = f.querySelector('button');
    function say(t, err) { msg.textContent = t; msg.classList.toggle('err', !!err); }
    input.addEventListener('input', function () { input.removeAttribute('aria-invalid'); say(''); });
    f.addEventListener('submit', function (ev) {
      ev.preventDefault();
      var email = input.value.trim();
      if (!EMAIL.test(email) || email.length > 254) { input.setAttribute('aria-invalid', 'true'); say('That doesn’t look like an email address.', true); input.focus(); return; }
      var pick = f.querySelector('input[name=platform]:checked'), platform = pick ? pick.value : 'both';
      btn.disabled = true; say('Adding you…');
      var body = new URLSearchParams({ email: email, kind: 'phone-app', platform: platform, source: 'site' });
      fetch(f.action, { method: 'POST', body: body }).then(function (r) {
        return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, status: r.status, j: j }; });
      }).then(function (r) {
        btn.disabled = false;
        if (r.ok) {
          var phone = { ios: 'iPhone', android: 'Android', both: 'iPhone and Android' }[platform];
          var t = d.createElement('div'); t.className = 'thanks ph-no-capture'; t.setAttribute('role', 'status'); t.tabIndex = -1;
          t.innerHTML = '<b>You’re on the list.</b> We’ll email <span></span> once, when Wisp Agents is out for ' + phone + '. Nothing else.';
          t.querySelector('span').textContent = email;
          f.replaceWith(t); t.focus();
        } else if (r.status === 429) say('Too many tries from here. Wait a minute, then try again.', true);
        else say((r.j && r.j.error) || 'That didn’t work. Try again in a moment.', true);
      }, function () { btn.disabled = false; say('Couldn’t reach the list. Try again, or email hello@trywisp.dev.', true); });
    });
  });
})();
</script>`;

// Before paint (no flash, no shift): which how-to tab every block shows. The reader's last pick, shared with the docs'
// synced tabs (same key, same labels); else Phone app on a phone (touch and narrow, or a mobile browser), else Desktop
// app, which is also what the HTML says for a reader without JavaScript.
const WAY_HEAD = `<script>try{var r=document.documentElement,l=localStorage.getItem('starlight-synced-tabs__way'),k={'Desktop app':'desktop','Phone app':'phone','TUI':'tui','CLI':'cli'}[l];if(!k){var n=navigator,m=n.userAgentData&&n.userAgentData.mobile;k=m||matchMedia('(max-width: 700px) and (pointer: coarse)').matches||/iPhone|iPod|Android.+Mobile/.test(n.userAgent)?'phone':'desktop'}r.dataset.way=k}catch(e){}</script>`;

const NAV = [
  ['Docs', DOCS_URL, ''],
  ['Guides', '/docs/guides/', 'opt'],
  ['Agents', '/agents/', 'opt'],
  ['Cloud', '/cloud.html', ''],
  ['Changelog', '/changelog.html', 'opt'],
];

function page({ path: p, title, description, crumbs = [], body, schema = [], noindex = false, image = '/assets/og.png', imageAlt = 'Wisp Agents: manage the code your agents write', head = '', type = 'website', main = 'doc' }) {
  const url = `${SITE}${p.replace(/index\.html$/, '')}`;
  const trail = [['Wisp Agents', '/'], ...crumbs];
  const breadcrumb = {
    '@type': 'BreadcrumbList',
    itemListElement: trail.map(([name, at], i) => ({ '@type': 'ListItem', position: i + 1, name, item: `${SITE}${at}` })),
  };
  const graph = noindex ? [] : [...(crumbs.length ? [breadcrumb] : []), ...schema];
  return `<!doctype html>
<html lang="en" data-way="desktop">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
${WAY_HEAD}
<title>${esc(title)}</title>
<meta name="description" content="${esc(description)}" />
${noindex ? '<meta name="robots" content="noindex" />' : `<link rel="canonical" href="${url}" />
<meta name="robots" content="index, follow, max-image-preview:large" />`}
<meta name="color-scheme" content="light dark" />
<meta name="theme-color" content="#f7f6f3" media="(prefers-color-scheme: light)" />
<meta name="theme-color" content="#0d0d0f" media="(prefers-color-scheme: dark)" />
<meta property="og:type" content="${type}" />
<meta property="og:locale" content="en_US" />
<meta property="og:site_name" content="Wisp Agents" />
<meta property="og:url" content="${url}" />
<meta property="og:title" content="${esc(title)}" />
<meta property="og:description" content="${esc(description)}" />
<meta property="og:image" content="${SITE}${image}" />
<meta property="og:image:width" content="1200" />
<meta property="og:image:height" content="630" />
<meta property="og:image:alt" content="${esc(imageAlt)}" />
<meta name="twitter:card" content="summary_large_image" />
<meta name="twitter:site" content="@trywispagents" />
<meta name="twitter:title" content="${esc(title)}" />
<meta name="twitter:description" content="${esc(description)}" />
<meta name="twitter:image" content="${SITE}${image}" />
<meta name="twitter:image:alt" content="${esc(imageAlt)}" />
<link rel="icon" href="/favicon.ico" sizes="48x48" />
<link rel="icon" href="/assets/icon.svg" type="image/svg+xml" />
<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png" sizes="180x180" />
<link rel="sitemap" type="application/xml" href="/sitemap.xml" />
<link rel="alternate" type="text/plain" href="/llms.txt" title="llms.txt" />
${graph.length ? `<script type="application/ld+json">${JSON.stringify({ '@context': 'https://schema.org', '@graph': graph })}</script>\n` : ''}<style>${CSS}</style>
${head}</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="nav">
  <div class="wrap nav-in">
    <a class="brand" href="/" aria-label="Wisp Agents, home">${MARK}Wisp Agents</a>
    <span class="alpha" title="Wisp Agents is in alpha: expect rough edges">alpha</span>
    <nav class="links" aria-label="Main">
      ${NAV.map(([t, h, c]) => `<a${c ? ` class="${c}"` : ''} href="${h}"${p === h || (h !== '/' && h.length > 1 && p.startsWith(h) && h !== DOCS_URL) ? ' aria-current="page"' : ''}>${t}</a>`).join('\n      ')}
    </nav>
    <a class="gh" href="${REPO}" rel="noopener">${GH_ICON}<span>GitHub</span></a>
    <a class="btn primary" href="/app.html">Get the app</a>
  </div>
</header>
<main id="main" class="${main}">
${crumbs.length ? `<div class="wrap"><nav class="crumbs" aria-label="Breadcrumb"><ol>${trail.map(([name, at], i) => (i < trail.length - 1 ? `<li><a href="${at}">${esc(name)}</a></li>` : `<li aria-current="page">${esc(name)}</li>`)).join('')}</ol></nav></div>\n` : ''}${body}
</main>
<footer class="foot">
  <div class="wrap foot-in">
    <div>
      <a class="brand" href="/">${MARK}Wisp Agents</a>
      <p class="muted small" style="margin-top:10px">Manage the code your agents write.<br />Open source, MIT. Runs on your machines.</p>
    </div>
    <div><h2>Product</h2><ul>
      <li><a href="/app.html">Desktop and phone apps</a></li>
      <li><a href="/cloud.html">Wisp Agents Cloud &amp; Grid</a></li>
      <li><a href="/agents/">Agents</a></li>
      <li><a href="/changelog.html">Changelog</a></li>
    </ul></div>
    <div><h2>Learn</h2><ul>
      <li><a href="/docs/">Docs</a></li>
      <li><a href="/docs/guides/">Guides</a></li>
      <li><a href="/docs/reference/cli/">CLI reference</a></li>
      <li><a href="/docs/integrations/your-agent/">Integrations</a></li>
    </ul></div>
    <div><h2>More</h2><ul>
      <li><a href="${REPO}" rel="noopener">GitHub</a></li>
      <li><a href="https://www.npmjs.com/package/wisp-agents" rel="noopener">npm</a></li>
      <li><a href="https://x.com/trywispagents" rel="noopener">X</a></li>
      <li><a href="/privacy.html">Privacy</a></li>
    </ul></div>
  </div>
</footer>
${PAGE_JS}
${ANALYTICS}
</body>
</html>
`;
}

// ---------- pieces ----------

/** A real screenshot from the app, light or dark as the reader's system is, AVIF with WebP for older browsers. */
function shot(name, widths, ratio, alt, { sizes = '100vw', eager = false } = {}) {
  const set = (s, ext) => widths.map((w) => `/assets/app/${name}-${s}-${w}.${ext} ${w}w`).join(', ');
  const [w, h] = ratio;
  return `<picture>
  <source media="(prefers-color-scheme: dark)" type="image/avif" srcset="${set('dark', 'avif')}" sizes="${sizes}" />
  <source media="(prefers-color-scheme: dark)" type="image/webp" srcset="${set('dark', 'webp')}" sizes="${sizes}" />
  <source type="image/avif" srcset="${set('light', 'avif')}" sizes="${sizes}" />
  <img src="/assets/app/${name}-light-${widths[1] ?? widths[0]}.webp" srcset="${set('light', 'webp')}" sizes="${sizes}" width="${w}" height="${h}" alt="${esc(alt)}"${eager ? ' fetchpriority="high"' : ' loading="lazy" decoding="async"'} />
</picture>`;
}
const DESK = [640, 960, 1440, 2160];
const PHONE = [240, 390, 780];
const DESK_3 = new Set(['desktop-org', 'desktop-new-task', 'desktop-machines']);
const deskShot = (name, alt, o) => `<div class="window"><div class="bar" aria-hidden="true"><i></i><i></i><i></i></div>${shot(name, DESK_3.has(name) ? [640, 960, 1440] : DESK, [1440, 900], alt, o)}</div>`;
const phoneShot = (name, alt, o) => `<div class="phone">${shot(name, PHONE, [390, 844], alt, o)}</div>`;

const wayShot = (html, phone = false) => `<div class="way-shot${phone ? ' is-phone' : ''}">${html}</div>`;
/** Where install is shown: wisp is in alpha, and where to say what broke. */
const ALPHA_NOTE = `<p class="muted small alpha-note">Wisp Agents is in alpha: expect rough edges, and <a href="${REPO}/issues">tell us what breaks</a>.</p>`;
const install = (cmd = 'npx wisp-agents') =>
  `<div class="install"><span class="dollar" aria-hidden="true">$</span><code>${esc(cmd)}</code><button class="copy" type="button" data-copy="${esc(cmd)}" aria-label="Copy command">${COPY_ICON}</button></div>`;

/** One task four ways: the desktop app, the phone app, the terminal UI and the command line. `desktop`, `phone`,
 * `tui`: HTML; `cli`: a shell snippet. The pick is one for the whole site and the docs: WAYS' labels are what
 * trywisp.dev/docs's synced tabs keep in localStorage (`starlight-synced-tabs__way`), and the head's script draws the
 * right one before paint (Desktop app with no pick, Phone app on a phone). */
const WAYS = [['desktop', 'Desktop app'], ['phone', 'Phone app'], ['tui', 'TUI'], ['cli', 'CLI']];
let waysN = 0;
function ways({ desktop, phone, tui, cli, label = '' }) {
  const n = ++waysN;
  const html = { desktop, phone, tui, cli: `<pre><code>${cli}</code></pre>` };
  const tab = ([k, t]) => `<button type="button" role="tab" id="w${n}-${k}" aria-controls="w${n}-${k}p" data-way-pick="${k}" aria-selected="${k === 'desktop'}"${k === 'desktop' ? '' : ' tabindex="-1"'}>${t}</button>`;
  const panel = ([k]) => `<div class="way" data-k="${k}" role="tabpanel" id="w${n}-${k}p" aria-labelledby="w${n}-${k}">${html[k]}</div>`;
  return `<div class="ways">
  <div class="ways-tabs" role="tablist" aria-label="Do it from">${WAYS.map(tab).join('')}${label ? `<span class="label">${esc(label)}</span>` : ''}</div>
  ${WAYS.map(panel).join('\n  ')}
</div>`;
}
// a shell line: the command, then a dim comment
const sh = (cmd, note = '') => `${esc(cmd)}${note ? `   <span class="muted"># ${esc(note)}</span>` : ''}`;

const LOGO = (a, size = 20) => `<img src="/assets/logos/${a.logo}" alt="" width="${size}" height="${size}"${a.mono ? ' class="mono"' : ''} />`;

const FAQ = [
  ['Is Wisp Agents free?', 'Yes. Wisp Agents is open source under the MIT license and runs on your own computers. You use the coding agents you already have, on your own plans and keys, so Wisp Agents adds nothing to your bill. Wisp Agents Cloud, a hosted instance on a server, is optional.'],
  ['Which coding agents does Wisp Agents work with?', 'Claude Code, Codex, Cursor CLI, opencode, GitHub Copilot CLI, Grok Build and Antigravity. Wisp Agents runs the real CLIs you already installed and signed in to, so each agent keeps its own models, tools and settings.'],
  ['Does my code leave my computer?', 'No. Wisp Agents runs on your machines. Your prompts and code go only to the agents you use, under your own accounts, as they would without Wisp Agents. Wisp Agents sends anonymous usage counts you can turn off, never your code or prompts.'],
  ['Do I need an API key?', 'No. If you already use Claude Code or Codex with a subscription, Wisp Agents uses that same sign-in. API keys work too, for agents that take them.'],
  ['Can I use it from the terminal?', 'Yes. Everything the app does, the CLI does (wisp "fix the flaky test", wisp ls, wisp ship), and wisp tui is a full terminal UI. After a command fails, wisp "check the error above" hands the agent what just happened in that terminal. Your agents can drive Wisp Agents too, through its MCP server.'],
  ['Is there a phone app?', 'The phone apps for iPhone and Android are coming. Join the waitlist on the Get the app page and we email you once, when yours is out. Today you can already use Wisp Agents from your phone’s browser, from Wisp Agents Cloud or a computer you put on the Grid.'],
  ['Does it run on Windows and Linux?', 'Yes. The desktop app runs on macOS, Windows and Linux, and the command line installs with npm on macOS, Windows and Linux, x64 and Arm (Node.js 18 or later).'],
  ['What does “as code” mean here?', 'Your agents, crew, machines, skills, MCP servers and workflows can live as files in your repo’s .wisp/ folder. You review them in a pull request, apply them with wisp apply, and every task the agents ran is kept next to the code it wrote.'],
];

// ---------- the home page ----------

function landing() {
  waysN = 0;
  const harnessList = AGENTS.map((a) => `<li><a href="/agents/${a.slug}.html">${LOGO(a)}${esc(a.name)}</a></li>`).join('');
  const body = `
<div class="wrap">
  <section class="hero" aria-labelledby="hero-h">
    <p class="kicker"><b>Wisp Agents</b> · Open source · MIT</p>
    <h1 id="hero-h">Manage the code your agents write.</h1>
    <p class="lede"><b>Declare</b> your coding agents as code. <b>Run</b> them anywhere, many at once. <b>Track</b> every line they write, right next to your code. From the desktop app, your phone or the terminal.</p>
    <div class="cta" id="install">
      ${install()}
      <a class="btn primary" href="/app.html" data-dl="hero">${DL_ICON}Get the app</a>
      <a class="btn ghost" href="/docs/start/install/">Read the docs</a>
    </div>
    <p class="muted small" style="margin-top:14px">Works with Claude Code, Codex, Cursor, opencode, Copilot, Grok and Antigravity, on the plans you already pay for.</p>
    ${ALPHA_NOTE}
    <div class="stage">
      ${deskShot('desktop-task', 'The Wisp Agents desktop app: a list of tasks on the left, and a finished task with its report: what the agent did, the diff and the tests', { eager: true, sizes: '(max-width: 1100px) 100vw, 1080px' })}
      ${phoneShot('ios-task', 'The Wisp Agents iPhone app showing the same task’s report', { sizes: '(max-width: 700px) 34vw, 260px' })}
    </div>
  </section>

  <section aria-labelledby="agents-h" style="padding-top:24px">
    <h2 id="agents-h" class="lead-h">One place for every coding agent you use</h2>
    <ul class="logos">${harnessList}</ul>
  </section>

  <section aria-labelledby="why-h">
    <div class="sec-head">
      <p class="eyebrow">Why Wisp Agents</p>
      <h2 id="why-h">The new way to build with agents</h2>
      <p>Coding agents now write much of the code. The hard part moved: keeping several of them busy, checking their work, and knowing later why a line is there. Wisp Agents is built for that part.</p>
    </div>
    <div class="why">
      <div class="card"><span class="num">01</span><h3>Your agents, your plans</h3><p>Wisp Agents runs the real Claude Code, Codex and the others, signed in with your own accounts. No new model, no markup, no lock-in.</p></div>
      <div class="card"><span class="num">02</span><h3>Many at once, nothing collides</h3><p>Each task gets its own git worktree. Run several at once, on this computer, your other ones or the cloud, and ship each as its own pull request.</p></div>
      <div class="card"><span class="num">03</span><h3>Done means done</h3><p>Give a task a goal and judges on other harnesses. It keeps going until they agree, or tells you why it can’t.</p></div>
      <div class="card"><span class="num">04</span><h3>Code that remembers why</h3><p>Every task is a file in your repo. <code>wisp blame</code> shows which task wrote a line and what it was asked. Rerun it, resume it, share it.</p></div>
    </div>
  </section>

  <section id="declare" aria-labelledby="declare-h">
    <div class="pillar">
      <div>
        <p class="eyebrow">1 · Declare</p>
        <h2 id="declare-h">Your AI dev team, as code<span class="tag">New</span></h2>
        <p class="muted">Agents, crew members, machines, skills, MCP servers, rules and workflows live in your repo’s <code>.wisp/</code> folder. Review a change to your team like any other change. Share it with a link.</p>
        <ul>
          <li>Variables, references and modules, like infrastructure as code.</li>
          <li><code>wisp plan</code> shows what will change; <code>wisp apply</code> makes it so.</li>
          <li>Skills, <code>AGENTS.md</code> notes and MCP servers shared with the whole team.</li>
        </ul>
        <p><a href="/docs/guides/declare/software-factory-as-code/">Build a software factory as code ${'→'}</a></p>
      </div>
      ${ways({
        label: 'Set up a team',
        desktop: `<ol><li>Open the <b>Org</b> page (its button is at the bottom of the rail on the left) and choose <b>Add member</b>: a reviewer, a fixer, a nightly bot.</li><li>Settings (<kbd>⌘K</kbd> → Settings) → <b>Setup</b> → <b>Make the formula</b> writes your whole setup as a file to commit.</li><li>Paste a formula in the same pane to import one: you see what it would change first.</li></ol>${wayShot(deskShot('desktop-org', 'The Org page in the Wisp Agents desktop app: the crew, and who hands work to whom', { sizes: '(max-width: 880px) 90vw, 540px' }))}`,
        phone: `<ol><li>Open the <b>Crew</b> tab to see your members and add one.</li><li>Tap a member’s story under the Tasks bar to see what they’re doing, and reply.</li><li>Writing the formula itself is easier on a computer: the Desktop app or the CLI.</li></ol>`,
        tui: `<ol><li><code>wisp tui</code>, then <kbd>g</kbd> <kbd>c</kbd> for the crew view.</li><li><kbd>n</kbd> adds a member, <kbd>a</kbd> gives one a job, <kbd>c</kbd> checks in.</li><li>Any CLI command runs from the reply box: <code>/crew create --name Ada</code>.</li></ol>`,
        cli: `<span class="c"># .wisp/wisp.yml</span>
<span class="k">wisp</span>: 1
<span class="k">agents</span>:
  <span class="k">reviewer</span>: { <span class="k">harness</span>: <span class="s">codex</span>, <span class="k">mode</span>: <span class="s">plan</span> }
  <span class="k">fixer</span>:    { <span class="k">harness</span>: <span class="s">claude</span>, <span class="k">model</span>: <span class="s">opus</span> }

${sh('wisp init', 'a .wisp/ folder in this repo')}
${sh('wisp plan', 'what applying it would change')}
${sh('wisp apply -f .wisp/wisp.yml')}`,
      })}
    </div>

    <div class="pillar" id="run">
      <div>
        <p class="eyebrow">2 · Run</p>
        <h2>Every harness, any machine, from anywhere</h2>
        <p class="muted">Start a task in one line. Wisp Agents runs the agent in its own worktree, streams what it does, and hands back a short report with the diff and the tests. The full transcript is always one click away.</p>
        <ul>
          <li>Follow up, queue the next step, or ask on the side with <code>btw</code> without stopping it.</li>
          <li><b>Compare</b> two agents on the same task, or <b>try it two ways</b> on one.</li>
          <li>Run it here, on your other computers over SSH, or in Wisp Agents Cloud.</li>
        </ul>
        <p><a href="/docs/guides/run/run-and-follow-up/">Run a task and follow up ${'→'}</a></p>
      </div>
      ${ways({
        label: 'Start and ship a task',
        desktop: `<ol><li>Press <kbd>N</kbd> for <b>New task</b>, pick the agent and model in its footer, type, <kbd>Enter</kbd>.</li><li>Answer its questions on the task page; reply below to follow up. <kbd>]</kbd> opens the side panel: Changes, PR, Shell, Preview.</li><li><b>Ship</b> (<kbd>S</kbd>) commits, pushes and opens the pull request.</li></ol><p class="small muted">In the browser it’s the same window, menus and keys.</p>${wayShot(deskShot('desktop-new-task', 'New task in the Wisp Agents desktop app: a prompt typed, with the mode, Claude Code and where it runs under it', { sizes: '(max-width: 880px) 90vw, 540px' }))}`,
        phone: `<ol><li>In <b>Tasks</b>, tap <b>New task</b>, write what you want, send.</li><li>Answer its questions in the task; the reply bar follows up. Swipe a row right to mark it done.</li><li><b>⋯</b> (iPhone) or <b>⋮</b> (Android) → <b>Ship it…</b> opens the pull request.</li></ol>${wayShot(phoneShot('ios-list', 'The task list in the Wisp Agents iPhone app: tasks that need you, and tasks still working', { sizes: '200px' }), true)}`,
        tui: `<ol><li><code>wisp tui</code>, <kbd>n</kbd> for a new task, <kbd>Enter</kbd> to send.</li><li><kbd>1</kbd>–<kbd>9</kbd> answers a question; <kbd>y</kbd> <kbd>a</kbd> <kbd>n</kbd> allow a tool.</li><li><code>/ship ship</code> opens the pull request; <kbd>d</kbd> shows the diff.</li></ol>`,
        cli: `${sh('wisp "fix the flaky reconnect test"', 'runs in its own worktree')}
${sh('wisp "add rate limiting to /login" --agent codex --effort high')}
${sh('wisp "audit the auth flow" --compare claude,codex')}
${sh('wisp attach last', 'follow it live; reply after')}
${sh('wisp ship <id>', 'commit, push, open the PR')}`,
      })}
    </div>

    <div class="pillar" id="track">
      <div>
        <p class="eyebrow">3 · Track</p>
        <h2>Know why every line exists<span class="tag">New</span></h2>
        <p class="muted">A repo built by agents should remember how it was built. Wisp Agents keeps each task in <code>.wisp/tasks</code>: what was asked, from which commit, what the agent said, the code it wrote.</p>
        <ul>
          <li><code>wisp blame</code> on a line, a file or a pull request gives the task and its ask.</li>
          <li><code>wisp resume</code> picks last month’s task up on today’s code.</li>
          <li>Tasks you ran in a harness outside Wisp Agents come in with <code>wisp adopt</code>.</li>
        </ul>
        <p><a href="/docs/guides/track/track-agent-work/">Track your agents’ work beside your code ${'→'}</a></p>
      </div>
      ${ways({
        label: 'Find out why',
        desktop: `<ol><li><kbd>⌘K</kbd> → <b>Repo history</b>: what agents did in this repo, from everyone who worked on it.</li><li>Open a task to read what was asked and its report; <b>Resume</b> carries it on, on today’s code.</li><li>Keep a task in the repo with <b>Track</b> in its <b>⋯</b> menu.</li></ol>${wayShot(deskShot('desktop-task', 'A task in the Wisp Agents desktop app: its turns, what was asked, and the report with the diff and the tests', { sizes: '(max-width: 880px) 90vw, 540px' }))}`,
        phone: `<ol><li>Open a task to read what was asked, its turns and its report.</li><li><b>⋯</b> / <b>⋮</b> → <b>Rerun…</b> asks the same again; <b>Keep and copy ›</b> forks or shares it.</li><li>The repo’s full history is in the Desktop app, the TUI and the CLI.</li></ol>${wayShot(phoneShot('ios-task', 'A task in the Wisp Agents iPhone app: its turns, what was asked, and the report', { sizes: '200px' }), true)}`,
        tui: `<ol><li><code>wisp tui</code>, then <kbd>g</kbd> <kbd>h</kbd> for the repo history view.</li><li><kbd>Enter</kbd> opens a task; <kbd>/</kbd> searches.</li><li>A task’s page says what it was asked and from which commit.</li></ol>`,
        cli: `${sh('wisp blame src/sync/peers.ts:118', 'the task that wrote it, and what it was asked')}
${sh("wisp blame '#812'", 'the tasks behind a pull request')}
${sh('wisp log src/sync', 'the tasks that touched it, newest first')}
${sh('wisp resume <id>', 'carry on, on today’s code')}
${sh('wisp adopt --all --since 7d', 'sessions you ran outside wisp')}`,
      })}
    </div>
  </section>

  <section aria-labelledby="more-h">
    <div class="sec-head">
      <p class="eyebrow">Everything else</p>
      <h2 id="more-h">One tool for the whole loop</h2>
      <p>Every feature works from the desktop app, the phone app, the TUI and the CLI, and from your own agent through Wisp Agents’ MCP tools.</p>
    </div>
    <div class="grid">
      ${[
        ['Worktrees and pull requests', 'Each task on its own branch and worktree. See the changes, fix conflicts, ship and merge from Wisp Agents.', 'wisp ship <id> --draft', '/docs/guides/run/worktrees-and-shipping/'],
        ['Compare and try it two ways', 'The same task on two agents side by side, or twice on one with one setting changed. Keep the better one.', 'wisp "…" --try skills=off', '/docs/guides/run/compare-and-try/'],
        ['Goals and judges', 'Agents keep going until judges on other harnesses agree the goal is met.', 'wisp goal set <id> "tests pass"', '/docs/guides/run/goals/'],
        ['Crew and check-ins', 'Named agents with a role, a kit and a schedule. They check in, pick up jobs and hand work to each other.', 'wisp crew assign <id> "review #812"', '/docs/guides/run/crew-and-check-ins/'],
        ['Automations and triggers', 'Run a task on a schedule, on a GitHub event, when a goal is met, or when anything else happens.', 'wisp automations tasks', '/docs/guides/run/schedules-and-triggers/'],
        ['Slack, Telegram, Discord, Linear…', 'Start tasks and talk to your crew from Slack, Telegram, Discord, ntfy, GitHub, Linear or email.', 'wisp help channels', '/docs/guides/run/from-slack-and-linear/'],
        ['Review by another agent', 'Have a second harness read the diff before you do. Blind spots differ; so do the bugs they catch.', 'wisp help review', '/docs/guides/run/review-by-another-agent/'],
        ['Long threads, fresh threads', 'Wisp Agents keeps a task’s Notes: goal, decisions, what failed. Start a fresh conversation from them when one wears out.', 'wisp fresh <id> --print', '/docs/guides/run/long-threads/'],
        ['Machines, Grid and Cloud', 'Run on your other computers over SSH, put one on the Grid, or get your own Wisp Agents in the cloud.', 'wisp "…" --on studio', '/docs/guides/run/machines/'],
        ['MCP from your harness', 'Your Claude Code or Codex can start Wisp Agents tasks, read results and talk to the crew, without leaving its chat.', 'wisp skill install --agent all', '/docs/guides/run/mcp-from-your-harness/'],
        ['Budgets and usage', 'See your plan’s limits and what each task took. Hold work near the limit instead of hitting it.', 'wisp usage', '/docs/guides/run/budgets-and-usage/'],
        ['Plugins', 'Everything in Wisp Agents is a plugin. Add a command, a pane, a channel or a theme with the same API the built-ins use.', 'wisp plugin new my-plugin', '/docs/plugins/make-wisp-your-own/'],
      ].map(([t, p, c, h]) => `<div class="card"><h3>${esc(t)}</h3><p>${esc(p)}</p><pre><code>${esc(c)}</code></pre><a class="more" href="${h}">Read the guide <span aria-hidden="true">→</span></a></div>`).join('\n      ')}
    </div>
  </section>

  <section id="apps" aria-labelledby="apps-h">
    <div class="sec-head">
      <p class="eyebrow">Apps</p>
      <h2 id="apps-h">On your desk, and in your pocket</h2>
      <p>The same Wisp Agents, everywhere: a fast native window on your computer, and apps for your phone so a question from an agent never waits for you to get back.</p>
    </div>
    <div class="apps">
      <div class="app-card desk">
        <h3>Desktop app</h3>
        <p class="muted">For macOS, Windows and Linux. Every task in one window, your shortcuts, notifications when an agent needs you. It updates itself.</p>
        <div class="dl">
          <a class="btn primary" href="${DL}/wisp-mac-arm64.dmg" data-dl="mac-arm64">${DL_ICON}macOS</a>
          <a class="btn ghost" href="${DL}/wisp-windows-x64-setup.exe" data-dl="win-x64">Windows</a>
          <a class="btn ghost" href="${DL}/wisp-linux-x64.deb" data-dl="linux-deb">Linux</a>
        </div>
        <p class="small muted" style="margin:0">Intel Mac, Arm and AppImage builds on the <a href="/app.html#desktop">download page</a>.</p>
        <div class="app-shots">${deskShot('desktop-org', 'The crew’s Org page in the Wisp Agents desktop app: who hands work to whom', { sizes: '(max-width: 880px) 92vw, 540px' })}</div>
      </div>
      <div class="app-card" id="phone">
        <h3>Phone apps <span class="tag">Waitlist</span></h3>
        <p class="muted">For iPhone and Android, built for each. Answer an agent’s question, check a report, start a task from the train. Join the waitlist and we email you once, when yours is out.</p>
        ${waitlistForm('home')}
        <div class="app-shots">
          ${phoneShot('ios-list', 'The task list in the Wisp Agents iPhone app', { sizes: '210px' })}
          ${phoneShot('android-task', 'A finished task’s report in the Wisp Agents Android app', { sizes: '210px' })}
        </div>
      </div>
    </div>
  </section>

  <section aria-labelledby="speed-h">
    <div class="sec-head">
      <p class="eyebrow">Speed is a feature</p>
      <h2 id="speed-h">Fast enough to forget it’s there</h2>
      <p>One Rust binary runs the server, the CLI, the TUI and the built-in plugins; a small Solid app sits on top. Speed budgets fail the build, so they stay met.</p>
    </div>
    <div class="stats">
      <div class="stat"><b>55 ms</b><span>from a cold start to the first answer, with 5,000 tasks</span></div>
      <div class="stat"><b>0.95 ms</b><span>to read the first page of a 5,000-task list</span></div>
      <div class="stat"><b>103 ms</b><span>from an agent’s step reaching every connected app (p99), 10 tasks running</span></div>
      <div class="stat"><b>102 MB</b><span>of memory at rest, holding 5,000 tasks</span></div>
    </div>
    <p class="muted small" style="margin-top:14px">Measured on 19 November 2026 on an Apple M2 (macOS 15.6) with the repo’s <code>spikes/server-budgets</code>: <code>wisp serve</code> in its release build on 5,000 seeded tasks, fake agents, no network; each time the median of 5 rounds (cold start: 5 restarts a round; the stream: 4 clients, 2,400 steps). The same budgets gate every pull request on Linux, where the numbers are higher.</p>
  </section>

  <section id="cloud" aria-labelledby="cloud-h">
    <div class="pillar">
      <div>
        <p class="eyebrow">Wisp Agents Cloud &amp; Grid</p>
        <h2 id="cloud-h">Agents that keep working when your laptop sleeps</h2>
        <p class="muted">Wisp Agents Cloud gives you your own instance on a server, signed in with GitHub or Google. Put your own computers on it too, and reach all of them from any browser or your phone. No SSH, no open ports. Or run a Grid on your own server for your team.</p>
        <p><a class="btn ghost" href="/cloud.html">About Wisp Agents Cloud ${ARROW}</a></p>
      </div>
      ${ways({
        label: 'Reach this computer from anywhere',
        desktop: `<ol><li>Settings → <b>Machines</b> → <b>Reach this computer from anywhere</b> → <b>Sign in and add it</b> (or Home’s tip → <b>Set up</b>).</li><li>Sign in with GitHub or Google.</li><li>Open your grid address from any browser or phone.</li></ol>${wayShot(deskShot('desktop-machines', 'Settings → Machines in the Wisp Agents desktop app: SSH, Tailscale, and Reach this computer from anywhere', { sizes: '(max-width: 880px) 90vw, 540px' }))}`,
        phone: `<ol><li>Open the <b>Computers</b> tab.</li><li><b>Continue with GitHub</b> or <b>Continue with Google</b>.</li><li>Your cloud instance and the computers on it show there; tap one to open its tasks.</li></ol>`,
        tui: `<ol><li><kbd>g</kbd> <kbd>m</kbd> for machines; <kbd>c</kbd> connects one.</li><li>From the reply box: <code>/grid-client signin --add-computer</code>.</li></ol>`,
        cli: `${sh('wisp cloud signin', 'Wisp Agents Cloud, or: wisp cloud signin <your grid>')}
${sh('wisp computer on', 'this computer on the grid')}
${sh('wisp ls --on cloud')}
${sh('wisp grid serve', 'run a Grid on your own server')}`,
      })}
    </div>
  </section>

  <section id="faq" aria-labelledby="faq-h">
    <div class="sec-head center" style="text-align:center"><p class="eyebrow">Questions</p><h2 id="faq-h">Frequently asked</h2></div>
    <div class="faq">
      ${FAQ.map(([q, a]) => `<details><summary>${esc(q)}</summary><p>${esc(a)}</p></details>`).join('\n      ')}
    </div>
  </section>

  <section class="close" aria-labelledby="close-h">
    <h2 id="close-h">Declare. Run. Track.</h2>
    <p class="lede">Your agents write the code. Wisp Agents keeps them busy, checks their work and remembers why.</p>
    <div class="cta">${install()}<a class="btn primary" href="/app.html">${DL_ICON}Get the app</a></div>
  </section>
</div>`;
  const homeDesc = 'Wisp Agents is an open-source app, TUI and CLI to run Claude Code, Codex and Cursor in parallel, declare agents as code, and track every line they write.';
  return page({
    path: '/',
    title: 'Wisp Agents — Manage the code your coding agents write',
    description: homeDesc,
    body,
    main: '',
    schema: [
      { '@type': 'WebSite', '@id': `${SITE}/#website`, url: `${SITE}/`, name: 'Wisp Agents', alternateName: ['wisp', 'wisp-agents'], description: homeDesc, publisher: { '@id': `${SITE}/#org` }, inLanguage: 'en' },
      { '@type': 'Organization', '@id': `${SITE}/#org`, name: 'Wisp Agents', alternateName: 'wisp', url: `${SITE}/`, email: 'hello@trywisp.dev', logo: { '@type': 'ImageObject', url: `${SITE}/assets/icon.png`, width: 1024, height: 1024 }, sameAs: [REPO, 'https://x.com/trywispagents', 'https://www.npmjs.com/package/wisp-agents'] },
      SOFTWARE,
      { '@type': 'FAQPage', '@id': `${SITE}/#faq`, mainEntity: FAQ.map(([q, a]) => ({ '@type': 'Question', name: q, acceptedAnswer: { '@type': 'Answer', text: a } })) },
    ],
  });
}

const SOFTWARE = {
  '@type': 'SoftwareApplication',
  '@id': `${SITE}/#app`,
  name: 'Wisp Agents',
  alternateName: ['wisp', 'wisp-agents'],
  softwareVersion: VERSION,
  url: `${SITE}/`,
  description: 'Open-source app, TUI and CLI to run Claude Code, Codex, Cursor and other coding agents in parallel, declare them as code, and track every line they write.',
  applicationCategory: 'DeveloperApplication',
  operatingSystem: 'macOS, Windows, Linux',
  license: 'https://opensource.org/licenses/MIT',
  codeRepository: REPO,
  offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
  downloadUrl: 'https://github.com/WispAgents/wisp/releases/latest',
  installUrl: `${SITE}/app.html`,
  softwareHelp: `${SITE}/docs/`,
  screenshot: `${SITE}/assets/app/desktop-task-light-1440.webp`,
  author: { '@id': `${SITE}/#org` },
};

/** The phone app waitlist: posts to wisp Cloud's list (apps/grid edge_waitlist.rs, kind phone-app). Without JavaScript the
 * form still posts there. */
function waitlistForm(where) {
  const id = `wl-${where}`;
  return `<form class="wl ph-no-capture" data-waitlist action="${GRID}/api/waitlist" method="post" novalidate>
  <input type="hidden" name="kind" value="phone-app" /><input type="hidden" name="source" value="site" />
  <label class="sr" for="${id}">Your email</label>
  <div class="wl-row">
    <input id="${id}" type="email" name="email" required autocomplete="email" inputmode="email" placeholder="you@example.com" maxlength="254" />
    <button class="btn primary" type="submit">${PHONE_ICON}Join the waitlist</button>
  </div>
  <fieldset><legend>For</legend>
    <label><input type="radio" name="platform" value="ios" /> iPhone</label>
    <label><input type="radio" name="platform" value="android" /> Android</label>
    <label><input type="radio" name="platform" value="both" checked /> Both</label>
  </fieldset>
  <p class="msg" role="alert" aria-live="polite"></p>
  <p class="small muted" style="margin:0">One email when the app is out, nothing else. <a href="/privacy.html#the-phone-app-waitlist">What we do with your address</a>.</p>
</form>`;
}

// ---------- Get the app ----------

function appPage() {
  waysN = 0;
  const row = (os, items) => `<tr><th scope="row">${os}</th><td>${items.map(([t, f]) => `<a href="${DL}/${f}" data-dl="${f}">${t}</a>`).join(' · ')}</td></tr>`;
  const body = `
<div class="wrap">
  <header class="doc-head">
    <p class="eyebrow">Get the app</p>
    <h1>Wisp Agents on your desk and in your pocket</h1>
    <p class="lede">The desktop app for macOS, Windows and Linux is ready today. The phone apps for iPhone and Android are next: join the waitlist and we email you once, when yours is out.</p>
  </header>
  <div class="apps">
    <div class="app-card desk" id="desktop">
      <h2 style="font-size:1.5rem;margin:0">Desktop app</h2>
      <p class="muted">A native window (Tauri, not Electron). Every task, your shortcuts, notifications when an agent needs you, and updates that install themselves.</p>
      <div class="scroll-x"><table class="table">
        ${row('macOS', [['Apple silicon (.dmg)', 'wisp-mac-arm64.dmg'], ['Intel (.dmg)', 'wisp-mac-x64.dmg']])}
        ${row('Windows', [['x64 installer', 'wisp-windows-x64-setup.exe'], ['Arm installer', 'wisp-windows-arm64-setup.exe']])}
        ${row('Linux', [['.deb x64', 'wisp-linux-x64.deb'], ['.deb Arm', 'wisp-linux-arm64.deb'], ['AppImage x64', 'wisp-linux-x64.AppImage'], ['AppImage Arm', 'wisp-linux-arm64.AppImage']])}
      </table></div>
      <p class="small muted">Or with Homebrew: <code>brew install --cask wispagents/tap/wisp</code>. Checksums are in <code>SHA256SUMS.txt</code> on the <a href="https://github.com/WispAgents/wisp/releases/latest">latest release</a>. The builds aren’t signed yet: on macOS, after the first open, System Settings → Privacy &amp; Security → <b>Open Anyway</b>; on Windows, SmartScreen → <b>More info</b> → <b>Run anyway</b>.</p>
      <div class="app-shots">${deskShot('desktop-task', 'The Wisp Agents desktop app with a finished task and its report', { sizes: '(max-width: 880px) 92vw, 540px' })}</div>
    </div>
    <div class="app-card" id="phone">
      <h2 style="font-size:1.5rem;margin:0">Phone apps <span class="tag">Waitlist</span></h2>
      <p class="muted">Built for each phone: Material on Android, Apple’s look on iPhone. Your tasks and crew, the answers and their diffs, a reply box, notifications, and your computers over SSH or Wisp Agents Cloud.</p>
      ${waitlistForm('app')}
      <p class="small muted">Can’t wait? Open your Wisp Agents Cloud instance, or a computer you put on the Grid, in your phone’s browser and choose <b>Add to Home Screen</b>: it runs full screen. <a href="/docs/guides/apps/phone-app/">How</a>.</p>
      <div class="app-shots">
        ${phoneShot('ios-list', 'The task list in the Wisp Agents iPhone app', { sizes: '210px' })}
        ${phoneShot('android-task', 'A finished task’s report in the Wisp Agents Android app', { sizes: '210px' })}
      </div>
    </div>
  </div>
  <section aria-labelledby="cli-h" style="padding-top:56px">
    <h2 id="cli-h">Prefer the terminal?</h2>
    <p class="muted">The <code>wisp</code> command runs everything the app does, and <code>wisp tui</code> is a full terminal UI. It needs Node.js 18 or later.</p>
    <p class="muted">A command just failed? Type <code>wisp "check the error above"</code>: the agent gets the command, its exit code, its output and your folder, with secrets masked. <a href="/docs/guides/run/terminal-context/">How it works</a>.</p>
    <div class="cta" style="justify-content:flex-start">${install('npm install -g wisp-agents')}</div>
    ${ALPHA_NOTE}
    <p class="small muted" style="margin-top:12px">Also: <code>curl -fsSL https://trywisp.dev/install.sh | bash</code> · <code>brew install wispagents/tap/wisp-agents</code> · try it with <code>npx wisp-agents</code>. <a href="/docs/start/install/">Install guide</a>.</p>
  </section>
</div>`;
  return page({
    path: '/app.html',
    title: 'Download Wisp Agents — macOS, Windows, Linux & Phone',
    description: 'Download the Wisp Agents desktop app for macOS, Windows and Linux, or join the iPhone and Android waitlist. Run and follow coding agents from anywhere.',
    crumbs: [['Get the app', '/app.html']],
    body,
    schema: [{ ...SOFTWARE, '@id': `${SITE}/app.html#app` }],
  });
}

// ---------- Wisp Agents Cloud ----------

function cloudPage() {
  waysN = 0;
  const body = `
<div class="wrap">
  <header class="doc-head" id="join">
    <p class="eyebrow">Wisp Agents Cloud &amp; Grid</p>
    <h1>Your own Wisp Agents in the cloud</h1>
    <p class="lede">Wisp Agents Cloud is your own instance on a server, yours alone. Your agents keep working when your laptop sleeps, and you reach it, and your own computers, from any browser or your phone.</p>
    <div class="cta" style="justify-content:flex-start"><a class="btn primary" href="${GRID}">Sign in to Wisp Agents Cloud ${ARROW}</a><a class="btn ghost" href="/docs/guides/run/machines/">How it works</a></div>
  </header>
  <h2 class="sr">What Wisp Agents Cloud includes</h2>
  <div class="grid">
    <div class="card"><h3>Always on</h3><p>Tasks run on your cloud instance while your computers are off. It sleeps when idle and wakes when you or a trigger need it.</p></div>
    <div class="card"><h3>Your computers too</h3><p><code>wisp computer on</code> puts a computer on the grid. Open it at your grid address from anywhere: no SSH, no open ports.</p></div>
    <div class="card"><h3>Your sign-ins</h3><p>Sign in to Claude Code, Codex and the others on your cloud instance, with your own plans. Secrets are encrypted.</p></div>
    <div class="card"><h3>Share</h3><p>Share a task, a setup or a crew member with a link, with the access you choose.</p></div>
    <div class="card"><h3>Run your own Grid</h3><p>The same service on your own server for your team: <code>wisp grid serve</code>, with sign-in through GitHub or Google. <a href="/docs/guides/run/machines/">How</a>.</p></div>
    <div class="card"><h3>From your phone</h3><p>Open your grid address on your phone and add it to your Home Screen. The phone apps are <a href="/app.html#phone">on the waitlist</a>.</p></div>
  </div>
  <section aria-labelledby="how-h" style="padding-top:56px">
    <h2 id="how-h">Get started</h2>
    ${ways({
      label: 'Wisp Agents Cloud',
      desktop: `<ol><li>Settings → <b>Machines</b> → <b>Reach this computer from anywhere</b> → <b>Sign in and add it</b>, or Home’s tip → <b>Set up</b>.</li><li>Continue with GitHub or Google.</li><li>Your cloud instance and your computers show under <b>Machines</b>.</li></ol>${wayShot(deskShot('desktop-machines', 'Settings → Machines in the Wisp Agents desktop app', { sizes: '(max-width: 880px) 90vw, 760px' }))}`,
      phone: `<ol><li>Open the <b>Computers</b> tab.</li><li><b>Continue with GitHub</b> or <b>Continue with Google</b>.</li><li>Your cloud instance and the computers on it show there.</li></ol>`,
      tui: `<ol><li><kbd>g</kbd> <kbd>m</kbd> opens the machines view.</li><li>Send <code>/grid-client signin --add-computer</code> from the reply box.</li></ol>`,
      cli: `${sh('wisp cloud signin')}
${sh('wisp cloud status')}
${sh('wisp computer on', 'reach this computer through the grid')}
${sh('wisp "nightly dependency check" --on cloud')}`,
    })}
  </section>
</div>`;
  const cloudDesc = 'Wisp Agents Cloud runs your coding agents on a dedicated server when your laptop sleeps, reachable from any browser or phone. Or self-host a Wisp Grid.';
  return page({
    path: '/cloud.html',
    title: 'Wisp Agents Cloud — Run Coding Agents 24/7 on a Server',
    description: cloudDesc,
    crumbs: [['Wisp Agents Cloud', '/cloud.html']],
    body,
    image: '/assets/og.png',
    schema: [{ '@type': 'WebPage', '@id': `${SITE}/cloud.html#page`, name: 'Wisp Agents Cloud', description: cloudDesc, url: `${SITE}/cloud.html`, isPartOf: { '@id': `${SITE}/#website` }, about: { '@id': `${SITE}/#app` }, inLanguage: 'en' }],
  });
}

// ---------- the harnesses ----------

function agentPage(a) {
  const others = AGENTS.filter((x) => x !== a);
  const body = `
<div class="wrap">
  <header class="doc-head">
    <div class="agent-head">${LOGO(a, 44)}<p class="eyebrow" style="margin:0">${esc(a.vendor)} · <code>${esc(a.bin)}</code></p></div>
    <h1>Run ${esc(a.name)} with Wisp Agents</h1>
    <p class="lede">${a.lede}</p>
    <div class="cta" style="justify-content:flex-start">${install(`wisp "fix the flaky test" --agent ${a.id}`)}<a class="btn ghost" href="/docs/guides/agents/${a.slug}/">Start with ${esc(a.name)} ${ARROW}</a></div>
  </header>
  <div class="prose">
    <h2>How Wisp Agents runs it</h2>
    <pre><code>${esc(a.run)}</code></pre>
    <div class="scroll-x"><table class="table">
      <thead><tr><th scope="col">Wisp Agents mode</th><th scope="col">What ${esc(a.name)} gets</th></tr></thead>
      <tbody>${Object.entries(MODES).map(([k, t]) => `<tr><td>${esc(t)}</td><td><code>${esc(a.modes[k])}</code></td></tr>`).join('')}</tbody>
    </table></div>
    <h2>Questions and permissions</h2>
    <p>${a.ask}</p>
    <p>${a.approve}</p>
    <h2>Models</h2>
    <p>${a.models}</p>
    <h2>Ask on the side, pick it up yourself</h2>
    <p>A <code>btw</code> question is ${a.btw}. <b>Open in ${esc(a.name)}</b> (<kbd>o</kbd> in the TUI) resumes the same conversation in its own CLI: <code>${esc(a.resume)}</code>.</p>
    <h2>Also</h2>
    <ul>${a.extras.map((x) => `<li>${x}</li>`).join('')}</ul>
    <h2>The other agents</h2>
    <ul class="logos" style="justify-content:flex-start">${others.map((o) => `<li><a href="/agents/${o.slug}.html">${LOGO(o)}${esc(o.name)}</a></li>`).join('')}</ul>
  </div>
</div>`;
  const desc = `Run ${a.name} with Wisp Agents in parallel git worktrees on your own plan. Route questions and approvals to you with a report, diff and test run.`;
  return page({
    path: `/agents/${a.slug}.html`,
    title: `Run ${a.name} in Parallel · Wisp Agents`,
    description: desc,
    crumbs: [['Agents', '/agents/'], [a.name, `/agents/${a.slug}.html`]],
    body,
    type: 'article',
    schema: [{
      '@type': 'TechArticle',
      headline: `Run ${a.name} with Wisp Agents`,
      description: desc,
      url: `${SITE}/agents/${a.slug}.html`,
      image: `${SITE}/assets/og.png`,
      dateModified: lastmod(['scripts/site.mjs', `plugins/${a.id}`]),
      author: { '@id': `${SITE}/#org` },
      publisher: { '@id': `${SITE}/#org` },
      isPartOf: { '@id': `${SITE}/#website` },
      about: { '@id': `${SITE}/#app` },
      inLanguage: 'en',
    }],
  });
}

function agentsIndex() {
  const body = `
<div class="wrap">
  <header class="doc-head">
    <p class="eyebrow">Agents</p>
    <h1>Every coding agent, one place</h1>
    <p class="lede">Wisp Agents runs the real CLIs you already installed and signed in to, on your own plans. Pick one per task, compare two on the same task, or let a crew mix them.</p>
  </header>
  <div class="grid">
    ${AGENTS.map((a) => `<a class="card" style="text-decoration:none;color:inherit" href="/agents/${a.slug}.html"><span class="agent-head" style="margin:0">${LOGO(a, 32)}<h2 style="font-size:1.2rem;margin:0">${esc(a.name)}</h2></span><p>${esc(a.vendor)} · <code>${esc(a.bin)}</code></p><p>${a.models}</p></a>`).join('\n    ')}
  </div>
</div>`;
  const desc = 'Wisp Agents runs Claude Code, Codex, Cursor CLI, opencode, GitHub Copilot CLI, Grok Build and Antigravity in parallel on your own plans, from one place.';
  return page({
    path: '/agents/',
    title: 'Run Claude Code, Codex, Cursor & More · Wisp Agents',
    description: desc,
    crumbs: [['Agents', '/agents/']],
    body,
    schema: [{
      '@type': 'CollectionPage',
      '@id': `${SITE}/agents/#page`,
      name: 'Coding agents supported by Wisp Agents',
      description: desc,
      url: `${SITE}/agents/`,
      isPartOf: { '@id': `${SITE}/#website` },
      mainEntity: {
        '@type': 'ItemList',
        itemListElement: AGENTS.map((a, i) => ({ '@type': 'ListItem', position: i + 1, name: a.name, url: `${SITE}/agents/${a.slug}.html` })),
      },
      inLanguage: 'en',
    }],
  });
}

// ---------- pages from markdown ----------

function mdPage({ path: p, file, title, description, eyebrow, h1, lede = '', base = `${REPO}/blob/main/docs/` }) {
  const src = readText(file);
  const html = markdown(src.replace(/^# .+\n/m, ''), base);
  const toc = [...html.matchAll(/<h2 id="([^"]+)">(.*?)<\/h2>/g)].map(([, id, t]) => `<li><a href="#${id}">${t}</a></li>`).join('');
  return page({
    path: p,
    title,
    description,
    crumbs: [[h1, p]],
    body: `
<div class="wrap">
  <header class="doc-head"><p class="eyebrow">${esc(eyebrow)}</p><h1>${esc(h1)}</h1>${lede}</header>
  <div class="two"><article class="prose">${html}</article><nav class="toc" aria-label="On this page"><ul>${toc}</ul></nav></div>
</div>`,
  });
}

const privacyPage = () =>
  mdPage({
    path: '/privacy.html',
    file: path.join(DOCS, 'privacy.md'),
    title: 'Privacy Policy · Wisp Agents',
    description: 'What Wisp Agents, its phone apps, Wisp Agents Cloud and trywisp.dev collect, what stays on your computer, and how to delete your cloud account.',
    eyebrow: 'Legal',
    h1: 'Privacy policy',
  });

/** The product has no version in its name on the site: release notes written while it was being rebuilt say "v2" and
 * "as v1 did"; readers get "now" and "as before". Code (`action@v1`, `ref=v1`) is left alone. */
export function noVersionNames(md) {
  const keep = [];
  const out = md
    .replace(/`[^`\n]*`/g, (c) => `\u0001${keep.push(c) - 1}\u0001`)
    .replace(/\*\*v2:\s+(\S)/g, (_, c) => `**${c.toUpperCase()}`)
    .replace(/\b([Ii])n v2\b,?/g, (_, i) => (i === 'I' ? 'Now' : 'now'))
    .replace(/\b(?:the )?v2's\s+/g, 'the ')
    .replace(/\bthe v2\s+/g, 'the ')
    .replace(/,? as (?:in |for )?v1(?: did)?\b/g, ', as before')
    .replace(/\(v1 (#\d+)\)/g, '($1)')
    .replace(/\bv1 (#\d+)/g, '$1')
    .replace(/\bmatch v1\b/g, 'match the earlier look')
    .replace(/\bup with v1\b/g, 'up as before')
    .replace(/\bfrom v1\b/g, 'from an earlier wisp')
    .replace(/\bv1's\b/g, "the earlier wisp's")
    .replace(/\b(?:a|the) v1\b/g, 'an earlier wisp')
    .replace(/\b(for|in|to|on) v2\b/g, '$1 wisp')
    .replace(/\bv2\b\s*/g, '')
    .replace(/\bv1\b/g, 'the earlier wisp')
    .replace(/\u0001(\d+)\u0001/g, (_, i) => keep[i]);
  return out;
}

function changelogPage() {
  const src = noVersionNames(readText(path.join(ROOT, 'CHANGELOG.md')));
  // Released versions only: Unreleased describes main, not what `npx wisp-agents` gets. Before the first release
  // there are none: the page shows what's coming in it instead, under its own name.
  const all = src.split(/^(?=## \[)/m);
  const released = all.filter((s) => /^## \[\d/.test(s));
  const upcoming = all.find((s) => /^## \[Unreleased\]/.test(s));
  const releases = released.length || !upcoming ? released : [upcoming.replace(/^## \[Unreleased\][^\n]*/, '## [Coming in the first release]')];
  const section = (r) => {
    const [, version, said] = r.match(/^## \[([^\]]+)\](?:\s*-\s*(\S+))?/);
    // a date only when it is one (a section can say "- unreleased" until its release day)
    const date = /^\d{4}-\d{2}-\d{2}$/.test(said ?? '') ? said : undefined;
    const rest = r.replace(/^.*\n/, '').replace(/^\[[^\]]+\]: \S+[ \t]*$/gm, '');
    const id = `${/^\d/.test(version) ? 'v' : ''}${version.toLowerCase().replace(/[^a-z0-9.-]+/g, '-')}`;
    return { version, date, html: `<section class="release" id="${esc(id)}" style="padding:0">
      <h2><a href="#${esc(id)}">${esc(version)}</a>${date ? ` <time datetime="${esc(date)}">${esc(date)}</time>` : ''}</h2>
      ${markdown(rest)}
    </section>` };
  };
  const weeks = [];
  for (const r of releases.map(section)) {
    const d = r.date ? new Date(`${r.date}T00:00:00Z`) : null;
    if (d) d.setUTCDate(d.getUTCDate() - ((d.getUTCDay() + 6) % 7));
    const key = d ? d.toISOString().slice(0, 10) : '';
    if (weeks.at(-1)?.key !== key) weeks.push({ key, d, list: [] });
    weeks.at(-1).list.push(r);
  }
  const weekName = (d) => (d ? `Week of ${d.toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' })}` : 'Not released yet');
  const list = weeks.map((w, i) => {
    const span = w.list.length > 1 ? `${w.list.at(-1).version} – ${w.list[0].version}` : w.list[0].version;
    return `<details class="rel-week"${i === 0 ? ' open' : ''}>
      <summary><b>${esc(weekName(w.d))}</b><span class="muted">${esc(span)}</span></summary>
      ${w.list.map((r) => r.html).join('\n')}
    </details>`;
  }).join('\n');
  const latest = released[0]?.match(/^## \[([^\]]+)\]/)?.[1];
  const clDesc = `Release notes for Wisp Agents (wisp-agents on npm)${latest ? `, latest ${latest}` : ''}: new features, improvements and fixes.`;
  return page({
    path: '/changelog.html',
    title: 'Wisp Agents Changelog — Releases & Updates',
    description: clDesc,
    crumbs: [['Changelog', '/changelog.html']],
    schema: [{ '@type': 'WebPage', '@id': `${SITE}/changelog.html#page`, name: 'Wisp Agents Changelog', description: clDesc, url: `${SITE}/changelog.html`, isPartOf: { '@id': `${SITE}/#website` }, inLanguage: 'en' }],
    body: `
<div class="wrap">
  <header class="doc-head">
    <p class="eyebrow">Changelog</p>
    <h1>What’s new in Wisp Agents</h1>
    <p class="lede">Every release of <code>wisp-agents</code>, a week at a time, newest first. Update with <code>npm install -g wisp-agents</code>, or let the app update itself.</p>
  </header>
  <article class="prose">${list}</article>
</div>
<script>
function openRelease() { var el = location.hash && document.getElementById(location.hash.slice(1)); var wk = el && el.closest('details'); if (wk && !wk.open) { wk.open = true; el.scrollIntoView(); } }
addEventListener('hashchange', openRelease); openRelease();
</script>`,
  });
}

function notFoundPage() {
  return page({
    path: '/404.html',
    title: 'Page not found · Wisp Agents',
    description: 'This page doesn’t exist.',
    noindex: true,
    body: `
<div class="wrap">
  <header class="doc-head">
    <p class="eyebrow">404</p>
    <h1>Nothing here</h1>
    <p class="lede">That page doesn’t exist, or it moved. Try the <a href="/">home page</a>, the <a href="/docs/">docs</a>, the <a href="/docs/guides/">guides</a> or the <a href="/changelog.html">changelog</a>.</p>
  </header>
</div>`,
  });
}

/** An old address kept alive: the page says where it went, search engines follow the canonical link. */
function redirectPage(from, to, title) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<title>${esc(title)} · Wisp Agents</title>
<meta name="robots" content="noindex, follow" />
<link rel="canonical" href="${to.startsWith('http') ? to : `${SITE}${to}`}" />
<meta http-equiv="refresh" content="0; url=${esc(to)}" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<style>body{font:16px/1.6 system-ui,sans-serif;margin:3rem auto;max-width:36rem;padding:0 1rem}</style>
</head>
<body>
<p>This page moved: <a href="${esc(to)}">${esc(title)}</a>.</p>
<script>location.replace(${JSON.stringify(to)} + location.hash)</script>
</body>
</html>
`;
}

// ---------- llms.txt, robots.txt ----------

function llmsTxt() {
  return `# Wisp Agents

> Wisp Agents manages the code your agents write: an open-source app, terminal UI and CLI that runs Claude Code, Codex, Cursor CLI, opencode, GitHub Copilot CLI, Grok Build and Antigravity in parallel (each task in its own git worktree, on your own plans), lets you declare agents, crew, machines, skills and MCP servers as code in a repo's .wisp/ folder, and keeps every task next to the code it wrote (wisp blame, wisp resume). Desktop app for macOS, Windows and Linux; phone apps for iPhone and Android on a waitlist; Wisp Agents Cloud and self-hosted Grids for always-on agents.

Install: \`npm install -g wisp-agents\` (Node.js 18+), \`brew install --cask wispagents/tap/wisp\` (desktop app), or \`npx wisp-agents\` to try it. Every feature works from the desktop app, the phone app, \`wisp tui\` and the CLI, and from your own agent through \`wisp mcp\`.

## Docs

- [Docs index for agents](${SITE}/docs/llms.txt): every docs page as markdown, with a one-line summary
- [All docs in one file](${SITE}/docs/llms-full.txt)
- [Install](${SITE}/docs/start/install.md)
- [Your first task](${SITE}/docs/start/first-task.md)
- [CLI reference](${SITE}/docs/reference/cli.md)
- [MCP tools](${SITE}/docs/reference/mcp-tools.md)

## Pages

- [Home](${SITE}/): what Wisp Agents is, Declare · Run · Track
- [Get the app](${SITE}/app.html): desktop downloads, phone app waitlist
- [Wisp Agents Cloud](${SITE}/cloud.html): your own instance on a server; self-hosted Grids
- [Agents](${SITE}/agents/): how Wisp Agents runs each coding agent
${AGENTS.map((a) => `- [${a.name}](${SITE}/agents/${a.slug}.html): flags, modes, models`).join('\n')}
- [Use Wisp Agents from your agent](${SITE}/docs/integrations/your-agent.md): MCP server and skill
- [Changelog](${SITE}/changelog.html)
- [Privacy](${SITE}/privacy.html)

## Source

- [GitHub](${REPO}): MIT
- [npm: wisp-agents](https://www.npmjs.com/package/wisp-agents)
`;
}

const ROBOTS = `User-agent: *
Allow: /

Sitemap: ${SITE}/sitemap.xml
Sitemap: ${SITE}/docs/sitemap-index.xml
`;

// ---------- build ----------

function lastmod(files) {
  try {
    return execFileSync('git', ['log', '-1', '--format=%cs', '--', ...files], { cwd: ROOT, encoding: 'utf8' }).trim() || today();
  } catch {
    return today();
  }
}
const today = () => new Date().toISOString().slice(0, 10);

export function build(out = DOCS) {
  const pages = [
    { path: '/', file: 'index.html', html: landing(), sources: ['scripts/site.mjs', 'docs/site.css'], priority: '1.0' },
    { path: '/app.html', file: 'app.html', html: appPage(), sources: ['scripts/site.mjs'], priority: '0.9' },
    { path: '/cloud.html', file: 'cloud.html', html: cloudPage(), sources: ['scripts/site.mjs'], priority: '0.8' },
    { path: '/agents/', file: 'agents/index.html', html: agentsIndex(), sources: ['scripts/site.mjs'], priority: '0.8' },
    ...AGENTS.map((a) => ({ path: `/agents/${a.slug}.html`, file: `agents/${a.slug}.html`, html: agentPage(a), sources: ['scripts/site.mjs', `plugins/${a.id}`], priority: '0.8' })),
    // integrations live in the docs now (your agent, MCP, the reference); docs/integrations.md describes an older API
    { path: '/integrations.html', file: 'integrations.html', html: redirectPage('/integrations.html', '/docs/integrations/your-agent/', 'Use wisp from your agent or editor'), noindex: true },
    { path: '/changelog.html', file: 'changelog.html', html: changelogPage(), sources: ['CHANGELOG.md'], priority: '0.5' },
    { path: '/privacy.html', file: 'privacy.html', html: privacyPage(), sources: ['docs/privacy.md'], priority: '0.3' },
    { path: '/404.html', file: '404.html', html: notFoundPage(), noindex: true },
    // the old guides now live in the docs: each address sends its readers (and search engines) there
    { path: '/guides/', file: 'guides/index.html', html: redirectPage('/guides/', '/docs/guides/', 'Guides'), noindex: true },
    ...MOVED_GUIDES.map(([slug, to, title]) => ({ path: `/guides/${slug}.html`, file: `guides/${slug}.html`, html: redirectPage(`/guides/${slug}.html`, to, title), noindex: true })),
  ];
  for (const p of pages) {
    fs.mkdirSync(path.dirname(path.join(out, p.file)), { recursive: true });
    fs.writeFileSync(path.join(out, p.file), p.html);
  }
  fs.writeFileSync(path.join(out, 'llms.txt'), llmsTxt());
  fs.writeFileSync(path.join(out, 'robots.txt'), ROBOTS);
  const urls = pages.filter((p) => !p.noindex).map((p) => `  <url>\n    <loc>${SITE}${p.path}</loc>\n    <lastmod>${lastmod(p.sources)}</lastmod>\n    <priority>${p.priority}</priority>\n  </url>`);
  fs.writeFileSync(path.join(out, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join('\n')}\n</urlset>\n`);
  return pages;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const out = path.resolve(process.argv[2] || DOCS);
  const pages = build(out);
  console.log(`site: ${pages.length} pages + sitemap.xml (${pages.filter((p) => !p.noindex).length} urls), llms.txt, robots.txt → ${path.relative(process.cwd(), out) || '.'}`);
}
