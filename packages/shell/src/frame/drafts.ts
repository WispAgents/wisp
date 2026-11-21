// Where the prompt boxes keep what's being written (kind 5, the client's drafts store: across reloads and updates,
// v1 F-task-32). New task's is `<home>:launcher` → `text`, the key wisp.stuff's share-in writes to as well; a reply's
// is its task's key → `reply`.

/** New task's draft: the entity and field */
export const launcherDraft = (home: string): readonly [string, string] => [`${home}:launcher`, 'text'];

/** A task's reply draft */
export const replyDraft = (task: string): readonly [string, string] => [task, 'reply'];

/** An empty box holds no draft (the store drops the key) */
export const draftValue = (v: string): string | undefined => (v === '' ? undefined : v);

/** New task's text with a share link carried from the Grid (docs/ux/2026-11-18-grid.md #8): on its own line after what's
 *  there, once. It's text in the box, never sent: the person presses Send (wisp.stuff's share card imports it). */
export const withShareLink = (draft: string, link: string): string =>
  draft.includes(link) ? draft : draft.trim() === '' ? link : `${draft.trimEnd()}\n${link}`;
