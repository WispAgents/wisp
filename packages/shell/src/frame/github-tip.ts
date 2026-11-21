// Home's "Sign in to GitHub" setup tip: shown only when this computer's gh is known to be signed out (or missing), from
// the same answer Settings → Agents' GitHub row reads (`git.tools`). Until it's known, or when gh can't tell, no tip:
// it never shows and then goes (QA r1 #23: it showed while gh was signed in).
import { defineCommand } from '@wisp/client';

export interface ToolsAnswer {
  readonly gh: { readonly installed: boolean; readonly signedIn?: boolean | null };
}

/** `git.tools`, quiet: read on every Home load; a peer that refuses it shows no tip, never a toast. */
export const tools = defineCommand<{ peer: string }, ToolsAnswer>({ id: 'git.tools', owner: (a) => a.peer, wire: () => null, optimistic: 'none', pending: 'the GitHub tip', quiet: true });

/** `undefined`: not answered yet; `null`: refused or offline. */
export function githubTip(t: ToolsAnswer | null | undefined): boolean {
  if (!t) return false;
  return !t.gh.installed || t.gh.signedIn === false;
}
