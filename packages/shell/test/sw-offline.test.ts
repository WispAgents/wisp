import { expect, test } from 'vitest';
import { offlineWaiting, waitingFor } from '../src/platform/sw-offline.ts';

const res = (status: number, headers: Record<string, string>) => ({ status, headers: new Headers(headers) });

// QA round 10 Grid #2: on a phone the worker answered an offline computer's tab from its cache (the stale app) or with
// the grid's JSON; the grid's offline answer names its waiting page, and only that page on this origin is taken
test('an_offline_computer_s_answer_sends_the_tab_to_its_waiting_page_and_nothing_else_does', () => {
  expect(offlineWaiting(res(503, { 'x-wisp-offline': '/_grid/offline?r=%2Ftasks' }))).toBe('/_grid/offline?r=%2Ftasks');
  expect(offlineWaiting(res(503, {}))).toBeNull();
  expect(offlineWaiting(res(200, { 'x-wisp-offline': '/_grid/offline?r=%2F' }))).toBeNull();
  // never another origin or another page
  expect(offlineWaiting(res(503, { 'x-wisp-offline': 'https://evil.example/_grid/offline' }))).toBeNull();
  expect(offlineWaiting(res(503, { 'x-wisp-offline': '//evil.example/_grid/offline' }))).toBeNull();
  expect(offlineWaiting(res(503, { 'x-wisp-offline': '/api/x' }))).toBeNull();
  expect(offlineWaiting(undefined)).toBeNull();
});

// the page the tab was on is what the waiting page opens again (not `/`: a task stays open)
test('the_waiting_page_is_for_the_page_the_tab_was_on', () => {
  expect(waitingFor('https://studio--ada.grid.example/t/01ab?tab=files')).toBe('/_grid/offline?r=%2Ft%2F01ab%3Ftab%3Dfiles');
  expect(waitingFor('https://studio--ada.grid.example/')).toBe('/_grid/offline?r=%2F');
});
