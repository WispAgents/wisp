// The phone apps' New task bar (QA r2 #2; ADR-0012: a bar collapses into More, it never runs off its edge): Goal,
// Worktree and Start later sat past a 390 px screen's edge or under Send, and Recipes were gone with their empty row.
// On a phone the tail folds into one More tool whose menu holds them all, then Start from an issue and Recipes. The
// fold itself is the kit's (a narrow desktop bar folds its tail the same way, QA desktop r2 #4).
export { foldTail, type FoldExtras } from '@wisp/ui/pure';
