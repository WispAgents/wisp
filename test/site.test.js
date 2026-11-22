// scripts/site.mjs: trywisp.dev generated pages, Wisp Agents branding, and SEO invariants.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, markdown, AGENTS } from '../scripts/site.mjs';
import { MOVED_GUIDES } from '../scripts/guides.mjs';

const DOCS = fileURLToPath(new URL('../docs/', import.meta.url));
const SITE = 'https://trywisp.dev';

const out = fs.mkdtempSync(path.join(os.tmpdir(), 'wisp-site-'));
const pages = build(out);
const read = (p) => fs.readFileSync(path.join(out, p.file), 'utf8');

test('every indexable page has Wisp Agents branding, SEO-sized title/description, canonical, one h1, no heading skips, and valid JSON-LD', () => {
  const titles = new Set();
  const descs = new Set();
  for (const p of pages.filter((x) => x.file && !x.noindex)) {
    const html = read(p);
    const title = html.match(/<title>(.*?)<\/title>/)?.[1];
    assert.ok(title, p.path);
    assert.ok(!titles.has(title), `duplicate title ${title}`);
    assert.ok(title.includes('Wisp Agents'), `${p.path}: title mentions Wisp Agents (${title})`);
    assert.ok(title.length <= 60, `${p.path}: title is ${title.length} chars (${title})`);
    titles.add(title);

    const desc = html.match(/<meta name="description" content="([^"]*)"/)?.[1];
    assert.ok(desc, `${p.path}: has description`);
    assert.ok(!descs.has(desc), `duplicate description on ${p.path}`);
    assert.ok(desc.length >= 70 && desc.length <= 160, `${p.path}: description is ${desc.length} chars`);
    descs.add(desc);

    assert.equal(html.match(/<h1[\s>]/g)?.length, 1, `${p.path}: one h1`);
    const levels = [...html.matchAll(/<h([1-6])[\s>]/g)].map((m) => Number(m[1]));
    for (let i = 1; i < levels.length; i++) {
      assert.ok(levels[i] <= levels[i - 1] + 1, `${p.path}: heading skip h${levels[i - 1]} -> h${levels[i]}`);
    }

    assert.ok(html.includes(`<link rel="canonical" href="${SITE}${p.path}" />`), p.path);
    assert.ok(html.includes('<meta property="og:site_name" content="Wisp Agents" />'), p.path);
    assert.ok(html.includes('<meta property="og:locale" content="en_US" />'), p.path);
    assert.ok(html.includes('<meta name="twitter:image:alt" content="Wisp Agents:'), p.path);
    assert.ok(html.includes('>Wisp Agents</a>'), `${p.path}: brand link says Wisp Agents`);

    const ld = [...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/gs)];
    assert.ok(ld.length >= 1, `${p.path}: has JSON-LD`);
    for (const m of ld) JSON.parse(m[1]);
  }
});

test('404 and redirects carry noindex and omit duplicate canonical to self', () => {
  for (const p of pages.filter((x) => x.noindex)) {
    const html = read(p);
    assert.match(html, /<meta name="robots" content="noindex/, p.path);
    assert.ok(!html.includes(`<link rel="canonical" href="${SITE}${p.path}"`), `${p.path}: no self-canonical`);
  }
});

test('internal assets and non-docs links resolve', () => {
  for (const p of pages.filter((x) => x.file && !x.noindex)) {
    const html = read(p);
    const refs = [
      ...[...html.matchAll(/(?:href|src)="(\/[^"#?]*)/g)].map((m) => m[1]),
      ...[...html.matchAll(/srcset="([^"]+)"/g)].flatMap((m) => m[1].split(',').map((s) => s.trim().split(/\s+/)[0]).filter((u) => u.startsWith('/'))),
    ];
    for (const href of refs) {
      if (href.startsWith('/docs/')) continue;
      if (href === '/sitemap.xml' || href === '/llms.txt') {
        assert.ok(fs.existsSync(path.join(out, href)), `${p.path} -> ${href}`);
        continue;
      }
      const rel = href.endsWith('/') ? `${href}index.html` : href;
      const found = [out, DOCS].some((dir) => fs.existsSync(path.join(dir, rel)));
      assert.ok(found, `${p.path} links to missing ${href}`);
    }
  }
});

test('sitemap, robots.txt and llms.txt are consistent and branded as Wisp Agents', () => {
  const xml = fs.readFileSync(path.join(out, 'sitemap.xml'), 'utf8');
  const locs = [...xml.matchAll(/<loc>(.*?)<\/loc>/g)].map((m) => m[1]);
  assert.deepEqual(locs, pages.filter((p) => !p.noindex).map((p) => `${SITE}${p.path}`));
  for (const a of AGENTS) assert.ok(locs.includes(`${SITE}/agents/${a.slug}.html`), a.slug);

  const robots = fs.readFileSync(path.join(out, 'robots.txt'), 'utf8');
  assert.match(robots, new RegExp(`Sitemap: ${SITE}/sitemap.xml`));
  assert.match(robots, new RegExp(`Sitemap: ${SITE}/docs/sitemap-index.xml`));

  const llms = fs.readFileSync(path.join(out, 'llms.txt'), 'utf8');
  assert.match(llms, /^# Wisp Agents/m);
});

test('moved guides stay alive as redirects', () => {
  assert.ok(MOVED_GUIDES.length >= 30);
  for (const [slug, to] of MOVED_GUIDES) {
    const html = fs.readFileSync(path.join(out, `guides/${slug}.html`), 'utf8');
    assert.ok(html.includes(`url=${to}`), slug);
    assert.match(html, /<meta name="robots" content="noindex, follow"/, slug);
  }
});

test('markdown: headings, nested lists, tables, code and links', () => {
  const html = markdown([
    '## The [MCP](#mcp) server',
    '',
    '- **One** with `code`',
    '  - nested *two*',
    '- three',
    '  wrapped',
    '',
    '| A | B |',
    '|---|---|',
    '| `x` | [docs](integrations.md#mcp) |',
    '',
    '```sh',
    'echo "<hi>"',
    '```',
    '',
    'See [the README](../README.md#cli).',
  ].join('\n'), 'https://github.com/WispAgents/wisp/blob/main/docs/');
  assert.match(html, /<h2 id="the-mcp-server">The <a href="#mcp">MCP<\/a> server<\/h2>/);
  assert.match(html, /<ul><li><strong>One<\/strong> with <code>code<\/code><ul><li>nested <em>two<\/em><\/li><\/ul><\/li><li>three wrapped<\/li><\/ul>/);
  assert.match(html, /<td><code>x<\/code><\/td><td><a href="\/integrations.html#mcp">docs<\/a><\/td>/);
  assert.match(html, /<pre data-lang="sh"><code>echo &quot;&lt;hi&gt;&quot;<\/code><\/pre>/);
  assert.match(html, /<a href="https:\/\/github.com\/WispAgents\/wisp\/blob\/main\/README.md#cli">the README<\/a>/);
});

test('the privacy policy keeps the contact and account deletion anchor Play links to', () => {
  const html = read(pages.find((p) => p.path === '/privacy.html'));
  assert.match(html, /<h2 id="delete-your-wisp-cloud-account">/);
  assert.match(html, /<a href="mailto:hello@trywisp.dev">/);
  assert.match(html, /<li><a href="#delete-your-wisp-cloud-account">/);
  assert.match(read(pages.find((p) => p.path === '/')), /<a href="\/privacy.html">Privacy<\/a>/);
  assert.match(read(pages.find((p) => p.path === '/cloud.html')), /<a href="\/privacy.html">Privacy<\/a>/);
  assert.match(read(pages.find((p) => p.path === '/changelog.html')), /<a href="\/privacy.html">Privacy<\/a>/);
});
