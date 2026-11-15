// The Org view's params (views.ts VIEW.org). In the phone apps the Crew tab opens it bare (the members list, the
// platform's screen) and its root bar's Org with `{ chart: true }` (the chart, pushed with Back); the browser draws
// the chart either way. Read by OrgPage's chunk (the view passes its params through).

export interface OrgParams {
  readonly chart: true;
}

export const orgParams = (p: unknown): OrgParams | null => (p && typeof p === 'object' && (p as { chart?: unknown }).chart === true ? { chart: true } : null);
