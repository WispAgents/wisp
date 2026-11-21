// A trusted theme's `contributes.partStyles` (ADR-0007) as CSS for `@layer plugins`. Pure: manifest maps in, one
// stylesheet's text out. The allow-list is part-styles.json beside this file, the same one the kernel's manifest
// check reads (crates/wisp-kernel/src/part_styles.rs), so a manifest that installed is exactly what applies here;
// anything else is dropped, declaration by declaration, never passed through:
//
//   { "composer.send": { "background": "var(--accent)" },     → [data-part~="composer.send"] { background: … }
//     ":root": { "--accent": "#e0457b" },                      → :root { --accent: … }   (token overrides)
//     "@dark": { ":root": { "--accent": "#ff7aa8" } } }        → dark only: [data-theme=dark], or the OS's dark
//                                                                 while the theme follows the system
//
// A part name is checked (lowercase, dot-namespaced), so `[data-part~="…"]` can't be closed from inside; a value
// can't hold `; { } < > \ @ !`, a comment, a control character or an odd quote, so it can't end its declaration or
// its rule, open an @import, or win with !important; url() only takes a data:image/ URL, and image-set()/src() are
// refused, so a theme fetches nothing from anywhere.

import ALLOW from './part-styles.json' with { type: 'json' };

/** One trusted plugin's styles, as `plugins.styles` answers. */
export interface PluginStyles {
  readonly plugin: string;
  readonly partStyles: unknown;
}

const PROPS: ReadonlySet<string> = new Set(ALLOW.properties);
const GROUPS: readonly string[] = ALLOW.groups;
const NAME = /^[a-z0-9][a-z0-9_-]*(\.[a-z0-9_-]+)+$/;
const CUSTOM = /^--[a-z0-9][a-z0-9_-]*$/;
const BAD_CHAR = /[;{}<>\\@!\u0000-\u001f\u007f]/;

export function partOk(name: string): boolean {
  return name.length <= ALLOW.maxName && NAME.test(name);
}

/** May a theme set `prop`? Under `:root` only custom properties (tokens). */
export function propertyOk(prop: string, tokens: boolean): boolean {
  return (prop.length <= ALLOW.maxName && CUSTOM.test(prop)) || (!tokens && PROPS.has(prop));
}

/** Why `v` can't be a value, or undefined when it can (the kernel's value_problem, rule for rule). */
export function valueProblem(v: string): string | undefined {
  if (v.trim() === '' || v.length > ALLOW.maxValue) return 'a value is 1 to 200 characters';
  if (BAD_CHAR.test(v) || v.includes('/*') || v.includes('*/')) return "a value can't hold ; { } < > \\ @ ! or a comment";
  if ((v.match(/"/g)?.length ?? 0) % 2 === 1 || (v.match(/'/g)?.length ?? 0) % 2 === 1) return "a value's quotes must be balanced";
  const low = v.toLowerCase();
  if (low.includes('image-set(') || low.includes('src(') || low.includes('expression(')) return "a value can't fetch anything (image-set, src)";
  for (let i = low.indexOf('url('); i >= 0; i = low.indexOf('url(', i + 4)) {
    const arg = low.slice(i + 4).trimStart().replace(/^["']/, '');
    if (!arg.startsWith('data:image/')) return 'url() takes only a data:image/ URL';
  }
  return undefined;
}

type Decls = [prop: string, value: string][];

function decls(v: unknown, tokens: boolean): Decls {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return [];
  const out: Decls = [];
  for (const [prop, value] of Object.entries(v)) {
    if (typeof value === 'string' && propertyOk(prop, tokens) && valueProblem(value) === undefined) out.push([prop, value.trim()]);
  }
  return out;
}

interface Rule {
  readonly selector: string;
  readonly decls: Decls;
}

function rules(map: unknown, nested: boolean, out: Map<string, Rule[]>, group: string): void {
  if (typeof map !== 'object' || map === null || Array.isArray(map)) return;
  for (const [key, v] of Object.entries(map)) {
    if (GROUPS.includes(key)) {
      if (!nested) rules(v, true, out, key);
      continue;
    }
    const tokens = key === ALLOW.tokens;
    if (!tokens && !partOk(key)) continue;
    const d = decls(v, tokens);
    if (d.length === 0) continue;
    out.set(group, [...(out.get(group) ?? []), { selector: tokens ? ':root' : `[data-part~="${key}"]`, decls: d }]);
  }
}

const block = (sel: string, d: Decls): string => `${sel} { ${d.map(([p, v]) => `${p}: ${v};`).join(' ')} }`;

/** The selector a rule gets in a theme group: dark when the theme is dark, or the OS is and the theme follows it. */
function scoped(group: string, r: Rule): string[] {
  if (group === '') return [block(r.selector, r.decls)];
  const mode = group === '@dark' ? 'dark' : 'light';
  const other = mode === 'dark' ? 'light' : 'dark';
  const inner = r.selector === ':root' ? '' : ` ${r.selector}`;
  return [
    block(`:root[data-theme="${mode}"]${inner}`, r.decls),
    `@media (prefers-color-scheme: ${mode}) { ${block(`:root:not([data-theme="${other}"])${inner}`, r.decls)} }`,
  ];
}

/** The stylesheet for every trusted plugin's styles, in order (a later plugin wins a tie), or '' when there's none. */
export function partStylesCss(list: readonly PluginStyles[]): string {
  const lines: string[] = [];
  for (const p of list) {
    let count = 0;
    const groups = new Map<string, Rule[]>();
    rules(p.partStyles, false, groups, '');
    const body: string[] = [];
    for (const group of ['', ...GROUPS]) {
      for (const r of groups.get(group) ?? []) {
        count += r.decls.length;
        if (count > ALLOW.maxRules) break; // the kernel refuses more; a copy that has more stops here
        body.push(...scoped(group, r));
      }
    }
    // the plugin id only in a comment, and only if it can't end one
    if (body.length > 0) lines.push(`/* ${/^[a-z0-9._-]+$/.test(p.plugin) ? p.plugin : 'plugin'} */`, ...body);
  }
  return lines.length === 0 ? '' : `@layer plugins {\n${lines.join('\n')}\n}\n`;
}
