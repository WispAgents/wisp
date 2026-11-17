// t('key'): this plugin's strings (src/i18n/en.json for what the first page may need, en.lazy.json for the rest,
// added by i18n/lazy.ts when the Thread loads), registered with the client through the catalogue in index.ts. Keys
// are typed: a missing string fails typecheck. `{name}` placeholders; `key.one` / `key.other` with a numeric count.
import en from './i18n/en.json' with { type: 'json' };

type LazyJson = typeof import('./i18n/en.lazy.json', { with: { type: 'json' } });
type Lazy = LazyJson extends { default: infer D } ? D : LazyJson;
type Raw = keyof typeof en | keyof Lazy;
type Base<K> = K extends `${infer B}.one` ? B : K extends `${infer B}.other` ? B : K;
export type Key = Base<Raw> | Raw;
export type Vars = Readonly<Record<string, string | number>>;

export const catalogue: Readonly<Record<string, string>> = en;

const fill = (s: string, vars: Vars | undefined): string =>
  vars ? s.replace(/\{(\w+)\}/g, (m, name: string) => (Object.hasOwn(vars, name) ? String(vars[name]) : m)) : s;

export function t(key: Key, vars?: Vars): string {
  const count = vars?.['count'];
  const plural = typeof count === 'number' ? catalogue[`${key}.${count === 1 ? 'one' : 'other'}`] : undefined;
  return fill(plural ?? catalogue[key] ?? key, vars);
}
