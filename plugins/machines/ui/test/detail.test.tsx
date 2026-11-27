// Machines' detail (owner's pick B, 2026-11-27). The old row put the version, Update and the agents in one squeezed line
// beside the name: at 390 px it broke one letter a line (QA screenshot). Now an online machine's version and agents are
// lines of their own in Facts, an update is a note with its button, and the list beside says how many tasks run there.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { englishT, KitProvider } from '@wisp/ui';
import '../src/i18n/lazy.ts';
import { listLine, MachineDetail } from '../src/panes/Card.tsx';
import type { PeerBody, PeerView, StatusBody } from '../src/commands.ts';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
  document.body.innerHTML = '';
});

const cloud = { id: 'c1', name: 'Cloud', transport: 'grid', address: 'me.grid.trywisp.dev', port: 443, data: { grid: 'https://grid.trywisp.dev' } } as unknown as PeerBody;
const online = {
  id: 'c1',
  state: 'online',
  error: '',
  remote: { version: '1.0.0-alpha.15', agents: [{ id: 'claude', name: 'Claude Code', available: true }, { id: 'codex', name: 'Codex', available: true }, { id: 'grok', name: 'Grok', available: false }] },
} as unknown as StatusBody;
const view = { id: 'c1', working: 2, outdated: '1.0.0-alpha.17' } as unknown as PeerView;

test('an_online_machines_version_and_agents_are_lines_of_their_own_and_its_update_a_note', () => {
  const host = document.createElement('div');
  document.body.append(host);
  stop = render(
    () => (
      <KitProvider t={englishT}>
        <MachineDetail m={cloud} s={online} view={view} onAct={() => undefined} onLetIn={() => Promise.resolve()} onCopy={() => undefined} editing={false} form={() => null} />
      </KitProvider>
    ),
    host,
  );
  const facts = [...host.querySelectorAll('[data-kit="fact"]')].map((f) => [f.querySelector('dt')?.textContent, f.querySelector('dd')?.textContent]);
  expect(facts.map((f) => f[0])).toEqual(['Working', 'Where', 'Version', 'Agents']);
  expect(facts[0]?.[1]).toBe('2 tasks');
  expect(facts[2]?.[1]).toBe('wisp 1.0.0-alpha.15');
  expect(host.querySelectorAll('[data-part="machines.agents"] [data-kit="pill"]').length, 'the ready ones, a chip each').toBe(2);
  expect(host.querySelector('[data-part="machines.update"]')?.textContent).toContain('1.0.0-alpha.17');
  expect(host.querySelector('[data-part="machines.meta"]'), 'no squeezed meta line').toBeNull();
});

test('the_list_beside_says_how_many_run_there_or_its_state', () => {
  expect(listLine(cloud, online, view)).toEqual({ sub: '2 working', dot: 'good' });
  expect(listLine(cloud, { ...online, state: 'off' } as StatusBody, undefined)).toEqual({ sub: 'Off', dot: 'muted' });
  expect(listLine(cloud, { ...online, state: 'error' } as StatusBody, undefined).dot).toBe('bad');
});
