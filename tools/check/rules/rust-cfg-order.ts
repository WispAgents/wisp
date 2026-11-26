// rust-cfg-order: in a test file, a crate-level `#![allow(…)]` (or `#![expect(…)]`) comes before its `#![cfg(…)]`.
// rustc applies `#![cfg(unix)]` first: on Windows it strips the crate, and the inner attributes after it with it,
// but the `//!` doc lines above are still linted. So `clippy -D warnings` on Windows failed on a doc_markdown the
// `#![allow]` was there to quiet (crates/wisp/tests/serve_install.rs, CI on 3350069e). Allow first, then cfg.

import { finding } from '../core/report.ts';
import { codeLines, crates, isTestFile, rustFiles } from '../core/rust.ts';
import type { Finding, Rule } from '../core/types.ts';

const CFG = /^\s*#!\[cfg\(/;
const ALLOW = /^\s*#!\[(?:allow|expect)\(/;

/** Findings for one file's text: each crate-level allow/expect below a crate-level cfg (exported for the rule's test). */
export function cfgOrder(file: string, text: string): Finding[] {
  const code = codeLines(text);
  const cfg = code.findIndex((l) => CFG.test(l));
  if (cfg < 0) return [];
  const out: Finding[] = [];
  for (let i = cfg + 1; i < code.length; i++) {
    if (ALLOW.test(code[i] ?? '')) out.push(finding(file, i + 1, 'rust-cfg-order', `this #![allow] sits below #![cfg] (line ${cfg + 1}): where the cfg is false it is stripped with the crate, and the //! docs above are linted without it. Put the #![allow] first`));
  }
  return out;
}

export const rustCfgOrder: Rule = {
  id: 'rust-cfg-order',
  description: "a test file's crate-level #![allow] comes before its #![cfg], so the lints it quiets stay quiet where the cfg is false",
  run(ctx) {
    const out: Finding[] = [];
    for (const c of crates(ctx.root)) {
      for (const f of rustFiles(ctx.root, c.dir).filter(isTestFile)) out.push(...cfgOrder(f, ctx.read(f)));
    }
    return out;
  },
};
