// The phone app keeps every computer's `setting` rows in one store (`<peer>:<key>`): New task read its kept harness
// with `list('setting').find(key)`, so another computer's row came first and the pick was back on each open. New task
// and the project order read their settings under `home` by key.
import { expect, test } from 'vitest';
import home from '../src/frame/HomeView.tsx?raw';
import projects from '../src/frame/projects.ts?raw';

test('new_task_reads_this_computers_settings_only', () => {
  for (const src of [home, projects]) {
    expect(src).not.toMatch(/entities\.list<[^>]*>\('setting'\)/);
    expect(src).toMatch(/entities\.row<[^>]*>\('setting', entityKey\(home as PeerId, /);
  }
});
