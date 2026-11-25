// An ADR's number is its name: code, tests, the CHANGELOG and other ADRs cite it as `ADR-00NN`. Two files
// with one number make every such citation ambiguous, and two branches that each take the next free number
// only collide in docs/adr/README.md at merge time (#915 and #917 both took 0119). So: one file per number,
// the file's heading names the number its filename has, and an index row shows the number of the file it
// links. A row whose file is gone is a warning (two such rows predate this rule).

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { finding } from '../core/report.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';

const ID = 'adr-numbers';
const DIR = 'docs/adr';
const FILE = /^(\d{4})-.+\.md$/;
const ROW = /^\| \[(\d{4})\]\(([^)]+)\)/;

export const adrNumbers: Rule = {
  id: ID,
  description: 'each ADR number names one file, its heading says that number, and the index links match',
  run(ctx: Ctx) {
    const dir = join(ctx.root, DIR);
    if (!existsSync(dir)) return [];
    const out: Finding[] = [];
    const owner = new Map<string, string>();
    for (const name of readdirSync(dir).sort()) {
      const m = FILE.exec(name);
      if (!m?.[1]) continue;
      const num = m[1];
      const rel = `${DIR}/${name}`;
      const first = owner.get(num);
      if (first) out.push(finding(rel, 1, ID, `ADR-${num} is already ${first}: take the next free number and update every ADR-${num} this change cites`));
      else owner.set(num, name);
      const head = readFileSync(join(dir, name), 'utf8').split('\n', 1)[0] ?? '';
      if (!head.startsWith(`# ADR-${num}`)) out.push(finding(rel, 1, ID, `the heading should start "# ADR-${num}", as the filename does`));
    }
    const index = join(dir, 'README.md');
    if (!existsSync(index)) return out;
    readFileSync(index, 'utf8').split('\n').forEach((line, i) => {
      const m = ROW.exec(line);
      if (!m?.[1] || !m[2]) return;
      const [, num, link] = m;
      if (!existsSync(join(dir, link))) {
        out.push(finding(`${DIR}/README.md`, i + 1, ID, `${link} does not exist`, 'warn'));
        return;
      }
      if (!link.startsWith(`${num}-`)) out.push(finding(`${DIR}/README.md`, i + 1, ID, `the row says ${num} but links ${link}`));
    });
    return out;
  },
};
