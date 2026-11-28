// Settings → Automations → Manage (audit A #8, #9): a saved trigger opens v1's form on its own kind, an edit keeps
// what the form doesn't show and the secret it isn't shown, and each row reads as v1 described it.
import { expect, test } from 'vitest';
import { describeSpec, emptyForm, fireLabel, formOfSpec, keyOfSpec, pick, specOfEdit, specOfForm, switchedSpec } from '../src/trigger-form.ts';

const o = { place: (p: string) => p.split('/').pop() ?? p, member: (id: string) => (id === 'm1' ? 'Ada' : undefined), words: (c: string) => (c === '0 9 * * 1-5' ? 'Every weekday at 9:00' : null) };

test('a schedule opens on its tile with its when and project; saving it again gives the same spec_audit_A_8', () => {
  const spec = { on: 'schedule', prompt: 'Tidy', project: '/code/web', mode: 'auto', cron: '0 9 * * 1-5' };
  const f = formOfSpec('morning', spec);
  expect(f).toMatchObject({ key: 'schedule', name: 'morning', when: '0 9 * * 1-5', project: '/code/web', prompt: 'Tidy', mode: 'auto' });
  expect(f && specOfEdit(f, { name: 'morning', spec })).toEqual({ name: 'morning', spec });
});

test('an edit keeps the fields only a formula sets, drops what the form emptied, and keeps the secret left blank_audit_A_8', () => {
  const spec = { on: 'webhook', prompt: 'Look', secret: '${secret:HOOK}', header: 'Sentry-Hook-Signature', debounce: '10m', rate: '5/h', branch: 'old', enabled: false };
  const f = formOfSpec('alerts', spec);
  expect(f?.key).toBe('webhook:sentry');
  expect(f?.secret).toBe(''); // never put back on screen
  const out = f && specOfEdit({ ...f, prompt: 'Look again' }, { name: 'alerts', spec });
  expect(out).toEqual({ name: 'alerts', spec: { on: 'webhook', prompt: 'Look again', secret: '${secret:HOOK}', header: 'Sentry-Hook-Signature', mode: 'plan', debounce: '10m', rate: '5/h', enabled: false } });
  // a new secret typed replaces it
  expect(f && specOfEdit({ ...f, secret: 'new' }, { name: 'alerts', spec })).toMatchObject({ spec: { secret: 'new' } });
  // the name stays the trigger's: a rename would leave the old one running beside it
  expect(f && specOfEdit({ ...f, name: 'renamed' }, { name: 'alerts', spec })).toMatchObject({ name: 'alerts' });
});

test('every tile comes back as itself; a source only a formula declares has no form_audit_A_8', () => {
  const tiles = [
    { name: 'a', key: 'schedule', when: 'every hour', prompt: 'x' },
    { name: 'a', key: 'ci', branch: 'main', prompt: 'x' },
    { name: 'a', key: 'issues', label: 'bug', prompt: 'x' },
    { name: 'a', key: 'review', prompt: 'x' },
    { name: 'a', key: 'finished', prompt: 'x' },
    { name: 'a', key: 'done', crew: 'm1', prompt: 'x' },
    { name: 'a', key: 'webhook', secret: 's', prompt: 'x' },
    { name: 'a', key: 'slack', users: 'U1', keyword: 'deploy', prompt: 'x' },
    { name: 'a', key: 'linear', secret: 's', label: 'wisp', prompt: 'x' },
    { name: 'a', key: 'webhook:pagerduty', secret: 's', prompt: 'x' },
  ];
  for (const t of tiles) {
    // as the form makes it: the tile picked (its preset's header), then its fields
    const made = specOfForm({ ...pick(emptyForm(), t.key), ...t });
    expect('spec' in made && keyOfSpec(made.spec), t.key).toBe(t.key);
  }
  expect(formOfSpec('x', { on: 'pr.opened', prompt: 'review it' })).toBeUndefined();
  expect(keyOfSpec({ on: 'chat.message', match: { channel: 'discord' } })).toBeUndefined();
});

test('each row reads as v1 described it, and only a schedule or a watcher has Run now_audit_A_8', () => {
  expect(describeSpec({ on: 'schedule', cron: '0 9 * * 1-5', project: '/code/web' }, o)).toBe('Every weekday at 9:00 in web');
  expect(describeSpec({ on: 'task.done', crew: 'm1' }, o)).toBe('When a task in Ada’s project is marked done → Ada');
  expect(describeSpec({ on: 'task.finished', project: '/code/api' }, o)).toBe('When a task in api finishes, a new task in its folder');
  expect(describeSpec({ on: 'ci.failed', branch: 'main', project: '/code/api', every: 15 }, o)).toBe('Watches failed CI on main in api every 15 min');
  expect(describeSpec({ on: 'webhook', header: 'Linear-Signature', match: { 'data.labels.name': 'wisp' }, project: '/code/api' }, o)).toBe('When a Linear issue is created with the label “wisp”, in api');
  expect(describeSpec({ on: 'pr.opened', project: '/code/api' }, o)).toBe('On pr.opened in api');
  expect(fireLabel({ on: 'schedule' })).toBe('run');
  expect(fireLabel({ on: 'manual' })).toBe('run');
  expect(fireLabel({ on: 'issue.labeled' })).toBe('check');
  expect(fireLabel({ on: 'task.done' })).toBeUndefined();
  expect(fireLabel({ on: 'webhook' })).toBeUndefined();
});

test('the switch turns a trigger off and back on: enabled is sent either way, since apply keeps a field left out_audit_G3', () => {
  const off = { on: 'schedule', cron: '* * * * *', prompt: 'Tidy', enabled: false };
  expect(switchedSpec(off, true)).toEqual({ ...off, enabled: true });
  expect(switchedSpec({ on: 'schedule', cron: '* * * * *', prompt: 'Tidy' }, false)).toMatchObject({ enabled: false });
});

test('an issue trigger takes only my issues, the same match the CLI and a formula write, and an edit keeps a formula’s own filter', () => {
  const base = { ...pick(emptyForm('/code/api'), 'issues'), name: 'mine', label: 'agent', prompt: 'Fix it' };
  // `wisp trigger add mine issue.labeled --label agent --assignee me` writes this same spec
  expect(specOfForm({ ...base, whose: 'assigned' })).toMatchObject({ spec: { on: 'issue.labeled', label: 'agent', match: { assignees: 'me' } } });
  expect(specOfForm({ ...base, whose: 'opened' })).toMatchObject({ spec: { match: { author: 'me' } } });
  const anyone = specOfForm(base);
  expect('spec' in anyone && anyone.spec['match']).toBeUndefined();
  // a saved one opens on its choice; `assignee: me` (the first assignee) reads as Assigned to me too
  expect(formOfSpec('mine', { on: 'issue.labeled', label: 'agent', match: { assignees: 'me' } })?.whose).toBe('assigned');
  expect(formOfSpec('mine', { on: 'issue.labeled', label: 'agent', match: { assignee: 'me' } })?.whose).toBe('assigned');
  expect(formOfSpec('mine', { on: 'issue.labeled', label: 'agent', match: { author: ['me', 'dependabot'] } })?.whose).toBe('');
  // an edit to Anyone's drops the me it showed, and keeps what only a formula said
  const spec = { on: 'issue.labeled', label: 'agent', prompt: 'Fix it', match: { assignees: 'me', author: ['me', 'dependabot'] } };
  const f = formOfSpec('mine', spec);
  expect(f && specOfEdit({ ...f, whose: '' }, { name: 'mine', spec })).toMatchObject({ spec: { match: { author: ['me', 'dependabot'] } } });
  const cleared = f && specOfEdit({ ...f, whose: '' }, { name: 'mine', spec: { ...spec, match: { assignees: 'me' } } });
  expect(cleared && 'spec' in cleared && cleared.spec['match']).toBeUndefined();
  expect(f && specOfEdit({ ...f, whose: 'opened' }, { name: 'mine', spec: { ...spec, match: { assignees: 'me' } } })).toMatchObject({ spec: { match: { author: 'me' } } });
  expect(describeSpec({ on: 'issue.labeled', label: 'agent', project: '/code/api', match: { assignees: 'me' } }, o)).toBe('Watches my assigned issues labelled agent in api every 5 min');
});

test('a review trigger wakes only the teammate asked: Asked of me is match.reviewer: me, as `--reviewer me` writes it', () => {
  const base = { ...pick(emptyForm('/code/api'), 'review'), name: 'my-reviews' };
  expect(base.mode).toBe('plan'); // PR text comes from outside
  const made = specOfForm({ ...base, whose: 'asked' });
  expect(made).toMatchObject({ spec: { on: 'pr.review_requested', project: '/code/api', every: 5, match: { reviewer: 'me' } } });
  expect('spec' in made && String(made.spec['prompt'])).toContain('gh pr diff {event.payload.number}'); // no prompt: the default one
  expect(formOfSpec('my-reviews', { on: 'pr.review_requested', match: { reviewer: 'me' } })).toMatchObject({ key: 'review', whose: 'asked' });
  // an issue choice doesn't carry over to a review trigger
  expect(pick({ ...emptyForm(), key: 'issues', whose: 'assigned' }, 'review').whose).toBe('');
  expect(specOfForm({ ...base, whose: 'assigned' })).not.toHaveProperty('spec.match');
  // an edit keeps a formula's own filter beside the me it shows
  const spec = { on: 'pr.review_requested', prompt: 'Review', match: { reviewer: 'me', base: 'main' } };
  const f = formOfSpec('my-reviews', spec);
  expect(f && specOfEdit({ ...f, whose: '' }, { name: 'my-reviews', spec })).toMatchObject({ spec: { match: { base: 'main' } } });
  expect(describeSpec({ on: 'pr.review_requested', project: '/code/api', match: { reviewer: 'me' } }, o)).toBe('Watches review requests for me in api every 5 min');
  expect(fireLabel({ on: 'pr.review_requested' })).toBe('check');
});

test('the filters the CLI writes come from the app too, back into the form, and an edit keeps the rest', () => {
  const base = { ...pick(emptyForm('/code/api'), 'review'), name: 'r' };
  // `--reviewer me --from trusted --no-drafts --paths src/**,!**/*.md --max-size 400 --milestone v2.1`
  const made = specOfForm({ ...base, whose: 'asked', from: 'trusted', skipDrafts: true, paths: 'src/**, !**/*.md', size: '400', milestone: 'v2.1' });
  const match = { reviewer: 'me', trusted: true, draft: false, paths: ['src/**', '!**/*.md'], size: '<=400', milestone: 'v2.1' };
  expect(made).toMatchObject({ spec: { match } });
  const spec = { on: 'pr.review_requested', project: '/code/api', prompt: 'Review', match: { ...match, labels: '!wip' } };
  const f = formOfSpec('r', spec);
  expect(f).toMatchObject({ whose: 'asked', from: 'trusted', skipDrafts: true, paths: 'src/**, !**/*.md', size: '400', milestone: 'v2.1' });
  // clearing every filter keeps only what the form never showed
  const cleared = f && specOfEdit({ ...f, whose: '', from: '', skipDrafts: false, paths: '', size: '', milestone: '' }, { name: 'r', spec });
  expect(cleared && 'spec' in cleared && cleared.spec['match']).toEqual({ labels: '!wip' });
  // a size the form has no step for (<=250) is a formula's: kept, not shown
  expect(formOfSpec('r', { ...spec, match: { size: '<=250' } })?.size).toBe('');
  // an issue trigger has From and Milestone, not a review's drafts, paths or size
  const issue = specOfForm({ ...pick(emptyForm('/code/api'), 'issues'), name: 'i', label: 'bug', prompt: 'x', from: 'people', milestone: 'v2.1', skipDrafts: true, size: '100' });
  expect('spec' in issue && issue.spec['match']).toEqual({ bot: false, milestone: 'v2.1' });
  expect(describeSpec(spec, o)).toBe('Watches review requests for me, from people who may push, not drafts, up to 400 lines, touching src/**, !**/*.md, in v2.1 in api every 5 min');
  expect(describeSpec({ on: 'issue.labeled', label: 'bug', project: '/code/api', match: { bot: false } }, o)).toBe('Watches issues labelled bug not from bots in api every 5 min');
});
