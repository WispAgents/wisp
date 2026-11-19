// The guides moved to the docs (trywisp.dev/docs/guides, ADR-0039: the marketing site links into the docs and stops
// carrying guides). The addresses people, search engines and older wisps link to stay alive: scripts/site.mjs writes
// each one as a page that sends its reader to where that topic lives now. Never drop a row: a link out there breaks.
//
//   [old slug under /guides/, where it lives now, that page's title]

const G = '/docs/guides';

export const MOVED_GUIDES = [
  ['run-claude-code-and-codex-in-parallel', `${G}/run/run-and-follow-up/`, 'Run a task and follow up'],
  ['run-coding-agents-on-multiple-machines', `${G}/run/machines/`, 'Machines, the Grid and wisp Cloud'],
  ['connect-machines-with-tailscale', `${G}/run/machines/`, 'Machines, the Grid and wisp Cloud'],
  ['move-a-task-to-another-machine-or-agent', `${G}/run/machines/`, 'Machines, the Grid and wisp Cloud'],
  ['one-board-for-all-your-coding-agents', '/docs/start/first-task/', 'Your first task'],
  ['run-multiple-claude-code-agents-in-parallel', `${G}/agents/claude-code/`, 'Start with Claude Code'],
  ['claude-code-vs-codex', `${G}/run/compare-and-try/`, 'Compare agents and try it two ways'],
  ['multi-agent-coding', `${G}/run/crew-and-check-ins/`, 'Crew and check-ins'],
  ['run-coding-agents-in-the-background', `${G}/run/run-and-follow-up/`, 'Run a task and follow up'],
  ['moving-off-jules', '/docs/start/install/', 'Install wisp'],
  ['moving-off-conductor', '/docs/start/install/', 'Install wisp'],
  ['make-claude-code-keep-going-until-tests-pass', `${G}/run/goals/`, 'Goals and judges'],
  ['review-claude-code-prs-with-codex', `${G}/run/review-by-another-agent/`, 'Review by another agent'],
  ['run-claude-code-24-7-without-a-vps', '/cloud.html', 'wisp Cloud'],
  ['use-wisp-cloud', '/cloud.html', 'wisp Cloud'],
  ['reach-your-computer-from-your-phone', `${G}/apps/phone-app/`, 'wisp on your phone'],
  ['share-to-wisp-from-your-phone', `${G}/apps/phone-app/`, 'wisp on your phone'],
  ['claude-code-remote-control-alternative', `${G}/apps/phone-app/`, 'wisp on your phone'],
  ['use-codex-from-your-phone', `${G}/apps/phone-app/`, 'wisp on your phone'],
  ['remote-coding-agent-apps-compared', '/app.html', 'Get the wisp app'],
  ['self-host-wisp-grid', `${G}/run/machines/`, 'Machines, the Grid and wisp Cloud'],
  ['customize-wisp-with-spells', '/docs/plugins/make-wisp-your-own/', 'Make wisp your own'],
  ['use-work-and-personal-claude-code-accounts', `${G}/agents/claude-code/`, 'Start with Claude Code'],
  ['several-claude-code-accounts-and-computers', `${G}/agents/claude-code/`, 'Start with Claude Code'],
  ['see-where-your-claude-code-usage-went', `${G}/run/budgets-and-usage/`, 'Budgets and usage limits'],
  ['share-crew-members', `${G}/run/crew-and-check-ins/`, 'Crew and check-ins'],
  ['keep-what-your-coding-agents-make', `${G}/track/track-agent-work/`, 'Track your agents’ work beside your code'],
  ['publish-an-app-your-agent-built', '/cloud.html', 'wisp Cloud'],
  ['organize-your-ai-agent-crew', `${G}/run/crew-and-check-ins/`, 'Crew and check-ins'],
  ['conductor-alternatives', '/', 'wisp'],
  ['wisp-vs-orca', '/', 'wisp'],
  ['wisp-vs-t3-code', '/', 'wisp'],
  ['wisp-vs-paseo', '/', 'wisp'],
  ['wisp-vs-cmux', '/', 'wisp'],
  ['wisp-vs-multica', '/', 'wisp'],
  ['wisp-vs-paradev', '/', 'wisp'],
  ['wisp-vs-claude-code-projects', '/', 'wisp'],
];
