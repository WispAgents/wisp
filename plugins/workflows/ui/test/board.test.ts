// ADR-0119's band: a workflow run as the board draws it. Phases become lanes in order, a foreach becomes one fan card
// with a dot per item, a small matrix stays a card per run, each card says how long it took and what it gave, and the
// header says where the run is, then how it ended. `/workflow` is offered in the `/` menu with the saved ones.

import { describe, expect, it } from 'vitest';
import ui from '../src/index.ts';
import '../src/i18n/lazy.ts';
import { asFormula, detailOf, duration, lanes, line, money, pill, runsOf, share, stepOf } from '../src/model.ts';
import { suggestions } from '../src/suggest.ts';
import type { RunView, StepView } from '../src/types.ts';

const step = (over: Partial<StepView> & Pick<StepView, 'id' | 'step' | 'state'>): StepView => ({ needs: [], ...over });

const run = (over: Partial<RunView> = {}): RunView => ({
  id: 'r1',
  workflow: 'audit',
  state: 'running',
  cost: 0.42,
  budget: 5,
  startedAt: 1_000,
  done: 3,
  total: 7,
  phases: ['Explore', 'Build', 'Verify'],
  spec: { tasks: { plan: { prompt: 'List the routes' } } },
  steps: [
    step({ id: 'plan', step: 'plan', title: 'Map the routes', phase: 'Explore', state: 'done', task: 't1', agent: 'claude', said: '12 routes', startedAt: 1_000, endedAt: 31_000, cost: 0.12 }),
    step({ id: 'fix[1]', step: 'fix', title: 'Fix a route', phase: 'Build', state: 'done', task: 't2', item: '/login', startedAt: 31_000, endedAt: 50_000 }),
    step({ id: 'fix[2]', step: 'fix', title: 'Fix a route', phase: 'Build', state: 'running', task: 't3', item: '/logout', startedAt: 31_000 }),
    step({ id: 'fix[3]', step: 'fix', title: 'Fix a route', phase: 'Build', state: 'failed', task: 't4', item: '/me', why: 'it ended error' }),
    step({ id: 'fix[4]', step: 'fix', title: 'Fix a route', phase: 'Build', state: 'waiting', item: '/admin' }),
    step({ id: 'review[agent=claude]', step: 'review', phase: 'Verify', state: 'waiting', matrix: { agent: 'claude' }, agent: 'claude', needs: ['fix'] }),
    step({ id: 'review[agent=codex]', step: 'review', phase: 'Verify', state: 'waiting', matrix: { agent: 'codex' }, agent: 'codex', needs: ['fix'] }),
  ],
  ...over,
});

describe('the workflow board', () => {
  it('registers the band on the task page', () => {
    expect(ui.extensions?.map((e) => e.attachTo.id)).toEqual(['shell.task:band']);
  });

  it('lays phases out as lanes, a foreach as one fan and a small matrix as a card per run', () => {
    const opened: string[] = [];
    const l = lanes(run(), 61_000, (t) => opened.push(t));
    expect(l.map((x) => [x.title, x.count, x.state])).toEqual([
      ['Explore', '1/1', 'done'],
      ['Build', '2/4', 'running'],
      ['Verify', '0/2', 'waiting'],
    ]);
    const plan = l[0]?.entries[0];
    expect(plan && 'step' in plan && plan.step).toMatchObject({ title: 'Map the routes', agent: 'claude', meta: '30s · $0.12', detail: '12 routes' });
    const fan = l[1]?.entries[0];
    if (!fan || !('fan' in fan)) throw new Error('fix is a fan');
    expect(fan.fan).toMatchObject({ title: 'Fix a route', count: '2/4', state: 'running', detail: '1 failed' });
    expect(fan.fan.items.map((i) => [i.title, i.state, i.meta ?? ''])).toEqual([
      ['/login', 'done', '19s'],
      ['/logout', 'running', '30s'],
      ['/me', 'failed', ''],
      ['/admin', 'waiting', ''],
    ]);
    fan.fan.items[1]?.onOpen?.();
    expect(opened).toEqual(['t3']);
    expect(fan.fan.items[3]?.onOpen, 'no task yet: nothing to open').toBeUndefined();
    const verify = l[2]?.entries.map((e) => ('step' in e ? [e.step.title, e.step.detail] : []));
    expect(verify).toEqual([
      ['review · claude', 'After fix'],
      ['review · codex', 'After fix'],
    ]);
  });

  it('a run without phases is one lane with no header', () => {
    const l = lanes(run({ steps: [step({ id: 'a', step: 'a', state: 'waiting' })] }), 0, () => undefined);
    expect(l.map((x) => x.title)).toEqual(['']);
  });

  it('says where the run is while it goes, then how it ended', () => {
    expect(pill(run())).toEqual({ text: 'Build · 3/7', tone: 'accent', live: true });
    expect(line(run(), 61_000)).toBe('1m 00s · $0.42 of $5.00');
    expect(share(run())).toBeCloseTo(3 / 7);
    const done = run({ state: 'done', done: 7, endedAt: 192_000, budget: null });
    expect(pill(done)).toEqual({ text: 'Done', tone: 'good', live: false });
    expect(line(done, 999_999)).toBe('3m 11s · $0.42 · 7 steps');
    expect(pill(run({ state: 'stopped' })).tone).toBe('neutral');
  });

  it('a foreach not made yet says what it waits on', () => {
    expect(detailOf(step({ id: 'fix', step: 'fix', state: 'waiting', fanout: true, needs: ['plan'] }))).toBe('Fans out once plan ends');
    expect(detailOf(step({ id: 'x', step: 'x', state: 'running', attempts: 2 }))).toBe('Working, try 2…');
  });

  it('shows a step whose turn is held as waiting, with why', () => {
    const held = step({ id: 'fix', step: 'fix', state: 'running', task: 't1', held: "claude's limit resets at 14:00" });
    expect(detailOf(held)).toBe("Waiting: claude's limit resets at 14:00");
    const card = lanes(run({ phases: [], steps: [held] }), 0, () => undefined)[0]?.entries[0];
    expect(card && 'step' in card ? card.step.state : undefined).toBe('waiting');
  });

  it('reads a synced row and a step task', () => {
    expect(runsOf({ id: 'h:1', summary: { workflowRuns: [run()] } })).toHaveLength(1);
    expect(runsOf({ id: 'h:1', summary: { workflowRuns: 'junk' } })).toEqual([]);
    expect(stepOf({ id: 'h:2', summary: { workflowStep: { run: 'r1', node: 'fix[2]' } } })).toEqual({ run: 'r1', node: 'fix[2]' });
  });

  it('copies the run as a formula to keep', () => {
    expect(JSON.parse(asFormula(run()))).toEqual({ wisp: 1, workflows: [{ name: 'audit', tasks: { plan: { prompt: 'List the routes' } } }] });
  });

  it('formats times and money', () => {
    expect([duration(9_400), duration(184_000), duration(3_720_000)]).toEqual(['9s', '3m 04s', '1h 02m']);
    expect([money(0), money(0.5), money(12.34)]).toEqual(['', '$0.50', '$12.3']);
  });
});

describe('/workflow in a prompt box', () => {
  const saved = [
    { name: 'release-check', description: 'Tests, audit, release note' },
    { name: 'audit', description: 'Look for auth holes' },
  ];
  it('offers the command while the word could still be it', () => {
    expect(suggestions('', []).map((i) => i.insert)).toEqual(['/workflow ']);
    expect(suggestions('wor', []).map((i) => i.insert)).toEqual(['/workflow ']);
    expect(suggestions('goal', [])).toEqual([]);
  });
  it('offers the saved workflows after it, by name first', () => {
    expect(suggestions('workflow ', saved).map((i) => i.label)).toEqual(['audit', 'release-check']);
    expect(suggestions('workflow rel', saved).map((i) => i.insert)).toEqual(['/workflow release-check ']);
    expect(suggestions('workflow note', saved).map((i) => i.label)).toEqual(['release-check']);
    expect(suggestions('workflow audit', saved), 'typed in full: Enter sends it').toEqual([]);
  });
});
