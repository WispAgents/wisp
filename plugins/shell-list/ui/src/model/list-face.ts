// What the list shows: its rows, the shape of rows while its topic is still coming, or the empty state once the peer
// has said there's nothing (QA round 2 #12: "No tasks yet" flashed before the rows). Pure.

export type ListFace = 'rows' | 'loading' | 'empty';

export const listFace = (items: number, loaded: boolean): ListFace => (items > 0 ? 'rows' : loaded ? 'empty' : 'loading');
