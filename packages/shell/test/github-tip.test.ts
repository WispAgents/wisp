// QA r1 #23 / lead: Home's "Sign in to GitHub" tip showed while gh was signed in (Settings → Agents' GitHub row said
// Signed in). It reads the same answer (`git.tools`' gh), and until that answer is known, or when gh can't tell, it
// stays hidden: no tip flicker, and no toast when a peer refuses the read.
import { expect, test } from 'vitest';
import { githubTip, tools } from '../src/frame/github-tip.ts';

test('the_github_tip_shows_only_when_gh_is_known_to_be_signed_out_or_missing', () => {
  expect(githubTip(undefined), 'not answered yet').toBe(false);
  expect(githubTip(null), 'refused or offline').toBe(false);
  expect(githubTip({ gh: { installed: true, signedIn: true } })).toBe(false);
  expect(githubTip({ gh: { installed: true } }), "gh couldn't tell").toBe(false);
  expect(githubTip({ gh: { installed: true, signedIn: false } })).toBe(true);
  expect(githubTip({ gh: { installed: false } })).toBe(true);
  expect(tools.quiet, 'a refusal is no tip, never a toast').toBe(true);
});
