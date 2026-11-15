// ADR-0011: each package's English strings are split into the ones its first-page code uses (i18n/en*.json, the
// shell's i18n/en.ts) and the ones only its lazily loaded code uses (i18n/en.lazy.json), which i18n/lazy.ts adds to the
// catalogue when a module that needs them loads. A key lives in one half only, and every lazy half has its loader,
// which the modules using its strings import.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const owners = [
  'packages/shell/src',
  ...readdirSync(`${repo}plugins`)
    .map((p) => `plugins/${p}/ui/src`)
    .filter((d) => existsSync(`${repo}${d}/i18n/en.lazy.json`)),
];
const json = (f: string): Record<string, string> => JSON.parse(readFileSync(f, 'utf8')) as Record<string, string>;
const eagerKeys = (dir: string): string[] =>
  dir === 'packages/shell/src'
    ? [...readFileSync(`${repo}${dir}/i18n/en.ts`, 'utf8').matchAll(/^ {2}'([^']+)':/gm)].map((m) => m[1] ?? '')
    : readdirSync(`${repo}${dir}/i18n`)
        .filter((f) => /^en(\.\w+)?\.json$/.test(f) && f !== 'en.lazy.json')
        .flatMap((f) => Object.keys(json(`${repo}${dir}/i18n/${f}`)));
const walk = (d: string): string[] => readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(`${d}/${e.name}`) : /\.tsx?$/.test(e.name) ? [`${d}/${e.name}`] : []));

describe('the strings split', () => {
  it('has packages with lazy strings', () => {
    expect(owners.length).toBeGreaterThan(10);
  });

  for (const dir of owners) {
    it(`${dir}: a key in one half only, and the lazy half has its loader, imported by its users`, () => {
      const lazy = Object.keys(json(`${repo}${dir}/i18n/en.lazy.json`));
      const eager = new Set(eagerKeys(dir));
      expect(lazy.filter((k) => eager.has(k))).toEqual([]);
      expect(existsSync(`${repo}${dir}/i18n/lazy.ts`)).toBe(true);
      // every module that names a lazy key literally imports the loader (else that string would show as its key)
      const set = new Set(lazy.flatMap((k) => [k, k.replace(/\.(one|other)$/, '')]));
      const missing = walk(`${repo}${dir}`)
        .filter((f) => !f.endsWith('/i18n/lazy.ts'))
        .filter((f) => {
          const src = readFileSync(f, 'utf8');
          return [...src.matchAll(/['"`]([a-z][\w-]*\.[\w.-]+)['"`]/g)].some((m) => set.has(m[1] ?? '')) && !/import '[./]*(i18n\/)?lazy\.ts';/.test(src);
        })
        .map((f) => f.slice(repo.length));
      expect(missing).toEqual([]);
    });
  }

  // shell.tsx register() picks a catalogue's halves (namespace keys, full-id keys) from the keys there at start, and
  // registers nothing for an empty one: strings lazy.ts adds later show only through a half registered then
  for (const dir of owners.filter((d) => d.startsWith('plugins/'))) {
    it(`${dir}: the eager half keeps a key of each kind its strings use, so its lazy strings show`, () => {
      const id = /id:\s*'([\w.-]+)'/.exec(readFileSync(`${repo}${dir}/index.ts`, 'utf8'))?.[1] ?? '';
      const ns = id.replace(/^wisp\./, '');
      const kind = (k: string): string => (id !== ns && k.startsWith(`${id}.`) ? 'id' : 'ns');
      const all = [...eagerKeys(dir), ...Object.keys(json(`${repo}${dir}/i18n/en.lazy.json`))];
      expect(new Set(eagerKeys(dir).map(kind))).toEqual(new Set(all.map(kind)));
    });
  }
});
