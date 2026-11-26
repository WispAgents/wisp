// Share… (v1 crew-share.js shareMember), pure: what can go along (kit, notebook, delegates) for these members, the
// export's args for the options ticked, the notes worth showing, and the text a bundle is copied or saved as.

import type { Member } from './types.ts';

export interface ShareOpts {
  kit: boolean;
  notebook: boolean;
  team: boolean;
}

export interface ShareWhat {
  /** files across their kits */
  kits: number;
  /** someone has a notebook to share */
  noted: boolean;
  /** one member's crew delegates (other members), who stay here unless ticked */
  mates: string[];
  /** the options as they start: the kit when there is one, never the notebook or delegates */
  start: ShareOpts;
}

export function shareWhat(members: readonly Pick<Member, 'id' | 'kit' | 'notebook' | 'team'>[], all: readonly Pick<Member, 'id' | 'name'>[]): ShareWhat {
  const kits = members.reduce((n, m) => n + (m.kit?.length ?? 0), 0);
  const noted = members.some((m) => !!m.notebook?.trim());
  const one = members.length === 1 ? members[0] : undefined;
  const mates = one ? (one.team ?? []).flatMap((x) => (x.crew ? (all.find((m) => m.id === x.crew)?.name ?? []) : [])) : [];
  return { kits, noted, mates, start: { kit: kits > 0, notebook: false, team: false } };
}

/** The peer's notes, without the one the delegates option already says (who stays here). */
export const shareNotes = (notes: readonly string[], mates: readonly string[]): string[] => notes.filter((n) => !(mates.length && / delegates to .*, who stays here/.test(n)));

/** The file as it is saved or copied (v1: pretty, with a newline at the end). */
export const bundleText = (bundle: unknown): string => `${JSON.stringify(bundle, null, 2)}\n`;

/** Who opens a crew link, from crew.share's answer (the grid's view of it, `link.access.kind`). */
export type LinkAccess = 'anyone' | 'people' | 'teams' | 'password';

export function linkAccess(link: unknown): LinkAccess {
  const k = (link as { access?: { kind?: unknown } } | null | undefined)?.access?.kind;
  return k === 'people' || k === 'teams' || k === 'password' ? k : 'anyone';
}

type HintKey = 'wisp.crew.share.linkHint' | 'wisp.crew.share.linkHintPeople' | 'wisp.crew.share.linkHintTeams' | 'wisp.crew.share.linkHintPassword';

/** The hint under the link: who opens it. Sharing again keeps who that is (set with `wisp cloud access`), so a link
 *  that isn't anyone's says it was kept, and the dialog offers to open it to anyone. */
export function linkHintKey(a: LinkAccess): HintKey {
  if (a === 'people') return 'wisp.crew.share.linkHintPeople';
  if (a === 'teams') return 'wisp.crew.share.linkHintTeams';
  if (a === 'password') return 'wisp.crew.share.linkHintPassword';
  return 'wisp.crew.share.linkHint';
}
