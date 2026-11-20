// ADR-0114's band: the_band_says_each_station_the_round_and_what_waits_on_you. A factory run's row, worked out as the band
// shows it: the header's words and tone, each station as a step, the tasks to open, Stop only while it goes.

import { describe, expect, it } from 'vitest';
import ui from '../src/index.ts';
import { t } from '../src/i18n.ts';
import '../src/i18n/lazy.ts';
import { endWhy, live, openable, pill, runOf, step, where } from '../src/model.ts';
import type { RunLine } from '../src/types.ts';

const run = (over: Partial<RunLine> = {}): RunLine => ({
  name: 'default',
  state: 'running',
  station: 'review',
  round: 2,
  max: 3,
  why: '',
  stations: [
    { id: 'spec', state: 'done', round: 1, tasks: ['s1'], output: 'SPEC: add adds', why: '' },
    { id: 'build', state: 'done', round: 2, tasks: ['b1', 'b2'], output: 'built', why: '' },
    { id: 'checks', state: 'done', round: 2, tasks: [], output: '1 of 1 passed', why: '' },
    { id: 'review', state: 'running', round: 2, tasks: ['r1'], output: '', why: '' },
    { id: 'ship', state: 'waiting', round: 0, tasks: [], output: '', why: '' },
  ],
  ...over,
});

const words = (s: { key: string; vars?: Record<string, string | number> }): string => {
  const vars: Record<string, string | number> = {};
  for (const [k, v] of Object.entries(s.vars ?? {})) vars[k] = typeof v === 'string' && v.startsWith('wisp.factory.') ? t(v as never) : v;
  return t(s.key as never, vars);
};

describe('wisp.factory ui', () => {
  it('declares its plugin id, one lazy band and a first-screen catalogue of one string', () => {
    expect(ui.id).toBe('wisp.factory');
    expect(ui.extensions?.map((e) => e.attachTo.id)).toEqual(['shell.task:band']);
    expect(Object.keys(ui.catalogue ?? {}).every((k) => k.startsWith('wisp.factory.'))).toBe(true);
  });

  it('the_band_says_each_station_the_round_and_what_waits_on_you', () => {
    const r = run();
    expect(words(where(r).said)).toBe('Review · round 2 of 3');
    expect(where(r).tone).toBe('accent');
    const steps = r.stations.map((s) => step(r, s));
    expect(steps.map((s) => s.status)).toEqual(['done', 'done', 'done', 'current', 'pending']);
    expect(words(steps[1]!.title)).toBe('Build · round 2');
    expect(steps[0]!.detail).toBe('SPEC: add adds');
    expect(words(steps[3]!.detail as never)).toBe('Working…');
    expect(openable(r).map((o) => words(o.said))).toEqual(['Open Spec', 'Open Build 1', 'Open Build 2', 'Open Review']);
    expect(live(r)).toBe(true);
    // waiting on you: the question on its station
    const w = run({ state: 'waiting', station: 'ship', stations: r.stations.map((s) => (s.id === 'ship' ? { ...s, state: 'asking', round: 2 } : s)), waiting: { station: 'ship', question: 'Merge it?', options: ['Merge', 'Leave it open'], askId: 'a1' } });
    expect(words(where(w).said)).toBe('Waiting on you');
    expect(where(w).tone).toBe('warn');
    expect(words(step(w, w.stations[4]!).detail as never)).toBe('Waiting on you: Merge it?');
    // audit G5: "Approve the spec?" shows the spec it asks about, not only the question
    const sp = run({ state: 'waiting', station: 'spec', round: 1, stations: r.stations.map((s) => (s.id === 'spec' ? { ...s, state: 'asking' } : s)), waiting: { station: 'spec', question: 'Approve the spec?', options: ['Approve', 'Stop'], askId: 'a2', context: 'SPEC: add adds' } });
    expect(words(step(sp, sp.stations[0]!).detail as never)).toBe('SPEC: add adds · waiting on you');
    // ended: how, why, and no Stop
    const f = run({ state: 'failed', station: '', why: 'Checks still fail after 3 round(s)', stations: r.stations.map((s) => (s.id === 'checks' ? { ...s, state: 'failed', why: '`npm test` exited 1' } : s)) });
    expect(words(where(f).said)).toBe('Failed');
    expect(step(f, f.stations[2]!).detail).toBe('`npm test` exited 1');
    expect(live(f)).toBe(false);
    expect(endWhy(f), 'no station says it: under the band').toBe('Checks still fail after 3 round(s)');
    // audit G15: said once, on its station, not again under the band
    const sh = run({ state: 'failed', station: '', why: 'ship failed: Nothing to ship', stations: r.stations.map((s) => (s.id === 'ship' ? { ...s, state: 'failed', why: 'ship failed: Nothing to ship' } : s)) });
    expect(endWhy(sh)).toBeUndefined();
    // audit G15: an ended run's state is the task's own status (Finished, Failed), not a second pill in the band
    expect(pill(r)).toEqual(where(r));
    expect(pill(run({ state: 'waiting' }))).toEqual(where(run({ state: 'waiting' })));
    for (const state of ['done', 'failed', 'stopped'] as const) expect(pill(run({ state, station: '' })), state).toBeUndefined();
    expect(words(where(run({ state: 'running', station: '', round: 1 })).said)).toBe('Starting');
    expect(words(where(run({ station: 'build', round: 1 })).said)).toBe('Build');
  });

  it('reads the run from a synced row, flattened from summary', () => {
    expect(runOf({ summary: { factory: run() } })?.name).toBe('default');
    expect(runOf({ factory: run() })?.station).toBe('review');
    expect(runOf({ summary: {} })).toBeUndefined();
    expect(runOf(undefined)).toBeUndefined();
    // a row from an older server: stations with only id and state still draw
    const old = { ...run(), stations: [{ id: 'build', state: 'done' }] } as unknown as RunLine;
    expect(openable(old)).toEqual([]);
    expect(step(old, old.stations[0]!).status).toBe('done');
  });
});
