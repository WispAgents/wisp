// Every shape this UI reads or sends, in one file, until wisp-ship-api derives them with ts-rs
// (packages/protocol/src/generated): switching is one edit here. The shapes are v1's:
// server/git.js prSummary / prDetailOf, server/services/ship.js, server/routes/ship.js, and the
// session summary (server/runner.js summarize) for the fields Ship it reads.

export type PrState = 'OPEN' | 'CLOSED' | 'MERGED';
export type ReviewDecision = 'APPROVED' | 'CHANGES_REQUESTED' | 'REVIEW_REQUIRED' | '';

/** A PR as a task row keeps it (git.js prSummary). */
export interface PrSummary {
  readonly number: number;
  readonly url: string;
  readonly title?: string;
  readonly state: PrState;
  readonly draft?: boolean;
  readonly review?: ReviewDecision;
  readonly mergeState?: string;
  /** GitHub can't merge it into its base */
  readonly conflict?: boolean;
  readonly head?: string;
  readonly branch?: string;
  readonly base?: string;
  readonly mergedAt?: string | null;
  readonly checks?: { readonly pass: number; readonly fail: number; readonly pending: number };
  /** just pushed and GitHub lists no checks yet: they're coming, not passed (ship-api `waitingChecks`) */
  readonly waitingChecks?: boolean;
  readonly checkedAt?: number;
}

/** wisp fixing the PR's merge conflicts itself (ship.js fixConflicts). */
export interface ConflictFix {
  readonly step: 'fixing' | 'pushing' | 'done' | 'failed';
  readonly why?: string;
  readonly head?: string;
}

/** The Steward of a task's PR (ADR-0132; ship-api `Steward`): it sends the task failing checks and new review threads
 *  (and with `review`, another vendor's high findings), pushes the fix, stops green or with why. Never merges. */
export interface Steward {
  readonly mode: 'fix' | 'review' | (string & {});
  readonly step: 'watching' | 'fixing' | 'pushing' | 'green' | 'stopped' | (string & {});
  readonly head?: string;
  readonly passes?: { readonly checks?: number; readonly threads?: number; readonly review?: number };
  readonly causes?: readonly string[];
  readonly reviewer?: string;
  readonly why?: string;
}

export type ShipStep = 'start' | 'commit' | 'push' | 'pr';

/** The live stage of a ship in flight, sent with each summary (ship.js shipAll). */
export interface Shipping {
  readonly step: ShipStep;
  readonly update?: boolean;
  readonly repo?: string;
  readonly at?: number;
  readonly of?: number;
  readonly startedAt?: number;
}

/** What Ship it would take (ship.js check): `repos` counts repos with something new (several repos). */
export interface ToShip {
  readonly dirty: number;
  readonly ahead: number;
  readonly unpushed: number;
  readonly repos?: number;
}

export interface Finding {
  readonly severity: 'high' | 'medium' | 'low';
  readonly text: string;
  readonly file?: string;
  readonly line?: number;
  /** what the user said that it breaks */
  readonly rule?: string;
  /** the 1-based constraint it breaks */
  readonly constraint?: number;
}

/** How much of the diff a review had (review.js coverage). */
export interface Coverage {
  readonly files: number;
  readonly seen: number;
  readonly cut: readonly string[];
  readonly missing: readonly string[];
}

/** Review by another harness (server/services/review.js), on the task's own PR. */
export interface Review {
  readonly status: 'running' | 'done' | 'failed' | 'held';
  /** false once a push landed after it */
  readonly current?: boolean;
  readonly head?: string;
  readonly findings: readonly Finding[];
  readonly by?: { readonly agent?: string };
  readonly why?: string;
  readonly summary?: string;
  readonly tests?: { readonly verdict: 'pass' | 'fail' | 'unclear'; readonly command?: string; readonly passed?: number; readonly failed?: number };
  readonly coverage?: Coverage;
  readonly constraints?: readonly string[];
}

/** A task's other folder (one task across repos). */
export interface AlsoRepo {
  readonly project: string;
  readonly branch?: string;
  readonly pr?: PrSummary;
}

/** The fields of a task row (entity `session`, topic `list`) that Ship it reads. */
export interface ShipSession {
  readonly id: string;
  readonly agent: string;
  readonly state?: 'running' | 'waiting' | 'done' | 'failed' | 'idle' | (string & {});
  readonly title?: string;
  readonly project?: string;
  readonly branch?: string;
  readonly pr?: PrSummary | null;
  readonly also?: readonly AlsoRepo[];
  /** PRs its agent attached (wisp_pr) */
  readonly prs?: readonly { readonly repo: string; readonly pr: PrSummary }[];
  readonly toShip?: ToShip;
  readonly shipping?: Shipping | null;
  readonly conflictFix?: ConflictFix;
  readonly steward?: Steward;
  readonly review?: Review;
  /** a worktree another tool left (Bring in): merge never removes it */
  readonly foreign?: boolean;
  readonly archived?: boolean;
  readonly pending?: boolean;
  readonly delegated?: boolean;
  /** review comments written in Changes and not sent yet */
  readonly unsentComments?: number;
  /** Settings → Review by another harness is on (the host sends it with the row) */
  readonly reviewOn?: boolean;
  /** Settings → Merge risk is on (wisp.risk sends it with the row) */
  readonly riskOn?: boolean;
  /** merge risk, one verdict per PR it judged (wisp.risk's RiskView), the task's own first */
  readonly risk?: readonly RiskVerdict[];
  /** other open tasks changing the same files (overlaps.js), in words */
  readonly overlap?: string;
}

export type RiskLevel = 'low' | 'medium' | 'high';

/** One PR's merge risk (wisp-risk-api RiskView): of `head`, `current` while that is the PR's head. */
export interface RiskVerdict {
  readonly url: string;
  readonly repo: string;
  readonly number: number;
  readonly head: string;
  readonly status: 'running' | 'done' | 'failed';
  readonly level?: RiskLevel;
  /** 0–100 */
  readonly score?: number;
  readonly summary?: string;
  readonly reasons?: readonly string[];
  readonly by?: { readonly agent: string; readonly model: string };
  readonly why?: string;
  /** only the start of a long diff was judged */
  readonly partial?: boolean;
  readonly at: number;
  readonly current: boolean;
}

/** One PR and its repo, as the row lists them. */
export interface RepoPr {
  readonly repo: string | undefined;
  readonly pr: PrSummary;
}

export type CheckState = 'pass' | 'fail' | 'pending' | 'skip';

export interface Check {
  readonly name: string;
  readonly workflow?: string;
  readonly state: CheckState;
  readonly url?: string;
  readonly ms?: number | null;
  readonly startedAt?: number | null;
  /** branch protection needs it */
  readonly required?: boolean;
}

export interface Note {
  readonly kind?: 'comment' | 'review';
  readonly state?: string;
  readonly author: string;
  readonly bot?: boolean;
  readonly body: string;
  readonly at?: number | null;
  readonly url?: string;
}

export interface Thread {
  readonly path: string;
  readonly line?: number | null;
  readonly outdated?: boolean;
  readonly notes: readonly Note[];
}

/** One PR in full for the pane (ship.js details → git.js prDetailOf). */
export interface PrDetail extends PrSummary {
  readonly repo?: string;
  readonly author?: string;
  readonly body?: string;
  readonly additions?: number;
  readonly deletions?: number;
  readonly changedFiles?: number;
  readonly files?: readonly { readonly path: string; readonly additions: number; readonly deletions: number }[];
  readonly checkList?: readonly Check[];
  readonly reviewers?: readonly { readonly author: string; readonly state: string }[];
  readonly notes?: readonly Note[];
  readonly threads?: readonly Thread[];
  readonly updatedAt?: number | null;
  /** gh couldn't read this one: the rest still shows */
  readonly error?: string;
}

/** Entity `pr`, id = the session's id, on topic `plugin:wisp.ship:<session id>`. The host keeps
 * it fresh while a client watches (20 s while a check runs, else 60 s): the pane never polls. */
export interface PrDetails {
  readonly prs: readonly PrDetail[];
  readonly at: number;
  /** the last look failed: what it last saw stays */
  readonly error?: string;
}

/** One repo in the Ship it dialog (GET /sessions/:id/ship). */
export interface ShipRepo {
  readonly repo?: string;
  readonly branch: string;
  readonly base: string;
  readonly remote?: string;
  readonly dirty: number;
  readonly ahead: number;
  readonly pr?: PrSummary | null;
}

export interface ShipInfo extends ShipRepo {
  /** several repos: each one; the top level sums dirty and ahead */
  readonly repos?: readonly ShipRepo[];
  readonly title: string;
  readonly body: string;
  /** at least one repo has a `.wisp/` folder: offer to save the task into `.wisp/` with the commit */
  readonly hasWisp?: boolean;
}

export interface ShipValues {
  readonly title: string;
  readonly body: string;
  readonly draft: boolean;
  readonly saveTask?: boolean;
  /** Keep it green (the Steward): `fix`, `review`, or `off` to switch off one that's on */
  readonly steward?: 'fix' | 'review' | 'off';
}

export interface ShipResult extends Partial<PrSummary> {
  readonly prs?: readonly { readonly repo: string; readonly number: number; readonly url: string; readonly state: PrState }[];
}

export interface MergeResult {
  readonly pr: PrSummary;
  readonly sync: { readonly ok: boolean; readonly text: string };
  readonly prs?: readonly { readonly repo: string; readonly number: number; readonly url: string; readonly state: PrState }[];
}

/** An agent's name, for "Review by Codex" (the agents list, entity `agent`). */
export interface AgentRow {
  readonly id: string;
  readonly name: string;
}

/** Entity kinds and topics this UI reads. */
export const SESSION = 'session';
export const DETAIL = 'pr';
export const AGENT = 'agent';
export const detailTopic = (sessionId: string): `plugin:wisp.ship:${string}` => `plugin:wisp.ship:${sessionId}`;
/** The task's reply box: its draft (drafts store) and the shell's focus target. */
export const REPLY_FIELD = 'reply';
