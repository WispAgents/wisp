// The band, one import away from the index (ADR-0105): the index's own import names only this module, so the first
// page carries no list of the band's chunks to preload; the band's own come when a task page shows it.
export const band = (): Promise<typeof import('./Band.tsx')> => import('./Band.tsx');
