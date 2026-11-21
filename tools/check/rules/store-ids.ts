// A plugin's stores are named under its client namespace (`client.defineStore({ id })`): the client refuses any other
// id when the store is defined, which throws in the component that asked, so its screen shows nothing (B46: the goal
// UI's `wisp.goal.*` stores left Set goal… and New task's Judges empty; the namespace of `wisp.goal` is `goal`).
// Read where it's cheap to fix: every `defineStore({ id: '<literal>' })` under plugins/<p>/ui/src must start with
// the namespace of the id its src/index.ts gives definePluginUi (built-ins `wisp.<name>` use `<name>`, as
// @wisp/shell's namespaceOf), and under packages/shell/src with `shell.`. A computed id isn't read.

import ts from 'typescript';
import { finding } from '../core/report.ts';
import { lineOf, visit } from '../core/source.ts';
import type { Finding, Rule } from '../core/types.ts';

const ID = 'store-ids';

/** The client namespace of a plugin id, as @wisp/shell's namespaceOf. */
const namespaceOf = (id: string): string => (id.startsWith('wisp.') ? id.slice('wisp.'.length) : id);

export const storeIds: Rule = {
  id: ID,
  description: "a plugin's defineStore ids start with its client namespace (the client refuses others at run time)",
  run(ctx) {
    const out: Finding[] = [];
    const spaces = new Map<string, string | undefined>();
    const spaceOf = (file: string): string | undefined => {
      if (file.startsWith('packages/shell/src/')) return 'shell';
      const m = /^plugins\/([^/]+)\/ui\/src\//.exec(file);
      if (!m?.[1]) return undefined;
      if (!spaces.has(m[1])) {
        const index = `plugins/${m[1]}/ui/src/index.ts`;
        const id = ctx.exists(index) ? /definePluginUi\(\{\s*id:\s*'([^']+)'/.exec(ctx.read(index))?.[1] : undefined;
        spaces.set(m[1], id && namespaceOf(id));
      }
      return spaces.get(m[1]);
    };
    for (const file of ctx.files) {
      if (!/\.tsx?$/.test(file)) continue;
      const space = spaceOf(file);
      if (!space || !ctx.read(file).includes('defineStore')) continue;
      const sf = ctx.source(file);
      visit(sf, (n) => {
        if (!ts.isCallExpression(n)) return;
        const callee = n.expression;
        if (!ts.isPropertyAccessExpression(callee) || callee.name.text !== 'defineStore') return;
        const decl = n.arguments[0];
        if (!decl || !ts.isObjectLiteralExpression(decl)) return;
        const prop = decl.properties.find((p): p is ts.PropertyAssignment => ts.isPropertyAssignment(p) && ts.isIdentifier(p.name) && p.name.text === 'id');
        if (!prop || !ts.isStringLiteralLike(prop.initializer)) return;
        const id = prop.initializer.text;
        if (id.startsWith(`${space}.`)) return;
        out.push(finding(file, lineOf(sf, prop), ID, `the store ${JSON.stringify(id)} isn't under this plugin's namespace: the client refuses it when it's defined (the component that asks throws); name it "${space}.${id.split('.').at(-1) ?? id}"`));
      });
    }
    return out;
  },
};
