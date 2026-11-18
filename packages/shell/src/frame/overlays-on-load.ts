// What a page load brings back of what was open over the place (QA round 4 #2, round 3 #9). v1 kept only the open task
// in its URL (#/s/<id>): no dialog, sheet or palette ever came back. So one rule for every overlay, not a list: a
// reload, or the app reopening its last place (`/`: the saved place), brings back the place only. A link opened on
// purpose that names an overlay (`?o=`: a shared link, the QA gates' deep links) and Back or Forward into the app keep
// it. Pure: the caller passes the navigation's type (PerformanceNavigationTiming.type) and the URL's search.

export interface LoadInfo {
  /** 'navigate' | 'reload' | 'back_forward' | 'prerender', or undefined where the browser doesn't say */
  readonly navigation: string | undefined;
  readonly search: string;
}

export function keepsOverlaysOnLoad(l: LoadInfo): boolean {
  if (l.navigation !== 'navigate' && l.navigation !== 'back_forward') return false;
  return new URLSearchParams(l.search).has('o');
}
