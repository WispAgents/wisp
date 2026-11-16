// i18n (plan §5.1): every user-facing string goes through t('key'). Catalogues come from plugin
// manifests, one per plugin, English only for now; a key belongs to the plugin whose id prefixes
// it. A missing key renders as the key itself and warns once in dev builds, so a gap is visible
// without breaking the screen.

import type { Scope } from './scope.ts';
import type { Warn } from './types.ts';

export type Catalogue = Readonly<Record<string, string>>;
export type Vars = Readonly<Record<string, string | number>>;

export interface I18n {
  /** registers `plugin`'s strings while `scope` lives */
  register(plugin: string, catalogue: Catalogue, scope: Scope): void;
  /**
   * `{name}` is replaced by vars.name. With a numeric `count`, `key.one` / `key.other` are tried
   * before `key` (English plural rules).
   */
  t(key: string, vars?: Vars): string;
  has(key: string): boolean;
}

export function createI18n(o: { dev?: boolean; warn?: Warn } = {}): I18n {
  const owners = new Map<string, Catalogue>(); // plugin → its strings
  const warned = new Set<string>();

  // what each key read last time, until a catalogue comes or goes: a lookup walked every plugin's catalogue (~60) for
  // each string, 11.5 ms of a Mark done's frame (the flicker gate's profile). Only what was found: a plugin's lazy half
  // adds strings to its catalogue later, so a miss is looked up again
  const found = new Map<string, string>();
  const search = (key: string): string | undefined => {
    for (const [plugin, cat] of owners) {
      if (key !== plugin && !key.startsWith(`${plugin}.`)) continue;
      const s = cat[key];
      if (s !== undefined) return s;
    }
    return undefined;
  };
  const lookup = (key: string): string | undefined => {
    const had = found.get(key);
    if (had !== undefined) return had;
    const s = search(key);
    if (s !== undefined) found.set(key, s);
    return s;
  };
  const fill = (s: string, vars: Vars | undefined): string =>
    vars ? s.replace(/\{(\w+)\}/g, (m, name: string) => (Object.hasOwn(vars, name) ? String(vars[name]) : m)) : s;

  return {
    register(plugin, catalogue, scope) {
      if (owners.has(plugin)) throw new Error(`strings for ${plugin} are registered twice`);
      for (const key of Object.keys(catalogue)) {
        if (!key.startsWith(`${plugin}.`)) throw new Error(`${plugin} can't define the string ${key}: keys start with "${plugin}."`);
      }
      owners.set(plugin, catalogue);
      found.clear();
      scope.add(() => {
        owners.delete(plugin);
        found.clear();
      });
    },
    t(key, vars) {
      const count = vars?.['count'];
      const plural = typeof count === 'number' ? lookup(`${key}.${count === 1 ? 'one' : 'other'}`) : undefined;
      const s = plural ?? lookup(key);
      if (s !== undefined) return fill(s, vars);
      if (o.dev && !warned.has(key)) {
        warned.add(key);
        o.warn?.(`missing string ${key}`);
      }
      return key;
    },
    has: (key) => lookup(key) !== undefined,
  };
}
