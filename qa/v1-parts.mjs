// v1's PARTS contract (web/js/spell-schema.js, deleted with v1): the selector of every element v1 named, as v1
// rendered it. qa/parity.mjs maps each to the [data-part] v2 renders, to diff v2 against design/v1-reference/.

export const PARTS = {
  topbar: '#topbar',
  'topbar.brand': '#topbar .brand',
  'topbar.projects': '#projects',
  'topbar.machines': '#machinesBtn',
  // In a browser tab only: the invite to the desktop app, and its card.
  'topbar.getapp': '#getAppBtn',
  'getapp.card': '.getapp',
  'topbar.automations': '#autoBtn',
  'topbar.search': '#searchBtn',
  'palette.tags': '.palette .pal-tags', // a task's topics (wisp_tag) on its row in ⌘K
  switcher: '.switcher', // ⌃Tab's list of the tasks you were last on
  'switcher.row': '.switcher .pal-row',
  // You, the round avatar in the top bar's corner, and its menu: your account on the grid, then
  // Machines, Automations, Get the app and Settings (the topbar.* parts of those name its rows).
  'topbar.account': '#meBtn',
  'account.menu': '#meMenu',
  'topbar.settings': '#settingsBtn',
  'topbar.view': '#viewBtn',
  feed: '#feed',
  // The task list's sections; every heading folds its section.
  'section.needs': '#secNeeds',
  'section.working': '#secWorking',
  'section.scheduled': '#secScheduled', // tasks and reminders set for later, and done tasks with follow-ups to come
  'scheduled.row': '#secScheduled .plan-row',
  // A scheduled task (or reminder) opened from its row: its page, the box with its prompt, its details and actions.
  'plan.page': '.plan-view',
  'plan.prompt': '.plan-view .pv-box',
  'plan.details': '.plan-view .pv-side', // its details column (a bar over the box when narrow)
  'plan.actions': '.plan-view .pv-side .it-actions', // Start now, Change start time…, Delete
  'section.drafts': '#secDrafts', // tasks saved with /draft, not started yet
  'drafts.row': '#secDrafts .plan-row',
  'section.forLater': '#secForLater', // finished tasks (and crew jobs) saved to look at again
  'section.done': '#secDone',
  'section.heading': '#feed .f-group > h4',
  // The crew: named agents pinned above Working (both layouts), each row a member with its spirit;
  // a member's page, and the editor that adds one.
  'section.crew': '#secCrew',
  'crew.row': '#secCrew .cr-row',
  'crew.avatar': '.cr-av',
  'crew.strip': '#secCrew .cr-strip', // the quiet members folded into one row of faces
  // The crew rail beside the list (side by side): one tile a member, Org and + at the bottom. The
  // sidebar's crewRail prop turns it off for the section; hiding it (or section.crew) does too.
  'crew.rail': '#crewRail',
  'crew.rail.face': '#crewRail .cl-face',
  'crew.rail.all': '#crewRail .cl-all', // All, above the faces: every task in the list again
  'crew.rail.pick': '#crewRail .cl-pip', // the pip on the rail's edge beside the picked tile (All or a face)
  // A face's hover card: on the page, not in the rail (the rail would clip it).
  'crew.rail.card': '.cl-card',
  'crew.rail.card.head': '.cl-card .cl-head',
  'crew.rail.card.row': '.cl-card .cl-row',
  'crew.rail.card.all': '.cl-card .cl-more',
  // A member's thread: the main pane in split view, in place in the feed.
  'crew.page': '.crew-view',
  'crew.thread': '.crew-view .cv-thread',
  'crew.details': '.crew-view .cv-about',
  'crew.about': '.crew-view .cv-page', // the phone app's About page, swiped in over the thread
  'crew.compose': '.crew-view .cv-dock',
  'crew.editor': '.crew-edit',
  // Its skills and instructions files (the section in its details), and the editor for one.
  'crew.kit': '.crew-view .cv-kit',
  'crew.record': '.crew-record', // Track record: a member's stats (right-click → Track record…) or the whole crew's
  'crew.record.subject': '.crew-record .rec-subject', // its title: picks a member or the whole crew
  'crew.compare': '.crew-record .rec-compare', // the whole crew, ranked by usage
  'crew.kitEditor': '.kit-edit',
  // Its notebook, opened to read and write in full (Edit beside Notebook).
  'crew.notebookEditor': '.nb-edit',
  // Generate: the dialog that has a member build its kit, and the files it proposed, under that job.
  'crew.kitGenerate': '.kit-gen',
  'crew.offer': '.crew-view .cv-offer',
  'turn.kitOffer': '#feed .t-offers', // kit files an agent proposed for a crew member, under its answer
  'crew.editor.events': '.crew-edit .ce-events', // its "Also starts when" chips
  'crew.editor.tint': '.crew-edit .ce-tint', // its task panel tint: their glow, none, or a colour
  // Sharing (crew-share.js): Share (a file, JSON, a gist or a spell) and Import with its review. Not
  // the review a spell's member needs before it resumes, nor the note saying which spell it's from:
  // a spell mustn't hide what gates its own members.
  // Org (org.js): the whole crew as a chart of who hands work to whom, opened from the Crew heading.
  'crew.org': '#secCrew .cr-org, #crewRail .cl-org', // the Org button, on the Crew heading or the rail
  'org.page': '.org-view',
  'org.chart': '.org-view .org-chart',
  'org.card': '.org-view .org-card', // a member in the chart
  'org.details': '.org-view .org-side', // the column beside the chart: the selected member's delegates
  'org.reset': '.org-view .org-reset', // Reset layout, once cards were moved by hand
  // My Stuff (stuff.js): what you and your agents kept, opened from the box beside New task.
  'stuff.button': '#splitStuff', // the box beside New task that opens it
  'stuff.topButton': '#stuffTop', // in the phone app, the top bar button that opens it
  'stuff.page': '.stuff-view',
  'stuff.filters': '.stuff-view .sf-bar', // search, the kinds, Starred, By time / By task, grid or list
  'stuff.card': '.stuff-view .st-card', // one thing kept
  'stuff.save': '.stuff-save', // Save to My Stuff on a reply's picture, video, doc or code
  'stuff.launch': '.stuff-view .st-launch', // Launch on an app's card
  'stuff.makePane': '.stuff-view .sf-make', // Make a pane in its header
  'stuff.makePane.dialog': '.modal.pane-make', // the Make a pane dialog
  'stuff.public': '.stuff-view .st-pub', // Public on a published app's card (copies its link)
  'crew.share': '.crew-share',
  'crew.import': '.crew-import',
  'section.archived': '#secArchived',
  toTop: '#toTop',
  launcher: '#launcher',
  'launcher.input': '#prompt',
  'launcher.projects': '#launcher .l-projects',
  'launcher.for': '#launcher .l-for', // For Ada: who the prompt goes to, with a member picked on the rail
  'launcher.attach': '#attach',
  'launcher.mic': '#mic', // the phone app's Dictate button (features/dictate.js)
  'launcher.where': '#whereBtn',
  'launcher.mode': '#mode',
  'launcher.harness': '#runner',
  // Auto's preview on the harness pill (what it will run for this prompt), and its popover: the
  // whole panel, the other picks it weighed and the one it suggests trying.
  'launcher.autoPreview': '#autoPv',
  // A shared task's link pasted in the prompt box: its card (who shared it, what comes, Import).
  'launcher.share': '#launcher .l-share',
  'launcher.sharedNote': '#launcher .l-shared', // Shared from outside wisp: above what came in from the share sheet or another site
  // Share… on a task: the dialog (who can open it, its work, the link).
  'share.dialog': '.share-modal',
  'compare.table': '.compare-modal .cmp-table', // Compare: the runs' numbers side by side
  'compare.note': '.compare-modal .cmp-note', // Compare: the plan total and the sentence against the first run
  'compare.diff': '.compare-modal .cmp-diff', // Compare: what differed between a Try it two ways' runs
  'autoPreview.panel': '.auto-pop',
  'autoPreview.alternatives': '.auto-pop .ap-alt',
  'autoPreview.suggest': '.auto-pop .ap-sug',
  // Settings → Auto: the benchmark sources Auto starts from, and each row's Never / Prefer toggles.
  'router.sources': '.router-modal .ro-sources',
  'router.rules': '.router-modal .ro-rule',
  // Accounts (a harness with more than one subscription): the account on a harness · model button
  // (launcher, follow-up box), and the Account row under Thinking in the harness picker.
  'runner.account': '.rb-a',
  'picker.accounts': '.picker .pk-acct',
  // The Thinking slider in the harness picker (its fill and wisps grow with the level).
  'picker.thinking': '.picker .pk-think',
  'picker.vary': '.picker .pk-vary', // Try it two ways: the What's different group
  'launcher.compare': '#compareBtn',
  'launcher.team': '#teamBtn',
  'launcher.issue': '#launcher .issue-btn', // Issue: start from one of the project's open issues (GitHub, Linear)
  'issues.picker': '.palette.issues', // the list of issues it opens
  // The Delegate popover (launcher and follow-ups): the lead strip, and each delegate's place chip.
  'delegate.panel': '.team-pop',
  'delegate.lead': '.team-pop .tp-lead',
  'delegate.place': '.team-pop .tp-row .tp-place',
  // A task's delegates at work (one row each, a crew member's opens its job), their reports' link to
  // that job, and the line on a crew job saying which task it came from (a link back to it).
  'delegate.live': '.team-live .tl-row',
  'delegate.more': '.team-more', // Show N more, under a long team (live rows or reports)
  'delegate.waiting': '#feed .w-panel.team-wait', // the lead's turn ended, its delegates still at work: Waiting on them, with Stop
  'delegate.open': '.sub-task .sub-open',
  'task.from': '.ask .ask-from',
  'task.latest': '.latest-pill', // over the reply box: something landed while you were away from the end
  // A delegate's task opened by itself: the arrow on its report that opens it, its row under the
  // lead in the list, the note in place of its follow-up box, and the open task's trail of the
  // tasks above it (each opens that task).
  'delegate.go': '.sub .sub-go',
  'delegate.row': '#feed .item.is-sub',
  'delegate.dock': '.it-body .sub-dock',
  'split.trail': '#splitHead .sh-trail',
  'launcher.worktree': '#wtBtn',
  // Goal (Done when): the launcher's toggle and its line under the prompt.
  'launcher.goal': '#goalBtn',
  'launcher.goalLine': '#launcher .l-goal',
  'launcher.goalJudges': '#judgesBtn',
  // The open task's goal band: the condition, the progress track, the last check; the row's pill.
  'goal.band': '#feed .item.open .goal-band',
  'goal.text': '#feed .item.open .goal-band .gb-text',
  'goal.track': '#feed .item.open .goal-band .gb-track',
  'goal.check': '#feed .item.open .goal-band .gb-check',
  'goal.judges': '#feed .item.open .goal-band .gb-judges',
  'goal.checks': '.menu.jv-pop', // the popover with each judge's vote and the rounds before
  'goal.run': '.goal-run', // a goal check's page: what one judge was told, did and voted
  'goal.runHead': '.goal-run .gr-head',
  'goal.runVote': '.goal-run .gr-vote',
  'goal.runPrompt': '.goal-run .gr-prompt',
  'card.goal': '#feed .goal-pill',
  'launcher.send': '#launch',
  'launcher.later': '#launchLater',
  'launcher.more': '#launchMore', // More options: the rest of the bar, until the first task
  'launcher.when': '#launcher .l-when',
  'launcher.recipes': '#launcher .l-recipes',
  'launcher.bringIn': '#launcher .l-bring', // the offer to bring in a project's Conductor worktrees
  'launcher.bringInTake': '#launcher .l-bring-take',
  'launcher.bringInSkip': '#launcher .l-bring-skip',
  'launcher.fab': '#promptFab',
  composer: '.composer',
  'composer.input': '.composer .reply',
  'composer.attach': '.composer .c-attach',
  'composer.mic': '.composer .c-mic', // the phone app's Dictate button (features/dictate.js)
  'composer.attachMenu': '.menu.att-menu', // the phone app's attach menu: Take photo, Record video, Choose files
  'composer.mode': '.composer .c-mode',
  'composer.harness': '.composer .c-runner',
  'composer.place': '.composer .c-place',
  'composer.team': '.composer .c-team',
  'composer.goal': '.composer .c-goal',
  'composer.goalLine': '.composer .c-goal-line',
  'composer.hint': '.composer .c-hint',
  'composer.cold': '.composer .c-cold', // Cache gone cold: what a reply to a large idle task re-reads (web/js/cold-cache.js)
  'composer.coldFork': '.composer .c-cold-fork', // its Fork… button
  'hold.compact': '.hd-compact', // Compact now in the hold row
  'hold.compactNote': '.hd-compact-note', // its two prices
  'menu.compact': '.menu button[data-a="compact"]', // Compact now in the task's menus
  'turn.compact': '#feed .turn.compact-turn', // a Compact now turn in the task, open or folded
  'limit.cold': '.b-limit .lm-cold', // what a wait to the reset may re-read, on the usage-limit card under Wait until
  'composer.ship': '.composer .c-ship',
  'composer.done': '.composer .c-done',
  'composer.queue': '.composer .c-queue',
  // Later: the follow-ups scheduled on a task (rows above the box), Send later, the time read from the box.
  'composer.later': '.composer .cq.later',
  // Follow-ups queued for after the run (above the box, before the scheduled ones): edit, drag to reorder.
  'composer.queued': '.composer .cq:not(.later)',
  'composer.sendLater': '.composer .c-later',
  'composer.when': '.composer .when-chip',
  'composer.send': '.composer .send',
  'composer.btwTip': '.w-tip',
  // Comments on an answer (features/comments.js): the waiting ones over the box, the button on a
  // block, the bubble over selected words, the note card, the marks, and the sent ones on a prompt.
  'composer.comments': '.composer .cmt-tray',
  'composer.sendComments': '.composer .cmt-send',
  'answer.comment': '.cmt-add',
  'answer.commentBubble': '.cmt-bubble',
  'answer.commentCard': '.cmt-card',
  'answer.commentMark': '.cmt-mark',
  'turn.comments': '#feed .ask-notes',
  card: '#feed .item',
  'card.head': '#feed .it-head',
  'card.dot': '#feed .it-head .q-dot',
  'card.title': '#feed .it-title',
  'card.sub': '#feed .it-sub',
  'card.live': '#feed .q-live',
  'card.side': '#feed .it-side',
  'card.status': '#feed .it-side .pill[class*="st-"]', // Finished, Blocked, Failed, At limit, Signed out, Refused…
  'card.pr': '#feed .it-side .pill.pr',
  // Another open task changes the same files: the pill, and the warning above the follow-up box.
  'card.overlap': '#feed .it-side .pill.overlap',
  'card.overlapNote': '#feed .overlap-strip',
  'card.overlapClose': '#feed .overlap-strip .ov-x', // its × (closed until something changes)
  'card.model': '#feed .it-side .model-logo',
  'card.machine': '#feed .it-side .q-machine',
  'card.run': '#feed .it-run', // the row's third line: harness, model and machine
  'card.project': '#feed .it-side .q-proj',
  'card.time': '#feed .it-side .q-time',
  'card.later': '#feed .it-side .later-pill', // the next scheduled follow-up's time
  'card.quickDone': '#feed .it-quick',
  'card.quickLater': '#feed .it-later', // Save for later / Unsave beside Mark done
  'card.choices': '#feed .it-choices',
  'card.swipe': '#feed .it-swipe', // the band under a row being swiped in the phone app
  'card.body': '#feed .it-body',
  // Forks (Fork…): the fork's link to the task it came from, the original's count of its forks,
  // a turn's Fork from this turn button and the line where a fork's own turns start.
  'card.forkedFrom': '#feed .it-meta .m.fork-from',
  'card.forks': '#feed .it-meta .m.forks',
  // Side discussions (/side): a side discussion's link to the task it's about, and that task's count of them.
  'card.sideOf': '#feed .it-meta .m.side-of',
  'card.sides': '#feed .it-meta .m.sides',
  'card.sidePill': '#feed .it-head .pill.side-pill', // "Side" on a side discussion's row
  'turn.sideStart': '#feed .turn .side-line', // the line above a side discussion's first message
  'turn.sideWarn': '#feed .turn .side-warns', // a side discussion's rows: it touched a file the main task changed too
  'turn.whatChanged': '#feed .turn .wc-line', // the line saying what moved since the last turn: CLI version, tool set, Codex model catalog
  'turn.noOutput': '#feed .w-noout', // No output for 12 min, in the working panel of a run gone quiet
  'card.setupWarn': '#feed .it-meta .m.setup-warn', // a worktree's .worktreeinclude copy or setup step failed
  'mention.picker': '.menu.cr-pick', // @ in a prompt box: crew, My Stuff and files
  'form.select': '.kit-sel', // a list's button in dialogs and Settings (ui/select.js)
  'form.selectMenu': '.menu.kit-menu', // its options
  'mention.file': '.cr-pick .fl-pick', // a file in it
  'turn.fork': '#feed .tb-fork',
  // A turn's usage receipt in its trail bar (38k tokens · 71% cache · 3% of weekly), and Past your
  // plan when the run was billed beyond the plan (extra usage, Codex credits, an API key).
  'turn.receipt': '#feed .turn .tb-receipt',
  'turn.overage': '#feed .turn .tb-over',
  // …and where the tokens went: the subagent type that wrote most (3 web-fetch agents · Opus 5.5 · 27k of 41k output).
  'turn.agents': '#feed .turn .tb-agents',
  // Its offer to run subagents on Haiku when they wrote most of the output on the task's model.
  'turn.agents.switch': '#feed .turn .tb-sub-switch',
  // Subagents on Haiku: its row in Settings → Usage.
  'settings.subagentModel': '.sx-row:has(select[name="subagentModel"])',
  'turn.forkStart': '#feed .fork-line:not(.side-line)',
  // Split view only (body.split): the task list on one side, the open task on the other.
  sidebar: 'body.split .feed-wrap',
  // The top bar is the sidebar's head. The foot is gone: what it held (you on the grid, Get the app,
  // Settings) and the nav (Machines, Automations) are in the avatar's menu, so these name those.
  'sidebar.foot': '#meBtn',
  'sidebar.new': '#splitNew',
  'sidebar.nav': '#splitNav',
  'sidebar.account': '#splitMe',
  'sidebar.grip': '#splitGrip',
  reconnecting: '#reconnecting', // Reconnecting to wisp…: over the open pane while the page has lost wisp
  'split.main': '#splitMain',
  'split.home': '#splitHome',
  'split.greeting': '#splitHome .sh-hello',
  'split.next': '#splitHome .sh-next:not(.sh-coming)',
  'split.upcoming': '#splitHome .sh-coming', // Coming up: what runs next, soonest first
  'split.upcoming.more': '#splitHome .sh-coming .sh-more', // Show all / Show fewer, past the first `count`
  'split.upcoming.day': '#splitHome .sh-coming .sh-day', // a day's heading in the full list (Today, Tomorrow, Fri 31 Oct)
  // Add to Home Screen (web/js/features/homescreen.js): a row on home on a wisp Grid in an iPhone's browser, until added or hidden.
  'home.install': '#splitHome .hs-install',
  // Spare allowance (web/js/spare.js): a row under Needs you when a weekly window resets soon with much left.
  'split.panes': '#splitHome .sh-panes', // apps from My Stuff pinned on home (settings.panes "<app>@home")
  'split.pane': '#splitHome .sh-pane', // one of them: its bar and its frame
  'split.pane.bar': '#splitHome .sh-pane .pa-bar',
  'split.pane.grip': '#splitHome .sh-grip', // the handles between panes and under them: hide them and the sizes are the spell's
  'split.pane.row': '#splitHome .sh-pane-row', // a pane's row when stacked (a phone, a narrow window): it opens as a page
  'split.usage': '#splitHome .sh-usage', // usage at a glance under the prompt box: each plan's 5-hour and weekly windows
  'split.spare': '#splitHome .sh-spare',
  'split.spare.start': '#splitHome .sh-spare .spr-start', // Use <harness>: picks that harness and account in the prompt box
  // Window changes (web/js/window-notice.js): a row when a plan's usage window appears, goes, changes length or moves its reset.
  'split.windowNotice': '#splitHome .sh-spare .wn-card',
  'split.feedback': '#splitHome .sh-fb', // Hit a bug, or an idea: opens Send feedback (hidden when feedback is off)
  // Split view's home: tips under the prompt box (after Needs you), each linking to its guide on trywisp.dev.
  tips: '#splitHome .sh-tips',
  'tips.setup': '#splitHome .sh-tips .tip-setup', // pinned on top until a harness can run here; opens Settings → Agents
  'tips.grid': '#splitHome .sh-tips .tip-grid', // pinned until this computer is on a wisp Grid (opens Machines); on a cloud computer, its address until opened once
  'tips.tools': '#splitHome .sh-tips .tip-tool', // pinned until git and gh are installed and gh is signed in to GitHub; opens Settings → Agents
  'split.head': '#splitHead',
  'split.title': '#splitHead .sh-title',
  // Stacked (narrow windows, the phone app): Back to the list, on a task's page and on home.
  'split.back': '.split-main .sh-back',
  'split.body': 'body.split #feed .item.open > .it-body',
  // The open task's details column, fixed on the right while its body scrolls: what the card head
  // says (state, PR, harness, project, time), how it runs (branch, mode, …), its PR and its actions.
  'split.details': 'body.split #feed .item.open > .it-body > .it-bar',
  'split.pr': 'body.split #feed .item.open .it-bar > .pr-strip',
  // Review by another harness (setting review): the PR row's chip (Not reviewed, Reviewing, N issues,
  // Reviewed) and the PR panel's section with its findings and Review again.
  'split.pr.review': '.pr-strip .pr-rv',
  // The open task's ways into its shell: the action row's Shell button, and on a phone the row
  // under the PR's that opens it as a page (where it runs, and whether yours is still running).
  'split.shell': 'body.split #feed .item.open .it-actions > .it-shell',
  'split.shellDoor': 'body.split #feed .item.open .it-bar > .shell-door',
  'list.placeholder': '#feedSkel', // the list's rows before it's in (a slow first open)
  'split.placeholder': 'body.split #feed .item.open > .it-skel', // a task opened before its copy is here
  'split.watch': 'body.split #feed .item.open .it-actions > .it-watch',
  'split.changes': 'body.split #feed .item.open .it-actions > .it-changes',
  // Narrow, the row that pins at the top once the task's bar has scrolled away: the state, the
  // actions as icons, the PR as a chip and the shell.
  'split.mini': 'body.split #feed .item.open .it-mini > .im-row',
  // The account a task runs on (a harness with more than one): its initial on the harness logo.
  'details.account': 'body.split #feed .item.open .it-props .model-logo .acc-i',
  // The open task's usage, its subagents' included: its share of its plan's windows (6% of weekly ·
  // 21% of 5-hour) and how full its context is (62% context), lines that open all of it.
  'details.plan': 'body.split #feed .item.open .it-props > .plan-used',
  // …the context's line alone, with its ring…
  'details.context': 'body.split #feed .item.open .it-props > .ctx-used',
  // …and what the click opens: tokens, the plan's windows, the context and each subagent.
  'details.usage': '.menu.plan-menu',
  // A turn that hit its account's usage limit: the card under its hero (Wait until…, Continue on
  // another account, Hand to another harness, Get extra usage).
  // A turn its harness turned down (server/refusal.js): the card under its hero (Hand to <another
  // harness>, with the same prompt). Not a question: the task goes on without it, so it can be hidden.
  'turn.refusal': '#feed .turn .b-refusal',
  // A turn whose harness is signed out: the card under its hero. Not a question either: the task goes on without it.
  'turn.signIn': '#feed .turn .b-signin',
  'split.actions': 'body.split #feed .item.open .it-bar > .it-actions',
  // Take over in wisp: the task's terminal (a sheet, over the open task in split view, full
  // screen on a phone), its header, the screen and the phone's row of keys (Esc, Tab, Ctrl, arrows).
  term: '#term .tm-box',
  'term.head': '#term .tm-head',
  'term.screen': '#term .tm-screen',
  'term.keys': '#term .tm-keys',
  // The side panel beside the open task: its tabs (Changes, PR #412, Shell, Preview, Watch), the line
  // saying where the task's folder is (computer, branch, what's not shipped), the files the task
  // changed (their summary line and list), the shell in that folder, and the PR's parts. The
  // PR's verdict and Merge aren't parts: a spell can't hide what decides a merge.
  panel: '#sidePanel',
  'panel.tabs': '#sidePanel .pn-tabs',
  'panel.changes': '#sidePanel .pn-changes',
  'panel.changes.summary': '#sidePanel .cx-sum',
  'panel.changes.list': '#sidePanel .cx-list',
  'panel.changes.edit': '#sidePanel .cx-edit', // Edit here on a file's row
  'panel.changes.open': '#sidePanel .cx-find', // Open file…: any file of the task, in the editor
  'panel.editor': '#sidePanel .pn-edit', // the editor over Changes
  'panel.editor.code': '#sidePanel .ed-scroll',
  'panel.editor.save': '#sidePanel .ed-save',
  'panel.editor.problems': '#sidePanel .ed-probs', // the language server's problems count
  'panel.editor.hover': '#sidePanel .ed-hover', // what a name is, on hover
  // Comments on a diff's lines (features/comments.js): the + over a line under the pointer, and the
  // numbered mark on a commented line's gutter.
  'panel.changes.comment': '#sidePanel .cmt-line-add',
  'panel.changes.commentMark': '#sidePanel .cmt-mark.cmt-line',
  // Reviewing (features/changes.js): a hunk's Revert, a file's Viewed toggle, and "2 of 7 viewed".
  'panel.changes.revertHunk': '#sidePanel .cx-hk-rev',
  'panel.changes.viewed': '#sidePanel .cx-viewed',
  'panel.changes.viewedCount': '#sidePanel .cx-seen',
  'panel.changes.stage': '#sidePanel .cx-stage', // a file's Stage / Unstage toggle (+ / −), before Viewed
  'panel.changes.stageHunk': '#sidePanel .cx-hk-stage', // a hunk's Stage / Unstage, beside its Revert
  'panel.changes.commit': '#sidePanel .cx-commit', // the commit row under the summary: message, count, Commit
  'panel.place': '#sidePanel .pn-place',
  'panel.foreign': '#sidePanel .pn-foreign', // the line on a task brought in from another tool: that its folder stays
  'panel.shell': '#sidePanel .pn-shell',
  'panel.pr': '#sidePanel .pn-pr',
  'panel.pr.review': '#sidePanel .pn-xrv',
  'panel.pr.review.again': '#sidePanel .pn-xrv [data-a="xreview"]',
  // The review's list of what the user ruled out (and Done when) it checked the diff against, and
  // on a finding that breaks one, the user's words.
  'panel.pr.review.constraints': '#sidePanel .pn-xrv .xrv-cons',
  'panel.pr.review.coverage': '#sidePanel .pn-xrv .xrv-cover',
  'panel.pr.review.rule': '#sidePanel .pn-xrv .xrv-rule',
  'panel.checks': '#sidePanel .pn-checks',
  'panel.conversation': '#sidePanel .pn-talk',
  'panel.files': '#sidePanel .pn-files',
  'panel.pane': '#sidePanel .pn-pane', // an app pinned beside tasks (settings.panes "<app>@task"): its tab's page
  'panel.pane.tab': '#sidePanel .pn-tabs [data-tab^="pane:"]',
  'panel.pane.bar': '#sidePanel .pn-pane .pa-bar',
  'panel.watch': '#sidePanel .pn-watch',
  'panel.browser': '#sidePanel .pn-browser', // a sandboxed task's browser, live
  // Preview (features/run.js): the task's app run from its worktree. Its section, the bar over the
  // running app (address, Pick element, Logs, Restart, Stop), Run in the box that starts it, and its logs.
  'panel.preview': '#sidePanel .pn-run',
  'panel.preview.bar': '#sidePanel .rn-bar',
  'panel.preview.run': '#sidePanel .rn-go',
  'panel.preview.pick': '#sidePanel .rn-pick',
  'panel.preview.logs': '#sidePanel .rn-log',
  'watch.caption': '#sidePanel .wt-cap',
  'watch.run': '#sidePanel .wt-run',
  // Settings: the dialog, its list of sections, and the head with the pane's name and close button.
  'settings': '.modal.sx',
  'settings.nav': '.sx-nav',
  'settings.head': '.sx-top',
  // Settings → General: which releases wisp updates to (stable, beta or nightly).
  'settings.updates': '.sx-updates',
  // Settings → Machines: whether other devices can reach this computer over SSH.
  'settings.reachable': '.sx-reach',
  // Machines: the three-step explainer, and the password box when a machine won't let this one in.
  'machines.howto': '.mc-howto',
  'machines.letin': '.mc-letin',
  // wisp Grid in Machines: this computer on your grid, or signing in to one.
  'machines.cloud': '.mc-grid',
  'machines.grid.here': '.gh-row',
  'machines.grid.previews': '.gh-pv', // Show local pages in previews, under this computer on the grid
  // The pill on each machine saying where it comes from (Over SSH, Cloud, Your computer · via grid).
  'machines.kind': '.mc-kind',
  'phone.options': '.phone-modal .ph-options', // Phone → Telegram → Say what you'd type in wisp: its switch and line
  'phone.github': '.phone-modal .ph-gh', // Phone → GitHub: comment /wisp on an issue or PR
  'phone.slack': '.phone-modal .ph-sk', // Phone → Slack: its status, setup steps and who's linked
  'phone.discord': '.phone-modal .ph-dc', // Phone → Discord: its status, setup steps and who's linked
  'phone.discordUrl': '.phone-modal .ph-dc .ph-endpoint', // Phone → Discord, on a cloud computer: the Interactions Endpoint URL to copy
  'phone.email': '.phone-modal .ph-em', // Phone → Email, on a cloud computer: the secret address and who may send
  // Settings → Cloud: your cloud computers, and signing in to a wisp Grid.
  'settings.worktrees': '.wtr-box', // Settings → Worktrees
  'settings.worktrees.cleanup': '.wtr-auto', // Clean up automatically
  'settings.worktrees.list': '.wtr-list', // The worktrees on disk
  'settings.cloud': '.sx-cloud',
  'settings.cloud.computer': '.cg-card',
  'settings.cloud.signin': '.cg-signin',
  'settings.cloud.add': '.cg-add', // Add an account, once you have one (opens the sign-in)
  'settings.cloud.sandboxes': '.sbx-card', // Sandboxes: /sandbox's boxes and the Claude sign-in they start with
  'settings.cloud.keys': '.key-card', // API keys: for scripting a cloud wisp
  'settings.cloud.keys.reveal': '.key-new', // A new API key, shown once
  // Settings → Connect, on a cloud computer: how to open it through its grid.
  'settings.connect': '.sx-connect',
  // Settings → Agents: signing in to a harness (its link, code and paste box).
  'settings.agent.signin': '.sx-login',
  // Settings → Agents: a harness's accounts (each with its ⋯ menu) and Add another account.
  'settings.accounts': '.sx-accts',
  // Settings → Connectors: your MCP servers, each row, and the servers found in the harnesses' own configs.
  'settings.connectors': '.cn-box',
  'connectors.row': '.cn-row',
  'connectors.import': '.cn-found',
  // Settings → Usage: Plan budget, each account's 5-hour and weekly windows, Ask near the limit and Wait for the reset.
  'settings.budget': '.sx-budget',
  // …its Automations stop at row, and Where it went: the tasks that used most of an account's longest window.
  'settings.autoStop': '.sx-budget .sx-row:has(.us-stop)',
  'budget.where': '.sx-budget .bg-where',
  // …Where it went: subagents: the share of that window each subagent type took.
  'budget.subagents': '.sx-budget .bg-sub',
  // …Banked resets: a Codex account's reset credits and when the next one expires.
  'budget.banked': '.sx-budget .bg-bank',
  // …Past your plan: an account's runs billed beyond its plan in the last 7 days.
  'budget.overage': '.sx-budget .bg-over',
  // …tokens per turn by account: each account's median turn per model this week (several accounts only).
  'budget.compare': '.sx-budget .bg-cmp',
  // Settings → Usage → Machine load: the RAM/CPU guard's reading and its three knobs (server/load.js).
  'settings.load': '.sx-load',
  // This Mac (This PC, This computer; features/machine.js): the top-bar chip that shows when the
  // computer is short, the pane and each of its sections, and a heavy running task's RAM on its row.
  'machine.chip': '#machineChip',
  'machine.pane': '.mh-box',
  'machine.gauges': '.mh-box .mh-gauges', // Memory, CPU and Disk with the last hour
  'machine.using': '.mh-box .mh-using', // What's using it: running tasks and crew members, Stop and Later
  'machine.leftBehind': '.mh-box .mh-left', // processes still running after their task finished
  'machine.freeUp': '.mh-box .mh-free', // Free up space: folders wisp can give back
  'machine.automations': '.mh-box .mh-auto', // Pause automations / Resume now and the limits
  'machine.day': '.mh-box .mh-day', // Last 24 hours
  'card.ram': '.item .q-res',
  // …a harness's own Automations stop at row, the Drain watch row, and its card on an account.
  'settings.autoStopBy': '.sx-budget .us-by-row',
  'settings.drainWatch': '.sx-budget .sx-row:has(#drainWatch)',
  // Harness watch (server/harness-watch.js): the line a task and Settings → Agents show when a
  // harness's program changed since wisp pinned it (with Trust this version), and the Ask before
  // running a changed harness row.
  'harness.change': '.hw-note',
  'settings.harnessAsk': '.sx-row:has(#harnessAsk)',
  // Lean Claude Code runs (server/lean.js): its row in Settings → Agents.
  'settings.leanRuns': '.sx-row:has(select[name="leanRuns"])',
  'settings.review': '.sx-row:has(select[name="review"])',
  // .env masking (server/env-mask.js): the line over a masked .env diff or command output, its
  // Reveal / Hide button, and the Hide .env values row in Settings → Privacy.
  'env.masked': '.env-mask',
  'env.reveal': '.env-mask .env-reveal',
  'settings.maskEnvFiles': '.sx-row:has(#maskEnv)',
  // Project trust (server/project-trust.js): the Ask before a repo's hooks, servers and setup run row,
  // and the line on a turn that ran without them. The card that waits for you is not a part.
  'settings.projectAsk': '.sx-row:has(#projectAsk)',
  'project.without': '.pt-without',
  // The right-click menu on a project (top bar, New task, the +N list) and its Edit project dialog.
  'project.menu': '.menu.project',
  'project.edit': '.project-edit',
  // The searchable list of every project (+N in the top bar or New task, a crew member's Works in).
  'project.list': '.menu.pm',
  // The Projects dialog's list (Manage projects), and the handle each row is dragged by.
  'project.rows': '.projects-modal .plist',
  'project.grip': '.projects-modal .pgrip',
  'budget.drain': '.sx-budget .bg-drain',
  // …and the Check Codex while work waits row.
  'settings.liveLimits': '.sx-budget .sx-row:has(#liveLimits)',
  'settings.webhookTriggers': '.sx-row:has(#hookTriggers)',
  'settings.acceptChannels': '.sx-row:has(#acceptChannels)',
  'settings.spareCard': '.sx-budget .sx-row:has(#spareCard)',
  'settings.windowNotice': '.sx-budget .sx-row:has(#windowNotice)',
  'settings.windowNotice.line': '.sx-budget .bg-note', // the line in an account's block, with ×
  // A hook trigger's editor (features/automations.js): its URL with Copy and Regenerate, and its last deliveries.
  'automations.hookUrl': '.auto-modal .au-hook',
  'automations.deliveries': '.auto-modal .au-deliv',
  'automations.slackPeople': '.auto-modal .k-people', // a Slack trigger's People: who may start it
  // The harness picker: how full a window at or past your Ask near the limit line is (server/guard.js).
  // The hold row and the held card aren't parts: they need you, like a due reminder.
  'harness.usage': '.rp-usage',
  // The image viewer: a picture from a reply or an attachment, full screen to zoom into, and its zoom buttons.
  viewer: '.lb',
  'viewer.bar': '.lb-bar',
  // Its previous / next buttons, when the reply has more than one picture.
  'viewer.nav': '.lb-nav',
  // The Copy image, Download and Show in folder buttons on a picture in a reply (on hover; always there on a touch screen).
  'image.tools': '.b-image .img-tools',
  // A video in a reply: the player, and its Download and Show in folder buttons.
  video: '.b-video',
  'video.tools': '.b-video .img-tools',
  // A live web page in a reply, framed as a phone, tablet or browser window: its address and size
  // switcher, the stage it sits on (max-height sets how tall it may get), and the line under it.
  preview: '.b-preview',
  'preview.head': '.b-preview .pv-head',
  'preview.stage': '.b-preview .pv-stage',
  'preview.foot': '.b-preview .pv-foot',
  // Places on a Google map in a reply: the card, its title row (with Route), the map (minHeight makes it taller),
  // and the list of places with Open in Google Maps.
  map: '.b-map',
  'map.head': '.b-map .mp-head',
  'map.stage': '.b-map .mp-stage',
  'map.list': '.b-map .mp-list',
  // A markdown file in a reply (the doc block): the card, its page of the file's opening (max-height
  // sets how much shows), and the full-screen reader with its contents column.
  doc: '.b-doc',
  'doc.preview': '.b-doc .doc-page',
  'doc.reader': '.dr',
  'doc.contents': '.dr-toc',
  // Another wisp task in a reply (the task block): the card, and its Open label.
  task: '.b-task',
  'task.open': '.b-task .tk-open',
  'stuff.block': '.b-stuff', // something from My Stuff in an answer
  'stuff.block.go': '.b-stuff .sb-go', // its Launch / Open
  'stuff.block.public': '.b-stuff .sb-pub', // a published app's public link, with Copy link
  'stuff.frame': '.ap', // an app running inside wisp, over the page
  'stuff.frame.publish': '.ap .ap-pub', // its Publish / Update link / Copy link
  // An interactive block in an answer (an agent's or a spell's app) and its title row. Not the
  // strip that asks before sending what the app offers: that one always shows.
  'block.app': '.b-app',
  'block.app.head': '.b-app .app-head',
  // Toasts: the stack, each card, its glyph, its action button, the ring that runs down while it
  // shows, and Clear all over the fanned-out stack.
  toasts: '#toasts',
  toast: '#toasts .toast',
  'toast.glyph': '#toasts .t-glyph',
  'toast.action': '#toasts .t-act',
  'toast.timer': '#toasts .t-time',
  'toast.clear': '#toasts .toasts-clear',
};
