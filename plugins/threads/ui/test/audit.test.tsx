// The compaction audit as the Thread tab and the turn show it (ADR-0096): "Kept 6 of 8" and what was lost, each item
// under its section; still asking; skipped and why; the Thread lists them newest first, the newest with its losses.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import type { JSX } from 'solid-js';
import { AuditLine, Compactions } from '../src/Audit.tsx';
import { t } from '../src/i18n.ts';
import '../src/i18n/lazy.ts';
import type { Audit } from '../src/types.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});
const NOW = 5_000_000;
const audit = (more: Partial<Audit> = {}): Audit => ({ task: 's1', turn: 't4', status: 'done', kept: 6, total: 8, lost: ['Constraints: No Redis', 'Tried and failed: express-rate-limit'], told: false, at: NOW - 180_000, ...more });
function mount(view: () => JSX.Element) {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(view, host);
  return (part: string): HTMLElement[] => [...host.querySelectorAll<HTMLElement>(`[data-part="${part}"]`)];
}

test('done: Kept 6 of 8 after the compaction, when, and each lost item under its section', () => {
  const q = mount(() => <AuditLine audit={audit()} now={NOW} t={t} />);
  expect(q('threads.audit.kept')[0]?.textContent).toBe('Kept 6 of 8');
  expect(q('threads.audit.what')[0]?.textContent).toBe('after the compaction');
  expect(q('threads.audit.time')[0]?.textContent).toBe('3m');
  expect(q('threads.audit.lostHead')[0]?.textContent).toBe('Lost (the next follow-up reminds the agent):');
  expect(q('threads.audit.item').map((n) => [n.querySelector('[data-part="threads.audit.section"]')?.textContent, n.querySelector('[data-part="threads.audit.text"]')?.textContent])).toEqual([
    ['Constraints', 'No Redis'],
    ['Tried and failed', 'express-rate-limit'],
  ]);
});

test('nothing lost reads so; once told, the losses say the agent was reminded', () => {
  const q = mount(() => <AuditLine audit={audit({ kept: 8, lost: [] })} now={NOW} t={t} />);
  expect(q('threads.audit.what')[0]?.textContent).toBe('after the compaction: nothing lost');
  expect(q('threads.audit.lost')).toEqual([]);
  dispose?.();
  const q2 = mount(() => <AuditLine audit={audit({ told: true })} now={NOW} t={t} />);
  expect(q2('threads.audit.lostHead')[0]?.textContent).toBe('Lost (the agent was reminded):');
});

test('still asking, and skipped with why', () => {
  const q = mount(() => (
    <>
      <AuditLine audit={audit({ status: 'running', kept: 0, total: 0, lost: [] })} now={NOW} t={t} />
      <AuditLine audit={audit({ status: 'skipped', why: 'Its agent can’t answer on a copy of its conversation.', kept: 0, total: 0, lost: [] })} now={NOW} t={t} />
    </>
  ));
  expect(q('threads.audit.running')[0]?.textContent).toBe('Checking what the compaction kept…');
  expect(q('threads.audit.skipped')[0]?.textContent).toBe('Compaction check skipped: Its agent can’t answer on a copy of its conversation.');
  expect(q('threads.audit.kept')).toEqual([]);
});

test('the Thread’s Compactions list them newest first; only the newest lists its losses; none: no section', () => {
  const q = mount(() => <Compactions audits={[audit(), audit({ turn: 't2', kept: 7, at: NOW - 3_600_000 })]} now={NOW} t={t} />);
  expect(q('threads.compactions.label')[0]?.textContent).toBe('Compactions');
  expect(q('threads.audit.kept').map((n) => n.textContent)).toEqual(['Kept 6 of 8', 'Kept 7 of 8']);
  expect(q('threads.audit.lost').length).toBe(1);
  dispose?.();
  expect(mount(() => <Compactions audits={[]} now={NOW} t={t} />)('threads.compactions')).toEqual([]);
});
