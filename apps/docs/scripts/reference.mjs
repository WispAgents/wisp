// The reference pages, generated from the code (ADR-0039: never written by hand): the CLI from its usage text
// (crates/wisp/src/args.rs HELP), every plugin's commands and settings from its manifest (plugins/*/host/wisp-plugin.json),
// the MCP tools from their definitions (plugins/mcp/core/defs), the answer blocks from the protocol agents are given
// (plugins/runtime/core/tests/goldens/system-prompt.txt). Written to docs/site/reference (git-ignored) before every
// dev and build, so a page can't go stale.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const out = path.join(repo, 'docs/site/reference');
const read = (p) => fs.readFileSync(path.join(repo, p), 'utf8');
const esc = (s) => String(s ?? '').replace(/\|/g, '\\|').replace(/\n+/g, ' ').trim();
const page = (file, title, description, order, body) => {
  const front = ['---', `title: ${JSON.stringify(title)}`, `description: ${JSON.stringify(description)}`, `sidebar:`, `  order: ${order}`, '---', ''];
  fs.writeFileSync(path.join(out, file), [...front, '> Generated from the code by `apps/docs/scripts/reference.mjs`: edit the source, not this page.', '', body.trim(), ''].join('\n'));
};

/** Every built-in plugin's manifest, by id. */
function manifests() {
  const dir = path.join(repo, 'plugins');
  return fs
    .readdirSync(dir)
    .map((name) => path.join(dir, name, 'host/wisp-plugin.json'))
    .filter((f) => fs.existsSync(f))
    .map((f) => JSON.parse(fs.readFileSync(f, 'utf8')))
    .sort((a, b) => a.id.localeCompare(b.id));
}

function cli() {
  const src = read('crates/wisp/src/args.rs');
  const m = src.match(/pub const HELP: &str = "([\s\S]*?)";\n/);
  if (!m) throw new Error('reference: the CLI usage text (HELP in crates/wisp/src/args.rs) moved');
  const help = m[1].replace(/\\"/g, '"').replace(/\\\\/g, '\\');
  page(
    'cli.md',
    'CLI',
    'Every wisp command and option, as `wisp --help` prints it.',
    1,
    `\`wisp --help\` prints this. \`wisp help <plugin> [verb]\` lists a plugin's verbs and a verb's flags: every plugin command below is also \`wisp <plugin> <verb>\` (see [Commands](../commands/)).

**Commands and tasks.** Words that aren't a command are a task: \`wisp fix the flaky test\` starts one. A line whose first word is one of wisp's command groups (\`computer\`, \`cloud\`, \`grid\`, \`crew\`, \`trigger\`, \`plugin\`, \`recipes\`, \`machines\`, \`machine\`, \`share\`, \`version\`, \`skill\`, \`mcp\`, …) is always that command, never a task; \`update\` is too when one word follows (\`wisp update apply\`), while \`wisp update the readme\` is a task. A few command words also start everyday prompts and are a task when their command's argument isn't there: \`wisp ship the docs\`, \`wisp run the tests\`, \`wisp plan the migration\` (\`ship\`, \`merge\`, \`run\`, \`plan\`, \`init\`, \`notes\`, \`export\`, …). A verb the group doesn't have (\`wisp computer start\`), a flag its command doesn't take, or words it doesn't take print the group's verbs or the command's usage and exit 2. To start a task that begins with such a word, quote it as one argument (\`wisp "computer start fails on boot, fix it"\`) or put \`--\` first (\`wisp -- computer start fails on boot\`). A plugin's name is its command only when one of its verbs follows (\`wisp channels refresh\`, where a bad flag is an error too); otherwise the line is a task, so \`wisp review the auth module\` and \`wisp update the readme\` start tasks.

\`\`\`text
${help.trim()}
\`\`\``,
  );
}

function commands(all) {
  const parts = [
    'Every command a plugin contributes. The app, the TUI, the CLI (`wisp <plugin> <verb>`), the SDKs, MCP tools and other plugins all call these. **Reads** marks a command that changes nothing.',
  ];
  for (const m of all) {
    const list = m.contributes?.commands ?? [];
    if (!list.length) continue;
    parts.push(`\n## ${m.id}\n\n${esc(m.description)}\n\n| Command | What it does | Reads |\n| --- | --- | --- |`);
    for (const c of list) parts.push(`| \`${c.id}\` | ${esc(c.title)} | ${c.readOnly ? 'yes' : ''} |`);
  }
  page('commands.md', 'Commands', 'Every command each built-in plugin contributes, from its manifest.', 2, parts.join('\n'));
}

function settings(all) {
  const parts = [
    'Every setting, by plugin, as Settings shows it and `wisp.yml` sets it (`settings:` with the key). The default applies until you change it.',
  ];
  for (const m of all) {
    const list = m.contributes?.settings ?? [];
    if (!list.length) continue;
    parts.push(`\n## ${m.id}\n\n| Key | Type | Default | What it does |\n| --- | --- | --- | --- |`);
    for (const s of list) {
      const type = s.values ? s.values.map((v) => `\`${v}\``).join(' · ') : s.type;
      const def = s.default === undefined ? '' : `\`${JSON.stringify(s.default)}\``;
      parts.push(`| \`${s.key}\` | ${type} | ${def} | ${esc(s.title)} |`);
    }
  }
  page('settings.md', 'Settings', 'Every setting each built-in plugin declares, with its type and default.', 3, parts.join('\n'));
}

function mcp() {
  const tools = (f) => JSON.parse(read(`plugins/mcp/core/defs/${f}.json`));
  const section = (title, intro, list) => {
    const rows = list.map((t) => {
      const args = Object.entries(t.inputSchema?.properties ?? {})
        .map(([k, v]) => `\`${k}\`${(t.inputSchema?.required ?? []).includes(k) ? '' : '?'}`)
        .join(', ');
      return `| \`${t.name}\` | ${esc(t.description)} | ${args} |`;
    });
    return `## ${title}\n\n${intro}\n\n| Tool | What it does | Arguments (? optional) |\n| --- | --- | --- |\n${rows.join('\n')}`;
  };
  page(
    'mcp-tools.md',
    'MCP tools',
    'The tools wisp gives an agent running in a task, and the ones `wisp mcp` gives your own agent or editor.',
    4,
    [
      'wisp serves its tools over MCP. A task\'s agent gets them on every run; `wisp mcp` serves them (stdio) to any agent or editor you point at it, so you can drive wisp from where you already work.',
      section('In a task', 'What an agent wisp runs can call.', tools('agent')),
      section('From your agent or editor (`wisp mcp`)', 'What `wisp mcp` offers outside a task.', tools('user')),
    ].join('\n\n'),
  );
}

function blocks() {
  const prompt = read('plugins/runtime/core/tests/goldens/system-prompt.txt');
  const at = prompt.indexOf('### Blocks');
  if (at < 0) throw new Error('reference: the blocks section of the protocol moved');
  const lines = prompt.slice(at).split('\n').slice(1);
  const items = [];
  for (const l of lines) {
    const m = l.match(/^- (\{"type":"([a-z]+)".*)$/);
    if (m) items.push({ type: m[2], shape: m[1], more: [] });
    else if (items.length && l.startsWith('  ') && l.trim()) items.at(-1).more.push(l.trim());
    else if (l.startsWith('## ') || l.startsWith('### ')) break;
  }
  const body = items
    .map((b) => `## \`${b.type}\`\n\n\`\`\`json\n${b.shape}\n\`\`\`\n${b.more.length ? `\n${b.more.join(' ')}\n` : ''}`)
    .join('\n');
  page(
    'answer-blocks.md',
    'Answer blocks',
    'The blocks an agent ends its answer with, which wisp draws: the shape and the rule for each.',
    5,
    `An agent wisp runs ends its final message with a \`\`\`wisp block: \`{"blocks": [...]}\`. wisp draws each block (the diff, the tests, the preview…) and hides the rest of the message. This is the catalogue the agent is given, word for word.\n\n${body}`,
  );
}

/** A JSON Schema type in words: `string`, `[agent]`, `a | b`. */
function typeOf(v) {
  if (!v || typeof v !== 'object') return '';
  if (v.$ref) return v.$ref.split('/').pop();
  if (v.const !== undefined) return `\`${JSON.stringify(v.const)}\``;
  if (v.enum) return v.enum.map((e) => `\`${e}\``).join(' · ');
  if (v.anyOf || v.oneOf) return [...new Set((v.anyOf ?? v.oneOf).map(typeOf).filter((t) => t && t !== 'null'))].join(' | ');
  if (v.allOf) return typeOf(v.allOf[0]);
  if (v.type === 'array') return `[${typeOf(v.items) || 'any'}]`;
  if (v.type === 'object' && v.additionalProperties && typeof v.additionalProperties === 'object') return `map of ${typeOf(v.additionalProperties) || 'any'}`;
  return Array.isArray(v.type) ? v.type.filter((t) => t !== 'null').join(' | ') : (v.type ?? '');
}

function fields(def) {
  const req = new Set(def.required ?? []);
  const rows = Object.entries(def.properties ?? {}).map(([k, v]) => `| \`${k}\`${req.has(k) ? '' : '?'} | ${esc(typeOf(v))} | ${esc(v.description)} |`);
  return rows.length ? `| Field (? optional) | Type | What it is |\n| --- | --- | --- |\n${rows.join('\n')}` : '';
}

function kinds() {
  const schema = JSON.parse(read('apps/docs/data/kinds.schema.json'));
  const defs = schema.$defs ?? {};
  const parts = [
    'Everything a formula (`.wisp/*.wisp.yml`, or `wisp apply -f file`) can declare, one section per top-level key. Each key takes a list of named items (`- name: reviewer`). `wisp schema` prints the same as JSON Schema, for your editor.',
    `## The formula\n\n${fields(schema)}`,
  ];
  for (const [key, v] of Object.entries(schema.properties ?? {})) {
    const ref = JSON.stringify(v).match(/"\$ref":"#\/\$defs\/([a-z]+)"/)?.[1];
    if (!ref || !defs[ref]) continue;
    const d = defs[ref];
    parts.push(`## \`${key}:\` (${ref})\n\n${esc(d.description)}\n\n${fields(d)}`);
    for (const [name, sub] of Object.entries(defs).filter(([n]) => n.startsWith(`${ref}.`))) {
      parts.push(`### ${name}\n\n${esc(sub.description)}\n\n${fields(sub)}`);
    }
  }
  page('kinds.md', 'Resource kinds', 'Every kind a formula can declare, with its fields, from the schema wisp checks it with.', 6, parts.join('\n\n'));
}

function triggers(all) {
  const rows = [];
  for (const m of all) {
    for (const t of m.contributes?.triggerSources ?? []) rows.push(`| \`${t}\` | ${m.id} | a source |`);
    for (const e of m.contributes?.events ?? []) if (e && typeof e === 'object' && e.durable) rows.push(`| \`${e.type}\` | ${m.id} | an event |`);
  }
  rows.sort();
  page(
    'triggers.md',
    'Events and trigger sources',
    "Every value a trigger's `on` takes, and the plugin that offers it.",
    8,
    `A trigger starts work when something happens. Its \`on\` is one of these. A trigger whose plugin is off stays declared but doesn't fire until you switch the plugin on. See [Schedules and triggers](../../guides/run/schedules-and-triggers/) and the trigger's fields in [Resource kinds](../kinds/).\n\n| \`on:\` | Offered by | Kind |\n| --- | --- | --- |\n${rows.join('\n')}`,
  );
}

function pluginApi() {
  const m = JSON.parse(read('plugins/plugins/core/data/manifest-fields.json'));
  const kit = JSON.parse(read('packages/shell/src/remote/kit.json'));
  // the allow-list the kernel checks a theme with and the shell applies it with (ADR-0007)
  const styles = JSON.parse(read('packages/shell/src/theme-styles/part-styles.json'));
  const rows = (o) => Object.entries(o).map(([k, v]) => `| \`${k}\` | ${esc(v)} |`).join('\n');
  const comps = Object.entries(kit.components ?? {})
    .map(([name, props]) => {
      const p = Object.entries(props)
        .map(([k, v]) => `\`${k}\`: ${Array.isArray(v) ? v.map((x) => `\`${x}\``).join(' · ') : esc(typeof v === 'object' ? JSON.stringify(v) : v)}`)
        .join('; ');
      const kids = (kit.parents ?? []).includes(name) ? ' Takes children (`k`).' : '';
      const ev = kit.events?.[name] ? ` Fires \`on\`: ${kit.events[name]}.` : '';
      return `| \`${name}\` | ${p}${kids}${ev} |`;
    })
    .join('\n');
  const places = Object.entries(kit.places ?? {})
    .map(([id, p]) => `| \`${id}\` | ${esc(p.words)} | ${p.drawn ? 'yes' : 'not drawn yet'} |`)
    .join('\n');
  const l = kit.limits ?? {};
  page(
    'plugin-api.md',
    'Plugin manifest and UI',
    "Every key of a plugin's wisp-plugin.json, and the components a plugin's UI can use, from the checker's own data.",
    7,
    `A plugin is a folder with a \`wisp-plugin.json\` and, for UI, one \`.js\` module. wisp checks the manifest against these keys: an unknown key is refused, with the one you probably meant. Start with [Your first plugin](../../plugins/first-plugin/).

## Manifest keys

| Key | What it is |
| --- | --- |
${rows(m.keys ?? {})}

## \`contributes\`

| Key | What it adds |
| --- | --- |
${rows(m.contributes ?? {})}

## The UI module

A \`contributes.ui\` entry's \`component\` is a \`.js\` file in the plugin's folder. It runs in a Worker in a sandboxed frame: no DOM, no network, no storage, no imports, one file of at most ${Math.round((l.module ?? 0) / 1024)} KB. Its \`wisp\` has only:

- \`wisp.render(tree)\`: draw this tree of components (call it again to update; nodes update in place).
- \`wisp.call(command, args)\`: run a wisp command as the plugin, checked against the plugin's own permissions (never yours). Every command is in [Commands](../commands/).
- \`wisp.on(event, fn)\`: \`fn(value)\` runs when a component with \`on: event\` fires.

A node is a string (text) or \`{ c: 'Component', p: { …props }, k: [ …children ], on: 'event' }\`. No HTML, no class, no style: anything not listed below is dropped, so a plugin always looks like wisp, in light and dark.

| Component | Props |
| --- | --- |
${comps}

## Where a plugin's UI shows (\`attachTo.id\`)

| Place | Where | Drawn |
| --- | --- | --- |
${places}

Limits: ${l.text} characters per text, ${l.nodes} nodes, ${l.depth} levels, ${l.items} items per Select.

## Themes (\`partStyles\`)

A theme is a plugin with only a manifest: \`contributes.partStyles\` maps the \`data-part\` names of what you want to restyle (see them in the browser's inspector: \`data-part="composer.send"\`) to CSS declarations. wisp writes them into the app's \`@layer plugins\`, after its own styles, so they win without \`!important\`. They apply while the plugin is on **and you trusted it**: a plugin you install is untrusted until you say otherwise, and an untrusted plugin's styles never apply.

\`\`\`sh
wisp plugin install ./pink      # installed, off and untrusted
wisp plugin trust user.pink     # its styles may apply
wisp plugin enable user.pink    # on: the app restyles at once; disable or remove it and they go
\`\`\`

\`\`\`json
"partStyles": {
  ":root": { "--accent": "#d6336c" },
  "composer.send": { "background": "#d6336c", "color": "#ffffff", "border-radius": "999px" },
  "@dark": { ":root": { "--accent": "#ff7aa8" }, "composer.send": { "background": "#ff7aa8", "color": "#1a1a1a" } }
}
\`\`\`

- A key is a part name (\`composer.send\`: lowercase, dot-namespaced), \`${styles.tokens}\` for token overrides (custom properties only, like \`--accent\`), or ${styles.groups.map((g) => `\`${g}\``).join(' / ')} holding the same map for that theme only (when the app's theme is set to it, or follows the system and the system is in it).
- A property is a custom property (\`--anything\`) or one of: ${styles.properties.map((x) => `\`${x}\``).join(', ')}.
- A value is plain CSS of at most ${styles.maxValue} characters: no \`;\`, braces, \`<\`, \`>\`, \`\\\`, \`@\`, \`!\`, comments or unbalanced quotes; \`url()\` only with a \`data:image/\` URL, and no \`image-set()\`: a theme fetches nothing. At most ${styles.maxRules} declarations. Anything else is refused when the plugin installs.`,
  );
}

function pluginExamples() {
  const list = JSON.parse(read('plugins/plugins/core/data/plugin-examples.json'));
  const body = list
    .map((x) => {
      const files = Object.entries(x.files ?? {}).map(([name, text]) => `\`${name}\`\n\n\`\`\`js\n${text.trim()}\n\`\`\``);
      return `## ${x.manifest.name}\n\n*Asked for:* ${x.ask}\n\n\`wisp-plugin.json\`\n\n\`\`\`json\n${JSON.stringify(x.manifest, null, 2)}\n\`\`\`\n\n${files.join('\n\n')}`;
    })
    .join('\n\n');
  page(
    'plugin-examples.md',
    'Plugin examples',
    'Whole plugins, manifest and code, that wisp checks in its own tests: copy one into ~/.wisp/plugins/<name> to start.',
    9,
    `Each is a complete plugin: its manifest passes wisp's own check in its tests, and agents that write plugins for you are shown the same ones. Put the files in \`~/.wisp/plugins/<name>/\`, then \`wisp plugin enable <id>\`. See [Build plugins](../../plugins/first-plugin/).\n\n${body}`,
  );
}

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
const all = manifests();
cli();
commands(all);
settings(all);
mcp();
blocks();
kinds();
triggers(all);
pluginApi();
pluginExamples();
console.log(`reference: ${fs.readdirSync(out).length} pages from ${all.length} plugins`);
