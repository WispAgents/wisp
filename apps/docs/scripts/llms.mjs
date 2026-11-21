// Docs for agents too (ADR-0039): after the build, every page is also its markdown (`/docs/<page>.md`), `/docs/llms.txt`
// lists them with their one-line description (llmstxt.org), and `/docs/llms-full.txt` is all of them in one file.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const src = path.resolve(here, '../../../docs/site');
const dist = path.resolve(here, '../dist');
const SITE = 'https://trywisp.dev/docs';

/** An MDX page as plain markdown: no imports, and each App · TUI · CLI tab as a bold label over its text. */
function plain(body) {
  return body
    .replace(/^import .+ from '[^']+';\n+/gm, '')
    .replace(/^<Tabs[^>]*>\n?/gm, '')
    .replace(/^<\/Tabs>\n?/gm, '')
    .replace(/^<TabItem label="([^"]+)">\n?/gm, '**$1:**\n\n')
    .replace(/^<\/TabItem>\n?/gm, '\n')
    // cards as a plain list of links
    .replace(/<LinkCard\s+title="([^"]*)"(?:\s+description="([^"]*)")?\s+href="([^"]*)"\s*\/>/g, (_, t, d, h) => `- [${t}](https://trywisp.dev${h})${d ? `: ${d}` : ''}`)
    .replace(/^\s*<\/?CardGrid>\n?/gm, '')
    .replace(/^\s*<Card title="([^"]*)"[^>]*>\n?/gm, '**$1**\n')
    .replace(/^\s*<\/Card>\n?/gm, '\n')
    .replace(/\n{3,}/g, '\n\n');
}

/** The page's front matter (title, description, sidebar order) and its body. */
function parse(text) {
  const m = text.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
  const front = m?.[1] ?? '';
  const field = (k) => {
    const v = front.match(new RegExp(`^${k}:\\s*(.*)$`, 'm'))?.[1]?.trim() ?? '';
    return v.startsWith('"') ? JSON.parse(v) : v;
  };
  const order = Number(front.match(/^\s+order:\s*(\d+)/m)?.[1] ?? 999);
  return { title: field('title'), description: field('description'), order, body: plain((m?.[2] ?? text).trim()) };
}

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walk(p) : /\.mdx?$/.test(e.name) ? [p] : [];
  });
}

if (!fs.existsSync(dist)) throw new Error('llms: build the site first (astro build)');
const SECTIONS = ['start', 'concepts', 'guides', 'integrations', 'plugins', 'reference', 'migrate'];
const pages = walk(src)
  .map((file) => {
    const rel = path.relative(src, file).replace(/\\/g, '/').replace(/\.mdx?$/, '');
    const slug = rel === 'index' ? '' : rel.replace(/\/index$/, '');
    return { slug, ...parse(fs.readFileSync(file, 'utf8')) };
  })
  .sort((a, b) => {
    const sa = SECTIONS.indexOf(a.slug.split('/')[0]);
    const sb = SECTIONS.indexOf(b.slug.split('/')[0]);
    return sa - sb || a.slug.split('/').slice(0, -1).join('/').localeCompare(b.slug.split('/').slice(0, -1).join('/')) || a.order - b.order || a.slug.localeCompare(b.slug);
  });

for (const p of pages) {
  const file = path.join(dist, p.slug ? `${p.slug}.md` : 'index.md');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `# ${p.title}\n\n${p.description ? `> ${p.description}\n\n` : ''}${p.body}\n`);
}

const url = (p) => `${SITE}/${p.slug ? `${p.slug}.md` : 'index.md'}`;
const bySection = new Map();
for (const p of pages) {
  const s = p.slug.split('/')[0] || 'start';
  if (!bySection.has(s)) bySection.set(s, []);
  bySection.get(s).push(p);
}
const names = { start: 'Get started', concepts: 'Concepts', guides: 'Guides', integrations: 'Integrations', plugins: 'Build plugins', reference: 'Reference', migrate: 'Import a wisp home' };
const index = [
  '# wisp',
  '',
  '> Your coding agents, as code. wisp runs Claude Code, Codex, Cursor and every other coding agent from your repo, in parallel, on any of your computers or in the cloud, and keeps every task they did next to the code they wrote.',
  '',
  'Each page below is plain markdown. `llms-full.txt` has all of them in one file.',
  '',
  ...[...bySection].flatMap(([s, list]) => [`## ${names[s] ?? s}`, '', ...list.map((p) => `- [${p.title}](${url(p)})${p.description ? `: ${p.description}` : ''}`), '']),
].join('\n');
fs.writeFileSync(path.join(dist, 'llms.txt'), index);
fs.writeFileSync(path.join(dist, 'llms-full.txt'), pages.map((p) => `# ${p.title}\n\nSource: ${url(p)}\n\n${p.body}\n`).join('\n---\n\n'));
console.log(`llms: ${pages.length} pages as markdown, llms.txt, llms-full.txt`);
