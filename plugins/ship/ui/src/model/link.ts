// A link in a task's text as one of its PRs (ADR-0145): `https://github.com/acme/api/pull/12/files#r9` is the
// task's PR #12 of acme/api when that PR is on the task (shipped, attached by wisp_pr, or pasted in a prompt).
// Pure and light: the `shell.task:link` extension's `if` reads it for every link the turns draw.
import type { RepoPr, ShipSession } from '../types.ts';
import { prsOf } from './prs.ts';

const PR_URL = /^https?:\/\/(?:www\.)?github\.com\/([\w.-]+)\/([\w.-]+)\/pull\/([1-9]\d{0,9})(?:[/?#].*)?$/i;

/** `owner/repo#n` of a GitHub PR link, lower-cased (GitHub's names are case-blind), or undefined. */
export function prKeyOf(href: string): string | undefined {
  const m = PR_URL.exec(href.trim());
  return m ? `${(m[1] ?? '').toLowerCase()}/${(m[2] ?? '').toLowerCase()}#${m[3] ?? ''}` : undefined;
}

/** The task's PR a link names, or undefined (not a PR link, or a PR the task doesn't have). */
export function prOfLink(s: ShipSession | undefined, href: string | undefined): RepoPr | undefined {
  const k = href ? prKeyOf(href) : undefined;
  if (!k) return undefined;
  return prsOf(s).find((x) => prKeyOf(x.pr.url) === k);
}
