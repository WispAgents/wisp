// An installed app left open (or resumed) never learned of a new build: the worker looked only on a navigation. The
// tab now asks it (`{t:'check'}`) when it comes back into view, when the app resumes and when its stream comes back
// live; each listener goes once the scope that holds it ends.
import { expect, test } from 'vitest';
import { signal } from '@wisp/client';
import type { Platform } from '@wisp/shell';
import { nullPlatform } from '@wisp/shell/platform';
import type { Toaster } from '@wisp/ui';
import { registerWorker } from '../src/worker.ts';

function fakes() {
  const posted: unknown[] = [];
  const docOn = new Map<string, EventListener>();
  const resumes = new Set<() => void>();
  const doc = {
    visibilityState: 'hidden' as DocumentVisibilityState,
    addEventListener: (t: string, fn: EventListener) => void docOn.set(t, fn),
    removeEventListener: (t: string) => void docOn.delete(t),
  };
  const container = {
    register: async () => ({}),
    controller: { postMessage: (m: unknown) => void posted.push(m) },
    addEventListener: () => {},
    removeEventListener: () => {},
  } as unknown as ServiceWorkerContainer;
  const platform: Platform = {
    ...nullPlatform(),
    kind: 'browser',
    mobile: true,
    onResume: (fn) => {
      resumes.add(fn);
      return () => resumes.delete(fn);
    },
  };
  const toaster = { show: () => {} } as unknown as Toaster;
  return { posted, docOn, resumes, doc, container, platform, toaster };
}

test('back_in_view_resumed_or_live_again_the_tab_asks_the_worker_to_check', () => {
  const f = fakes();
  const live = signal(false);
  const off = registerWorker(f.platform, f.toaster, (k) => k, { container: f.container, doc: f.doc as unknown as Document, live });
  const visibility = (s: DocumentVisibilityState): void => {
    f.doc.visibilityState = s;
    f.docOn.get('visibilitychange')?.(new Event('visibilitychange'));
  };
  visibility('hidden');
  expect(f.posted).toEqual([]);
  visibility('visible');
  expect(f.posted).toEqual([{ t: 'check' }]);
  for (const fn of f.resumes) fn();
  expect(f.posted).toHaveLength(2);
  live(true);
  expect(f.posted).toHaveLength(3);
  live(false);
  expect(f.posted, 'going down asks nothing').toHaveLength(3);
  off();
  visibility('visible');
  for (const fn of f.resumes) fn();
  live(true);
  expect(f.posted, 'taken off with the scope').toHaveLength(3);
  expect(f.resumes.size).toBe(0);
});
