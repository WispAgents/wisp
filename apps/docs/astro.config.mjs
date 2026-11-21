// trywisp.dev/docs (ADR-0039): Starlight over docs/site, the reference generated from the code (scripts/reference.mjs),
// every page also as markdown with llms.txt (scripts/llms.mjs). Served from GitHub Pages under the site (pages.yml).
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// A sidebar group listing the pages under docs/site/<dir>, in their `sidebar.order`. Starlight 0.43 matches an
// autogenerate `directory` against each entry's `filePath` with `src/content/docs/` stripped (the only place it
// expects the collection), but our pages come from docs/site through a glob loader (src/content.config.ts), so
// their filePath is `../../docs/site/<dir>/<page>.md` (relative to apps/docs): a bare `directory: 'start'` matched
// nothing and every group rendered empty (2026-11-19). The directory is therefore spelled as filePath has it.
// scripts/check-sidebar.mjs fails the build if a group comes out empty again.
const DOCS = '../../docs/site';
const pages = (dir) => [{ autogenerate: { directory: `${DOCS}/${dir}` } }];

export default defineConfig({
  site: 'https://trywisp.dev',
  base: '/docs',
  trailingSlash: 'always',
  // the CSS goes in each page: no stylesheet request holds the first paint (about 200 ms on a slow phone)
  build: { inlineStylesheets: 'always' },
  // the guides (docs/site, outside this app) import the shared components as '@wisp-docs/Shot.astro'
  vite: { resolve: { alias: { '@wisp-docs': fileURLToPath(new URL('./src/components', import.meta.url)) } } },
  integrations: [
    starlight({
      title: 'wisp docs',
      description: 'Manage the code your agents write: run Claude Code, Codex and every coding agent from the app, the TUI or the CLI; declare them as code; track every line they write.',
      // every page: the site's card for links shared on X, Slack, LinkedIn…; the page's own title and description
      // come from Starlight (og:title, og:description, canonical)
      head: [
        { tag: 'meta', attrs: { property: 'og:image', content: 'https://trywisp.dev/assets/og.png' } },
        { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
        { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
        { tag: 'meta', attrs: { property: 'og:image:alt', content: 'wisp: manage the code your agents write' } },
        { tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
        { tag: 'meta', attrs: { name: 'twitter:site', content: '@trywispagents' } },
        { tag: 'meta', attrs: { name: 'twitter:image', content: 'https://trywisp.dev/assets/og.png' } },
        { tag: 'link', attrs: { rel: 'alternate', type: 'text/plain', href: '/docs/llms.txt', title: 'llms.txt' } },
        // How-to tabs (Desktop app · Phone app · TUI · CLI, synced as "way"): a reader on a phone who never picked one
        // gets Phone app first. Before paint, so Starlight's restore script draws it at once (no flash, no shift); with
        // JavaScript off the HTML's first tab, Desktop app, shows. The same key and labels as trywisp.dev's own tabs.
        // The pick also goes on <html data-way>: the desktop app's screenshots (Shot.astro) stay out of the layout, so
        // never downloaded, unless Desktop app is the tab shown, even when a slow phone lays the page out mid-parse.
        {
          tag: 'script',
          content:
            "try{var k='starlight-synced-tabs__way',W={'Desktop app':'desktop','Phone app':'phone','TUI':'tui','CLI':'cli'},r=document.documentElement,l=localStorage.getItem(k);if(!l){var n=navigator,m=n.userAgentData&&n.userAgentData.mobile;if(m||matchMedia('(max-width: 700px) and (pointer: coarse)').matches||/iPhone|iPod|Android.+Mobile/.test(n.userAgent)){l='Phone app';localStorage.setItem(k,l)}}r.dataset.way=W[l]||'desktop';var f=function(){setTimeout(function(){var t=document.querySelector('starlight-tabs[data-sync-key=way] [role=tab][aria-selected=true]');if(t&&W[t.textContent.trim()])r.dataset.way=W[t.textContent.trim()]},0)};document.addEventListener('click',f);document.addEventListener('keyup',f)}catch(e){}",
        },
      ],
      logo: { src: './src/assets/wisp.svg', replacesTitle: false },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/WispAgents/wisp' }],
      editLink: { baseUrl: 'https://github.com/WispAgents/wisp/edit/main/docs/site/' },
      lastUpdated: true,
      customCss: ['./src/styles/wisp.css'],
      // the alpha badge beside the title, on every page (as the site's header has it)
      components: { SiteTitle: './src/components/SiteTitle.astro' },
      sidebar: [
        { label: 'Get started', items: pages('start') },
        { label: 'Concepts', items: pages('concepts') },
        {
          label: 'Guides',
          items: [
            { label: 'Overview', link: '/guides/' },
            { label: 'Start with your agent', collapsed: true, items: pages('guides/agents') },
            { label: 'Apps', items: pages('guides/apps') },
            { label: 'Declare', items: pages('guides/declare') },
            { label: 'Run', items: pages('guides/run') },
            { label: 'Track', items: pages('guides/track') },
          ],
        },
        { label: 'Integrations', items: pages('integrations') },
        { label: 'Build plugins', items: pages('plugins') },
        { label: 'Reference', items: pages('reference') },
        { label: 'Import a wisp home', items: pages('migrate') },
      ],
    }),
  ],
});
