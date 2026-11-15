// The phone apps' destinations (ADR-0065 step 8, design/mobile.md §A.1): Tasks · Crew · Stuff · Computers in the
// navigation bar (Android) / tab bar (iOS). Each is a place navigation already has: Tasks is the list (Home), Crew
// and Stuff are those plugins' pages, Computers is the phone app's own view (registered by @wisp/mobile). A
// destination whose view isn't registered isn't offered. Which one is selected follows from what's open, never
// from a press, so Back, a notification or a link keep it right. Pure.

import type { ExtensionId, Intent, Json, Main } from '@wisp/client';
// its labels are lazy strings: they load with this module (PhoneNav's chunk), so the bar's first frame has them
import '../i18n/lazy.ts';

export type DestinationId = 'tasks' | 'crew' | 'stuff' | 'computers';

/** The phone app's Computers screen: its saved computers, launching one, adding one. */
export const COMPUTERS_VIEW = 'phone.computers';

export interface Destination {
  readonly id: DestinationId;
  /** the main view it opens; none for Tasks (the list) */
  readonly view?: string;
  /** i18n key */
  readonly label: string;
  readonly icon: string;
  readonly iconSelected?: string;
}

export const DESTINATIONS: readonly Destination[] = [
  { id: 'tasks', label: 'shell.dest.tasks', icon: 'feed', iconSelected: 'feed-fill' },
  { id: 'crew', view: 'crew.org', label: 'shell.dest.crew', icon: 'crew', iconSelected: 'crew-fill' },
  { id: 'stuff', view: 'stuff.page', label: 'shell.dest.stuff', icon: 'stuff' },
  { id: 'computers', view: COMPUTERS_VIEW, label: 'shell.dest.computers', icon: 'laptop' },
];

/** The destinations offered: Tasks always, the others while their view is registered. */
export function destinationsOf(registered: (view: string) => boolean): readonly Destination[] {
  return DESTINATIONS.filter((d) => d.view === undefined || registered(d.view));
}

/** The destination what's open belongs to: a crew page (Org, a member's thread) is Crew's, a Stuff page Stuff's, the
 *  Computers view Computers'; the list, a task and any other view are under Tasks. */
export function destinationOf(main: Main): DestinationId {
  if (main.kind === 'crew') return 'crew';
  if (main.kind !== 'view') return 'tasks';
  const id = String(main.id);
  if (id === COMPUTERS_VIEW) return 'computers';
  if (id.startsWith('crew.')) return 'crew';
  if (id.startsWith('stuff.')) return 'stuff';
  return 'tasks';
}

/** What selecting a destination does: Tasks goes to the list (closing a task), the others open their view. */
export function intentOf(d: Pick<Destination, 'view'>): Intent {
  return d.view === undefined ? { t: 'home' } : { t: 'openView', id: d.view as ExtensionId, params: null as Json };
}
