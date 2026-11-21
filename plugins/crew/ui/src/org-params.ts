// The Org view's params (views.ts VIEW.org). In the phone apps the Crew tab opens it bare (the members list, the
// platform's screen) and its root bar's Org with `{ chart: true }` (the chart, pushed with Back); the browser draws
// the chart either way. Read by OrgPage's chunk (the view passes its params through).

export interface OrgParams {
  readonly chart: true;
}

export const orgParams = (p: unknown): OrgParams | null => (p && typeof p === 'object' && (p as { chart?: unknown }).chart === true ? { chart: true } : null);

/** Opening the Org chart (the stories' Org bubble, the Crew tab's Org): the phone apps' chart, the browser's page. */
export const orgOpen = (): { t: 'openView'; id: 'crew.org'; params: OrgParams } => ({ t: 'openView', id: 'crew.org', params: { chart: true } });
