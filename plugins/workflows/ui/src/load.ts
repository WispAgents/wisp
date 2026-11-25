// The row and the board, one import away from the index (ADR-0105): the index's own import names only this module, so
// the first page carries no list of their chunks to preload; theirs come when a task page shows them.
export const card = (): Promise<typeof import('./Card.tsx')> => import('./Card.tsx');
export const board = (): Promise<typeof import('./Board.tsx')> => import('./Board.tsx');
